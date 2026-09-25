use crate::events::{Burn, Mint, Transfer};
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

/// Moves `amount` of tokens from `from` to `to`, leaving total supply untouched.
///
/// The debit runs before the credit so an insufficient balance aborts before the
/// recipient is paid, and the amount is validated before any balance is read.
///
/// # Panics
///
/// Panics on a non-positive `amount` or when `from` holds less than `amount`.
pub fn transfer(e: &Env, from: &Address, to: &Address, amount: i128) {
    require_positive_amount(amount);
    debit(e, from, amount);
    credit(e, to, amount);
    Transfer {
        from: from.clone(),
        to: to.clone(),
        amount,
    }
    .publish(e);
}

/// Moves `amount` from `from` to `to` on `spender`'s authority.
///
/// The allowance is reduced first, then this goes through exactly the same
/// [`transfer`] path. Reducing the allowance up front is what makes a failed
/// spend cost the spender nothing: the rejection is a panic, and a panic
/// reverts the whole invocation, so the balance check and the allowance
/// reduction cannot disagree about whether the spend happened.
///
/// # Panics
///
/// Panics through [`crate::allowance::consume_allowance`] when the live
/// allowance is insufficient or expired, and with everything [`transfer`] panics
/// on.
pub fn transfer_from(e: &Env, spender: &Address, from: &Address, to: &Address, amount: i128) {
    crate::allowance::consume_allowance(e, from, spender, amount);
    transfer(e, from, to, amount);
}

/// Destroys `amount` of the caller's own tokens, reducing balance and supply.
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
/// Mirrors [`burn`] but draws on an allowance instead of the holder's signature.
/// It emits the same `["burn", from]` event: the tokens left `from`'s account
/// either way, and an indexer rebuilding balances from the event log should not
/// have to care which signature was used.
///
/// # Panics
///
/// Panics through [`crate::allowance::consume_allowance`] when the live
/// allowance is insufficient or expired, and with everything [`burn`] panics on.
pub fn burn_from(e: &Env, spender: &Address, from: &Address, amount: i128) {
    crate::allowance::consume_allowance(e, from, spender, amount);
    burn(e, from, amount);
}
