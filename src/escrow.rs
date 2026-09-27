use crate::events::EscrowCreated;
use crate::storage_types::{DataKey, EscrowRecord, EscrowStatus};
use crate::validation::require_positive_amount;
use soroban_sdk::{token, Address, Env};

/// Number of escrows created so far, which is also the next escrow id.
///
/// Ids are handed out by incrementing this counter rather than by deriving
/// anything from the call, so an id is never reused and a settled or refunded
/// escrow cannot be confused with a live one that happens to sit at the same
/// storage offset.
pub fn count(e: &Env) -> u32 {
    e.storage().persistent().get(&DataKey::EscrowCount).unwrap_or(0)
}

/// Total token amount held across all active escrows.
pub fn value_locked(e: &Env) -> i128 {
    e.storage()
        .persistent()
        .get(&DataKey::EscrowValueLocked)
        .unwrap_or(0)
}

/// Stores an escrow record under the next id and advances the counter past it.
fn store_new(e: &Env, record: &EscrowRecord) -> u32 {
    let id = count(e);
    e.storage().persistent().set(&DataKey::EscrowRecord(id), record);
    e.storage().persistent().set(&DataKey::EscrowCount, &id + 1);
    id
}

/// Adds `amount` to the total held in active escrows.
fn increase_value_locked(e: &Env, amount: i128) {
    let new_total = value_locked(e)
        .checked_add(amount)
        .unwrap_or_else(|| panic!("EscrowOverflow: locked value would overflow i128"));
    e.storage()
        .persistent()
        .set(&DataKey::EscrowValueLocked, &new_total);
}

/// Reads an escrow record.
///
/// # Panics
///
/// Panics with `EscrowNotFound` when no escrow exists at `id`.
pub fn record(e: &Env, id: u32) -> EscrowRecord {
    e.storage()
        .persistent()
        .get(&DataKey::EscrowRecord(id))
        .unwrap_or_else(|| panic!("EscrowNotFound: no escrow with id {}", id))
}

/// Holds `amount` of `token` on the contract for `beneficiary` and returns the
/// new escrow's id.
///
/// Funds leave the depositor's account immediately and only reach the
/// beneficiary once the event completes, so the contract — not the depositor —
/// holds them in the meantime.
///
/// The transfer goes through the token's own SEP-41 client rather than this
/// contract's internal ledger, because the escrowed token is a separate
/// contract: a caller may escrow a token this contract knows nothing about.
///
/// # Panics
///
/// Panics through [`require_positive_amount`] on a non-positive `amount`, and
/// propagates whatever the token contract raises when the depositor's balance
/// or the token's own authorization rules are not satisfied.
pub fn create(
    e: &Env,
    depositor: &Address,
    beneficiary: &Address,
    token_address: &Address,
    amount: i128,
    deadline_ledger: u32,
) -> u32 {
    require_positive_amount(amount);
    depositor.require_auth();

    // Move the funds before recording the escrow. A token transfer that fails
    // panics and reverts the whole invocation, so there is no window in which
    // an escrow exists for funds that never arrived.
    token::TokenClient::new(e, token_address).transfer(
        depositor,
        &e.current_contract_address(),
        &amount,
    );

    let id = store_new(
        e,
        &EscrowRecord {
            depositor: depositor.clone(),
            beneficiary: beneficiary.clone(),
            token: token_address.clone(),
            amount,
            deadline_ledger,
            status: EscrowStatus::Active,
        },
    );
    increase_value_locked(e, amount);

    EscrowCreated {
        depositor: depositor.clone(),
        beneficiary: beneficiary.clone(),
        id,
        token: token_address.clone(),
        amount,
        deadline_ledger,
    }
    .publish(e);

    id
}
