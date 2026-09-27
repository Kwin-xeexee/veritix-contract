use crate::control;
use crate::events::{Burn, Clawback, Mint, Transfer, TransferWithMemo};
use crate::storage_types::DataKey;
use crate::validation::{require_memo_within_limit, require_positive_amount};
use soroban_sdk::{Address, Bytes, Env, Vec};

/// Tokens in circulation. An absent key means the contract has minted nothing.
pub fn total_supply(e: &Env) -> i128 {
    e.storage()
        .persistent()
        .get(&DataKey::TotalSupply)
        .unwrap_or(0)
}

/// The configured hard cap on total supply, or 0 when supply is unlimited.
///
/// The `0` sentinel means "no cap has been configured" rather than "no tokens may
/// ever exist": the cap is only written by the initializer, and an unset cap
/// must not read as a cap of zero.
pub fn max_supply(e: &Env) -> i128 {
    e.storage().persistent().get(&DataKey::MaxSupply).unwrap_or(0)
}

/// Tokens held by `account`. An absent key means a zero balance.
pub fn balance_of(e: &Env, account: &Address) -> i128 {
    e.storage()
        .persistent()
        .get(&DataKey::BalanceOf(account.clone()))
        .unwrap_or(0)
}

/// Every account that currently holds a positive balance, in credit order.
pub fn holders(e: &Env) -> Vec<Address> {
    e.storage()
        .persistent()
        .get(&DataKey::HolderSet)
        .unwrap_or_else(|| Vec::new(e))
}

/// Number of accounts in [`holders`], cached so counting stays O(1).
pub fn holder_count(e: &Env) -> u32 {
    e.storage()
        .persistent()
        .get(&DataKey::HolderCount)
        .unwrap_or(0)
}

/// Tokens of `account` held in active escrows and therefore not transferable.
pub fn escrow_locked(e: &Env, account: &Address) -> i128 {
    e.storage()
        .persistent()
        .get(&DataKey::EscrowLocked(account.clone()))
        .unwrap_or(0)
}

/// Tokens `account` may actually move right now.
///
/// A raw balance overstates what is available whenever part of it is escrowed or
/// the account is frozen, and a caller that trusts the balance instead of this
/// figure is the bug this view exists to prevent. Frozen accounts report `0`
/// because nothing at all is transferable, and the subtraction is floored at
/// zero so a stale lock record can never make this view wrap negative.
pub fn spendable_balance(e: &Env, account: &Address) -> i128 {
    if control::is_frozen(e, account) {
        return 0;
    }
    let available = balance_of(e, account) - escrow_locked(e, account);
    if available > 0 {
        available
    } else {
        0
    }
}

/// Adds `account` to the holder set if it is not already a member.
fn add_holder(e: &Env, account: &Address) {
    let mut set = holders(e);
    if !set.contains(account.clone()) {
        set.push_back(account.clone());
        e.storage().persistent().set(&DataKey::HolderSet, &set);
    }
    e.storage()
        .persistent()
        .set(&DataKey::HolderCount, &set.len());
}

/// Removes `account` from the holder set if it is a member.
fn remove_holder(e: &Env, account: &Address) {
    let mut set = holders(e);
    if let Some(index) = set.first_index_of(account.clone()) {
        set.remove(index);
        e.storage().persistent().set(&DataKey::HolderSet, &set);
    }
    e.storage()
        .persistent()
        .set(&DataKey::HolderCount, &set.len());
}

/// Credits `amount` to `account` without touching total supply.
///
/// A transition from zero to a positive balance is what puts an account on the
/// holder set, so the two can never disagree.
pub fn credit(e: &Env, account: &Address, amount: i128) {
    let new_balance = balance_of(e, account)
        .checked_add(amount)
        .unwrap_or_else(|| panic!("BalanceOverflow: {} credit would overflow i128", amount));
    e.storage()
        .persistent()
        .set(&DataKey::BalanceOf(account.clone()), &new_balance);
    add_holder(e, account);
}

/// Debits `amount` from `account`.
///
/// Reaching zero deletes the balance key and drops the account from the holder
/// set in the same step, so an enumerating sweep can never pay a holder who
/// has already emptied their balance.
///
/// # Panics
///
/// Panics with `InsufficientBalance` when the account holds less than `amount`.
pub fn debit(e: &Env, account: &Address, amount: i128) {
    let balance = balance_of(e, account);
    if balance < amount {
        panic!("InsufficientBalance: {} available, {} required", balance, amount);
    }
    let new_balance = balance - amount;
    if new_balance == 0 {
        e.storage()
            .persistent()
            .remove(&DataKey::BalanceOf(account.clone()));
        remove_holder(e, account);
    } else {
        e.storage()
            .persistent()
            .set(&DataKey::BalanceOf(account.clone()), &new_balance);
    }
}

