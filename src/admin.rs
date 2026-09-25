//! Administration, initialization, and ownership transfer.
//!
//! The contract has exactly one initialization path and a two-step ownership
//! transfer. Both exist for the same reason: an irreversible mistake here
//! bricks the contract permanently, because the admin key is the only way to
//! reach anything else.
//!
//! # Invariants
//!
//! - `DataKey::Admin` is written exactly once, by [`initialize`].
//! - Administrative state lives in instance storage, so every function that
//!   touches it calls [`bump_instance`] before reading. An archived instance
//!   means the contract does not exist.
//! - Authorization is checked before storage is read for authorization
//!   purposes, and every function that can change control ends in either
//!   `require_auth` on the relevant address or a panic.

use soroban_sdk::{contractevent, Address, Env};

use crate::storage_types::{bump_instance, DataKey};

/// Emitted once, when the initial admin is set.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminInitialized {
    /// The address that was made admin.
    #[topic]
    pub admin: Address,
}

/// Emitted when the admin nominates a new admin. No control has moved yet.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminProposed {
    /// The admin that proposed the transfer.
    #[topic]
    pub previous_admin: Address,
    /// The nominee, which has not yet accepted.
    pub new_admin: Address,
}

/// Emitted when a nominee accepts and becomes the admin.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminTransferred {
    /// The admin that handed over control.
    #[topic]
    pub previous_admin: Address,
    /// The nominee, now in control.
    pub new_admin: Address,
}

/// Whether the contract has been initialized, i.e. whether an admin is set.
///
/// Reads instance storage without bumping: this is a cheap existence check
/// that callers use to decide whether to initialize, and a caller asking
/// "am I initialized?" has no reason to keep the contract alive.
pub fn is_initialized(e: &Env) -> bool {
    e.storage().instance().has(&DataKey::Admin)
}

/// Panic unless the contract has been initialized.
///
/// Call this at the top of every entry point that reads or writes contract
/// state. An uninitialized contract has no admin and no balances, so operating
/// on one is always a caller error rather than a legitimate state.
///
/// # Panics
///
/// With `"contract not initialized"` if no admin is set.
pub fn require_initialized(e: &Env) {
    if !is_initialized(e) {
        panic!("contract not initialized");
    }
}

/// Load the stored admin, or panic if the contract is uninitialized.
///
/// # Panics
///
/// With `"contract not initialized"` if no admin is set.
fn load_admin(e: &Env) -> Address {
    match e.storage().instance().get(&DataKey::Admin) {
        Some(admin) => admin,
        None => panic!("contract not initialized"),
    }
}

/// Authorize `caller` as the admin, or panic.
///
/// This is the single shared authorization check for admin-only functions.
/// Modules call this instead of re-implementing the load-and-compare, so the
/// rule that only the admin may act cannot drift between entry points.
///
/// # Panics
///
/// With `"contract not initialized"` if no admin is set, or `"caller is not
/// the admin"` if `caller` is not the stored admin.
pub fn check_admin(e: &Env, caller: &Address) {
    require_initialized(e);
    let admin = load_admin(e);
    if admin != *caller {
        panic!("caller is not the admin");
    }
    caller.require_auth();
}

/// Set the initial admin.
///
/// Writes `DataKey::Admin` once. Re-running this is rejected rather than
/// silently reassigning control, because a second call would hand the contract
/// to whoever invoked it last.
///
/// # Panics
///
/// With `"contract already initialized"` if an admin is already set.
pub fn initialize(e: Env, admin: Address) {
    bump_instance(&e);
    if is_initialized(&e) {
        panic!("contract already initialized");
    }
    e.storage().instance().set(&DataKey::Admin, &admin);
    AdminInitialized { admin }.publish(&e);
}

/// Propose a new admin without handing over control yet.
///
/// A one-step transfer to a mistyped address is unrecoverable: the admin key is
/// the only route to every other capability, so sending it to an address
/// nobody controls bricks the contract permanently. Splitting the transfer
/// into propose and accept means the new admin has to prove it can sign before
/// anything moves.
///
/// The proposal is stored alongside `AdminActiveAfterLedger`, which records the
/// ledger from which the nominee becomes authoritative once it accepts.
///
/// # Panics
///
/// With `"contract not initialized"` if the contract is uninitialized, or
/// `"cannot transfer ownership to the current admin"` if `new_admin` is
/// already the admin.
pub fn transfer_ownership(e: &Env, new_admin: Address) {
    bump_instance(e);
    require_initialized(e);
    let admin = load_admin(e);
    admin.require_auth();

    if admin == new_admin {
        panic!("cannot transfer ownership to the current admin");
    }

    e.storage()
        .instance()
        .set(&DataKey::PendingAdmin, &new_admin);
    e.storage()
        .instance()
        .set(&DataKey::AdminActiveAfterLedger, &e.ledger().sequence());
    AdminProposed {
        previous_admin: admin,
        new_admin,
    }
    .publish(e);
}

/// Accept a pending ownership transfer, becoming the admin.
///
/// The nominee authorizes this call, which is what makes the transfer
/// two-step: an admin who mistypes the destination cannot lock the contract
/// out, because a transfer to an address that cannot sign simply never
/// completes and the existing admin keeps control.
///
/// On success the proposal is cleared, so a completed transfer cannot be
/// replayed, and `AdminActiveAfterLedger` is set to the accepting ledger.
///
/// # Panics
///
/// With `"no pending admin transfer"` if there is no proposal, or
/// `"caller is not the proposed admin"` if `new_admin` is not the stored
/// nominee.
pub fn accept_admin(e: &Env, new_admin: Address) {
    bump_instance(e);
    require_initialized(e);

    let pending: Address = match e.storage().instance().get(&DataKey::PendingAdmin) {
        Some(pending) => pending,
        None => panic!("no pending admin transfer"),
    };

    if pending != new_admin {
        panic!("caller is not the proposed admin");
    }

    new_admin.require_auth();

    let previous_admin = load_admin(e);
    e.storage().instance().set(&DataKey::Admin, &new_admin);
    e.storage().instance().remove(&DataKey::PendingAdmin);
    e.storage()
        .instance()
        .set(&DataKey::AdminActiveAfterLedger, &e.ledger().sequence());
    AdminTransferred {
        previous_admin,
        new_admin,
    }
    .publish(e);
}
