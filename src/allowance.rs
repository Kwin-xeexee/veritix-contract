use crate::events::Approve;
use crate::storage_types::DataKey;
use soroban_sdk::{contracttype, Address, Env};

/// A live allowance: how much, and until when.
///
/// `#[contracttype]` is what makes this storable: the derive supplies the
/// `Val` conversions that `Env::storage` needs, and the pair travels as one
/// atomic value so a reader can never observe an amount from one grant beside
/// an expiration from another.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Allowance {
    pub amount: i128,
    pub expiration_ledger: u32,
}

/// Reads the stored allowance entry, or an empty one.
fn load(e: &Env, from: &Address, spender: &Address) -> Allowance {
    e.storage()
        .persistent()
        .get(&DataKey::Allowance(from.clone(), spender.clone()))
        .unwrap_or(Allowance {
            amount: 0,
            expiration_ledger: 0,
        })
}

/// Whether `stored` has already lapsed.
///
/// An allowance is expired once `expiration_ledger` is strictly behind the
/// current ledger, so it is still live on the ledger it expires on.
fn is_expired(e: &Env, stored: &Allowance) -> bool {
    stored.expiration_ledger < e.ledger().sequence()
}

/// Writes an allowance, or deletes the entry when `amount` is zero.
fn store(e: &Env, from: &Address, spender: &Address, amount: i128, expiration_ledger: u32) {
    if amount == 0 {
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
}

/// The amount `spender` may still move on `from`'s behalf.
///
/// An expired allowance reads as `0` rather than as an error. A spender that
/// never spends gets nothing back, which is the whole point of an expiration,
/// and a UI polling this view gets a number it can act on instead of a failure
/// it has to special-case.
pub fn allowance(e: &Env, from: &Address, spender: &Address) -> i128 {
    let stored = load(e, from, spender);
    if is_expired(e, &stored) {
        0
    } else {
        stored.amount
    }
}

/// The ledger at which the `from`/`spender` allowance expires.
///
/// Reports the stored value even once the allowance has lapsed, so a caller can
/// tell "expired at ledger 500" apart from "never granted". The amount alone
/// cannot make that distinction: both read as `0`.
pub fn allowance_expiration(e: &Env, from: &Address, spender: &Address) -> u32 {
    e.storage()
        .persistent()
        .get(&DataKey::AllowanceExpiration(from.clone(), spender.clone()))
        .unwrap_or(0)
}

/// Authorizes `spender` to move up to `amount` of `from`'s tokens.
///
/// Overwrites rather than accumulates, so a spender handed a new number cannot
/// end up with the old one plus the new one. Use [`increase_allowance`] and
/// [`decrease_allowance`] to adjust an existing grant without the re-approval
/// race.
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
    require_non_negative(amount);
    if amount > 0 && expiration_ledger < e.ledger().sequence() {
        panic!(
            "ExpirationInPast: expiration ledger {} is not after the current ledger {}",
            expiration_ledger,
            e.ledger().sequence()
        );
    }
    store(e, from, spender, amount, expiration_ledger);
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
/// Callers run this before moving anything, so a rejected invocation spends no
/// allowance. Because every rejection is a panic and a panic reverts the whole
/// invocation, the ordering is a question of which error a caller sees rather
/// than of what survives.
///
/// # Panics
///
/// Panics with `InsufficientAllowance` when the live allowance is below
/// `amount`. An expired allowance is a live allowance of `0`, so spending one
/// fails here rather than later.
pub fn consume_allowance(e: &Env, from: &Address, spender: &Address, amount: i128) -> i128 {
    let current = allowance(e, from, spender);
    if current < amount {
        panic!(
            "InsufficientAllowance: {} available, {} required",
            current, amount
        );
    }
    let remaining = current - amount;
    store(e, from, spender, remaining, allowance_expiration(e, from, spender));
    remaining
}

/// Raises the `from`/`spender` allowance by `amount`, keeping its expiration.
///
/// Adjusting by a delta rather than overwriting avoids the re-approval race: two
/// clients changing the same grant concurrently cannot lose an update, because
/// neither has to read the current value first and write it back.
///
/// # Panics
///
/// Panics with `NoAllowance` when the live allowance is `0` — either never
/// granted or already expired. There is no expiration left to preserve, and
/// quietly inventing a standing grant with no expiry is exactly the risk an
/// expiration exists to bound. Call `approve` instead.
pub fn increase_allowance(e: &Env, from: &Address, spender: &Address, amount: i128) -> i128 {
    require_positive_delta(amount);
    let current = allowance(e, from, spender);
    if current == 0 {
        panic!("NoAllowance: call approve before adjusting an allowance");
    }
    let new_amount = current
        .checked_add(amount)
        .unwrap_or_else(|| panic!("AllowanceOverflow: {} + {} overflows", current, amount));
    let expiration = allowance_expiration(e, from, spender);
    store(e, from, spender, new_amount, expiration);
    Approve {
        from: from.clone(),
        spender: spender.clone(),
        amount: new_amount,
        expiration_ledger: expiration,
    }
    .publish(e);
    new_amount
}

/// Lowers the `from`/`spender` allowance by `amount`, keeping its expiration.
///
/// Saturates at zero instead of underflowing: reducing an allowance is how a
/// holder pulls back authority, and "reduce by more than I granted" must not
/// fail or wrap into a huge grant. A reduction to zero revokes the allowance
/// outright.
///
/// A live allowance of `0` — never granted, or already expired — is a no-op
/// returning `0` rather than a failure. Pulling authority back is idempotent
/// and safe to retry, and there is nothing left to reduce.
///
/// # Panics
///
/// Panics with `InvalidAmount` when `amount` is not strictly positive.
pub fn decrease_allowance(e: &Env, from: &Address, spender: &Address, amount: i128) -> i128 {
    require_positive_delta(amount);
    let current = allowance(e, from, spender);
    if current == 0 {
        return 0;
    }
    let new_amount = current.saturating_sub(amount);
    let expiration = allowance_expiration(e, from, spender);
    store(e, from, spender, new_amount, expiration);
    Approve {
        from: from.clone(),
        spender: spender.clone(),
        amount: new_amount,
        expiration_ledger: expiration,
    }
    .publish(e);
    new_amount
}

/// Rejects a negative `amount`, allowing the zero that means "revoke".
fn require_non_negative(amount: i128) -> () {
    assert!(
        amount >= 0,
        "InvalidAmount: amount must not be negative, got {}",
        amount
    );
}

/// Rejects a delta that is not strictly positive.
fn require_positive_delta(amount: i128) -> () {
    assert!(
        amount > 0,
        "InvalidAmount: delta must be strictly positive, got {}",
        amount
    );
}