/// Adds `amount` to total supply, enforcing the configured cap.
pub fn increase_supply(e: &Env, amount: i128) {
    let new_supply = total_supply(e)
        .checked_add(amount)
        .unwrap_or_else(|| panic!("SupplyOverflow: minting {} would overflow i128", amount));
    let cap = max_supply(e);
    if cap > 0 && new_supply > cap {
        panic!("SupplyCapExceeded: {} would exceed the cap of {}", new_supply, cap);
    }
    e.storage()
        .persistent()
        .set(&DataKey::TotalSupply, &new_supply);
}

/// Subtracts `amount` from total supply.
pub fn decrease_supply(e: &Env, amount: i128) {
    let supply = total_supply(e);
    let new_supply = supply
        .checked_sub(amount)
        .unwrap_or_else(|| panic!("SupplyUnderflow: burning {} would take supply below zero", amount));
    e.storage()
        .persistent()
        .set(&DataKey::TotalSupply, &new_supply);
}

/// Brings `amount` of new tokens into circulation for `to`.
pub fn mint(e: &Env, to: &Address, amount: i128) {
    require_positive_amount(amount);
    increase_supply(e, amount);
    credit(e, to, amount);
    Mint {
        to: to.clone(),
        amount,
    }
    .publish(e);
}

/// Destroys `amount` of `from`'s own tokens, reducing balance and supply.
pub fn burn(e: &Env, from: &Address, amount: i128) {
    require_positive_amount(amount);
    debit(e, from, amount);
    decrease_supply(e, amount);
    Burn {
        from: from.clone(),
        amount,
    }
    .publish(e);
}

/// Destroys `amount` of `from`'s tokens on `spender`'s authority, reducing
/// balance and supply.
///
/// Mirrors [`burn`] but consumes an allowance instead of requiring the holder to
/// sign, and emits the same `["burn", from]` event: the tokens left `from`'s
/// account either way, and an indexer reconstructing balances from events should
/// not have to care which signature was used.
pub fn burn_from(e: &Env, spender: &Address, from: &Address, amount: i128) {
    require_positive_amount(amount);
    crate::allowance::consume_allowance(e, from, spender, amount);
    debit(e, from, amount);
    decrease_supply(e, amount);
    Burn {
        from: from.clone(),
        amount,
    }
    .publish(e);
}

/// Removes `amount` from `from` for an admin clawback, without the holder
/// authorizing the spend.
pub fn clawback(e: &Env, admin: &Address, from: &Address, amount: i128) {
    require_positive_amount(amount);
    debit(e, from, amount);
    decrease_supply(e, amount);
    Clawback {
        admin: admin.clone(),
        from: from.clone(),
        amount,
    }
    .publish(e);
}

/// Moves `amount` of tokens from `from` to `to`, leaving total supply untouched.
///
/// The compliance checks live here rather than in the contract entry point so
/// every way of moving tokens consults the same ones. They run before any
/// balance is read, and the debit runs before the credit, so a rejected transfer
/// cannot leave a half-applied state behind.
pub fn transfer(e: &Env, from: &Address, to: &Address, amount: i128) {
    require_positive_amount(amount);
    control::require_not_paused(e);
    control::require_not_frozen(e, from);
    control::require_not_frozen(e, to);
    debit(e, from, amount);
    credit(e, to, amount);
    Transfer {
        from: from.clone(),
        to: to.clone(),
        amount,
    }
    .publish(e);
}

/// Moves `amount` from `from` to `to` on `spender`'s authority, consuming the
/// spender's allowance.
///
/// Identical to [`transfer`] apart from who authorized it, and it emits the same
/// `["transfer", from, to]` event for the same indexing reason as [`burn_from`].
pub fn transfer_from(e: &Env, spender: &Address, from: &Address, to: &Address, amount: i128) {
    require_positive_amount(amount);
    control::require_not_paused(e);
    control::require_not_frozen(e, from);
    control::require_not_frozen(e, to);
    crate::allowance::consume_allowance(e, from, spender, amount);
    debit(e, from, amount);
    credit(e, to, amount);
    Transfer {
        from: from.clone(),
        to: to.clone(),
        amount,
    }
    .publish(e);
}

/// Moves `amount` from `from` to `to` and tags the movement with an opaque memo.
///
/// Same state transition as [`transfer`], with the memo carried only in the
/// event and never written to storage.
pub fn transfer_with_memo(e: &Env, from: &Address, to: &Address, amount: i128, memo: Bytes) {
    require_memo_within_limit(memo.len());
    require_positive_amount(amount);
    control::require_not_paused(e);
    control::require_not_frozen(e, from);
    control::require_not_frozen(e, to);
    debit(e, from, amount);
    credit(e, to, amount);
    TransferWithMemo {
        from: from.clone(),
        to: to.clone(),
        amount,
        memo,
    }
    .publish(e);
}
