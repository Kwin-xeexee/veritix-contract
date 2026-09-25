use crate::authorization;
use crate::events::{Burn, Mint, Transfer, TransferWithMemo};
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
/// The `0` sentinel means "no cap configured" rather than "no tokens may ever
/// exist": the cap is only written by the initializer, and an unset cap must
/// not block minting. A deployment that never configures one is uncapped.
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
///
/// Airdrops and dividend sweeps enumerate this instead of scanning balances, so
/// a stale entry is not a cosmetic problem — it pays a holder who has already
/// emptied their balance. Insert and removal are therefore handled in
/// [`credit`] and [`debit`] rather than left to callers.
pub fn holders(e: &Env) -> Vec<Address> {
    e.storage()
        .persistent()
        .get(&DataKey::HolderSet)
        .unwrap_or_else(|| Vec::new(e))
}

/// Number of accounts in [`holders`].
///
/// Cached alongside the set so the common case — "how many people do I need to
/// pay" — does not deserialize the whole set on every query.
pub fn holder_count(e: &Env) -> u32 {
    e.storage()
        .persistent()
        .get(&DataKey::HolderCount)
        .unwrap_or(0)
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
/// Callers that bring new tokens into circulation must also call
/// [`increase_supply`] so the two ledgers cannot drift apart. A transition from
/// zero to a positive balance is what puts an account on the holder set.
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
/// Reaching zero removes the account from the holder set in the same step that
/// deletes its balance key, so the set cannot outlive the balances it mirrors.
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
///
/// # Panics
///
/// Panics with `SupplyCapExceeded` when the cap is set and the new total would
/// exceed it, and with `SupplyOverflow` on an `i128` overflow.
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
///
/// This is the only place that credits balances and grows supply together, so
/// the two ledgers cannot be updated independently and drift apart.
///
/// # Panics
///
/// Panics through [`require_positive_amount`] on a non-positive `amount` and
/// through [`increase_supply`] when the cap would be exceeded.
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

/// Destroys `amount` of `from`'s tokens, reducing both the balance and supply.
///
/// # Panics
///
/// Panics on a non-positive `amount` and when the balance is too small.
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

/// Moves `amount` of tokens from `from` to `to`, leaving total supply untouched.
///
/// The authorization check lives here rather than in the contract entry point
/// so that every way of moving tokens — plain, memo-carrying, or added later —
/// consults the same flag. The debit runs before the credit so an insufficient
/// balance aborts before the recipient is paid.
///
/// # Panics
///
/// Panics on a non-positive `amount`, when `from` is not authorized, or when
/// `from` holds less than `amount`.
pub fn transfer(e: &Env, from: &Address, to: &Address, amount: i128) {
    require_positive_amount(amount);
    authorization::require_authorized(e, from);
    debit(e, from, amount);
    credit(e, to, amount);
    Transfer {
        from: from.clone(),
        to: to.clone(),
        amount,
    }
    .publish(e);
}

/// Moves `amount` from `from` to `to` and tags the movement with an opaque
/// `memo`.
///
/// Same state transition as [`transfer`], and the same authorization check, with
/// the memo carried only in the event. Nothing about the memo is interpreted or
/// stored, which is what lets a ticketing platform attach an order reference
/// without a second write to the contract's storage.
///
/// # Panics
///
/// Panics through [`require_memo_within_limit`] when the memo exceeds
/// [`crate::validation::MAX_MEMO_BYTES`], and with everything [`transfer`]
/// panics on.
pub fn transfer_with_memo(e: &Env, from: &Address, to: &Address, amount: i128, memo: Bytes) {
    require_memo_within_limit(memo.len());
    require_positive_amount(amount);
    authorization::require_authorized(e, from);
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
