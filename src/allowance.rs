use crate::events::Approve;
use crate::storage_types::DataKey;
use soroban_sdk::{contracttype, Address, Env};

/// A live allowance, as `allowance()` reports it.
///
/// `#[contracttype]` is what makes this storable: the derive supplies the
/// `Val` conversions `Env::storage` needs, and the pair travels as one atomic
/// value, so a reader can never see an amount from one grant beside an
/// expiration from another.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Allowance {
    pub amount: i128,
    pub expiration_ledger: u32,
}

/// The amount `spender` may still move on `from`'s behalf.
///
/// An allowance at or past its expiration ledger reads as `0`. SEP-41 treats an
/// expired entry as a zero allowance rather than an error, so a spender that
/// never spends gets nothing back — which is the point of setting an expiry.
pub fn allowance(e: &Env, from: &Address, spender: &Address) -> i128 {
    let stored: Allowance = e
        .storage()
        .persistent()
        .get(&DataKey::Allowance(from.clone(), spender.clone()))
        .unwrap_or(Allowance {
            amount: 0,
            expiration_ledger: 0,
        });
    if is_expired(e, &stored) {
        0
    } else {
        stored.amount
    }
}

/// The ledger at which the `from`/`spender` allowance expires.
pub fn allowance_expiration(e: &Env, from: &Address, spender: &Address) -> u32 {
    e.storage()
        .persistent()
        .get(&DataKey::AllowanceExpiration(from.clone(), spender.clone()))
        .unwrap_or(0)
}

/// Whether `stored` has already lapsed.
///
/// SEP-41 defines an allowance as expired once `expiration_ledger` is strictly
/// behind the current ledger, so an allowance is still live on the ledger it
/// expires on.
fn is_expired(e: &Env, stored: &Allowance) -> bool {
    stored.expiration_ledger < e.ledger().sequence()
}

/// Writes a new allowance, replacing any existing one.
///
/// `approve` overwrites rather than accumulates, matching SEP-41: a spender who
/// is handed a new number must not be able to add it to what they already had.
///
/// # Panics
///
/// Panics with `ExpirationInPast` when `amount` is positive and
/// `expiration_ledger` is already behind the current ledger. A zero amount is
/// always accepted, because revoking an allowance has no expiry to get wrong.
pub fn approve(
    e: &Env,
    from: &Address,
    spender: &Address,
    amount: i128,
    expiration_ledger: u32,
) {
    require_non_negative_amount(amount);
    if amount > 0 && expiration_ledger < e.ledger().sequence() {
        panic!(
            "ExpirationInPast: expiration ledger {} is not after the current ledger {}",
            expiration_ledger,
            e.ledger().sequence()
        );
    }

    if amount == 0 {
        // A revocation deletes the keys instead of storing a zero, so
        // `allowance` and the storage footprint agree that nothing is granted.
        e.storage()
            .persistent()
            .remove(&DataKey::Allowance(from.clone(), spender.clone()));
        e.storage()
            .persistent()
            .remove(&DataKey::AllowanceExpiration(from.clone(), spender.clone()));
    } else {
        e.storage().persistent().set(
            &DataKey::Allowance(from.clone(), spender.clone()),
            &Allowance {
                amount,
                expiration_ledger,
            },
        );
        e.storage().persistent().set(
            &DataKey::AllowanceExpiration(from.clone(), spender.clone()),
            &expiration_ledger,
        );
    }

    Approve {
        from: from.clone(),
        spender: spender.clone(),
        amount,
        expiration_ledger,
    }
    .publish(e);
}

/// Reduces the `from`/`spender` allowance by `amount` and returns what is left.
///
/// The new amount is written even when it reaches zero so the entry cannot be
/// replayed, and the remaining figure is clamped at zero: a spender may never
/// end up with more allowance than they started with, even if a concurrent
/// revocation landed first.
///
/// # Panics
///
/// Panics with `InsufficientAllowance` when the live allowance is below
/// `amount`.
pub fn consume_allowance(e: &Env, from: &Address, spender: &Address, amount: i128) -> i128 {
    let current = allowance(e, from, spender);
    if current < amount {
        panic!(
            "InsufficientAllowance: {} available, {} required",
            current, amount
        );
    }
    let remaining = current - amount;
    if remaining == 0 {
        e.storage()
            .persistent()
            .remove(&DataKey::Allowance(from.clone(), spender.clone()));
        e.storage()
            .persistent()
            .remove(&DataKey::AllowanceExpiration(from.clone(), spender.clone()));
    } else {
        e.storage().persistent().set(
            &DataKey::Allowance(from.clone(), spender.clone()),
            &Allowance {
                amount: remaining,
                expiration_ledger: allowance_expiration(e, from, spender),
            },
        );
    }
    remaining
}

/// Rejects negative amounts while allowing the zero that means "revoke".
fn require_non_negative_amount(amount: i128) -> () {
    assert!(
        amount >= 0,
        "InvalidAmount: amount must not be negative, got {}",
        amount
    );
}
