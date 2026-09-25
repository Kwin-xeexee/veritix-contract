use crate::control;
use crate::events::{Burn, Clawback, Mint, Transfer};
use crate::events::Mint;
use crate::storage_types::DataKey;
use crate::validation::require_positive_amount;
use soroban_sdk::{Address, Env};

/// Tokens in circulation. An absent key means the contract has minted nothing.
pub fn total_supply(e: &Env) -> i128 {
    e.storage()
        .persistent()
        .get(&DataKey::TotalSupply)
        .unwrap_or(0)
}

/// The configured hard cap, or 0 when supply is uncapped.
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

/// Tokens of `account` that are held in active escrows and cannot be spent.
pub fn escrow_locked(e: &Env, account: &Address) -> i128 {
    e.storage()
        .persistent()
        .get(&DataKey::EscrowLocked(account.clone()))
        .unwrap_or(0)
}

/// Tokens `account` may actually move right now.
///
/// A raw balance overstates what is available whenever part of it is escrowed
/// or the account is frozen, and a caller that trusts the balance instead of
/// this figure is the bug this view exists to prevent. Frozen accounts report
/// `0` because nothing at all is transferable. The subtraction is floored at
/// zero so stale lock records can never make this view wrap negative.
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

/// Credits `amount` to `account` without touching total supply.
///
/// Callers that bring new tokens into circulation must also call
/// [`increase_supply`] so the two ledgers cannot drift apart.
pub fn credit(e: &Env, account: &Address, amount: i128) {
    let new_balance = balance_of(e, account)
        .checked_add(amount)
        .unwrap_or_else(|| panic!("BalanceOverflow: {} credit would overflow i128", amount));
    e.storage()
        .persistent()
        .set(&DataKey::BalanceOf(account.clone()), &new_balance);
}

/// Debits `amount` from `account`.
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
    } else {
        e.storage()
            .persistent()
            .set(&DataKey::BalanceOf(account.clone()), &new_balance);
    }
}

/// Adds `amount` to total supply, enforcing the configured cap.
///
/// A cap of 0 is the "no cap" sentinel: `max_supply` is unset until the
/// initializer opts into a limit, and an absent cap must not block minting.
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

/// Removes `amount` from `from` for an admin clawback, without the holder
/// authorizing the spend.
///
/// This is deliberately separate from [`burn`]: the two differ in who allowed
/// the tokens to leave and in which event they emit, so an auditor can tell a
/// holder's own burn apart from an admin recovery in the event log even though
/// both reduce supply.
///
/// # Panics
///
/// Panics on a non-positive `amount` and when the balance is too small.
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

/// Moves `amount` from `from` to `to`, leaving total supply untouched.
///
/// Every compliance control lands here. Pausing and the frozen flags are
/// checked before any balance is read, so a blocked transfer cannot leave a
/// half-applied state behind, and the debit runs before the credit so an
/// insufficient balance aborts before the recipient is paid.
///
/// # Panics
///
/// Panics on a non-positive `amount`, when either party is frozen, while the
/// contract is paused, or when `from` holds less than `amount`.
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

/// Records `amount` of `account`'s tokens as held by an active escrow.
///
/// # Panics
///
/// Panics when the account's balance cannot cover the lock.
pub fn lock_in_escrow(e: &Env, account: &Address, amount: i128) {
    require_positive_amount(amount);
    let locked = escrow_locked(e, account);
    let new_locked = locked
        .checked_add(amount)
        .unwrap_or_else(|| panic!("BalanceOverflow: escrow lock would overflow i128"));
    if new_locked > balance_of(e, account) {
        panic!(
            "InsufficientBalance: {} held, cannot lock {}",
            balance_of(e, account),
            new_locked
        );
    }
    e.storage()
        .persistent()
        .set(&DataKey::EscrowLocked(account.clone()), &new_locked);
}

/// Releases `amount` of `account`'s escrow lock.
///
/// # Panics
///
/// Panics when the lock does not cover `amount`.
pub fn unlock_from_escrow(e: &Env, account: &Address, amount: i128) {
    require_positive_amount(amount);
    let locked = escrow_locked(e, account);
    if locked < amount {
        panic!("EscrowLockUnderflow: {} locked, {} requested", locked, amount);
    }
    let new_locked = locked - amount;
    if new_locked == 0 {
        e.storage()
            .persistent()
            .remove(&DataKey::EscrowLocked(account.clone()));
    } else {
        e.storage()
            .persistent()
            .set(&DataKey::EscrowLocked(account.clone()), &new_locked);
    }
}
