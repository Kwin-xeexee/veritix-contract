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

/// Emitted when the contract is initialized with a hard supply cap.
///
/// There is deliberately no counterpart event: the cap can never be raised
/// afterwards.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaxSupplyInitialized {
    /// The address that was made admin.
    #[topic]
    pub admin: Address,
    /// The cap fixed for the lifetime of the contract.
    pub max_supply: i128,
}

/// Emitted when the clawback co-signer is set or replaced.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClawbackCosignerSet {
    /// The admin that set the co-signer.
    #[topic]
    pub admin: Address,
    /// The co-signer that must now authorise every clawback.
    pub cosigner: Address,
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

/// The current admin.
///
/// # Panics
///
/// With `"contract not initialized"` if no admin is set. This is deliberately
/// not an `Option`: a caller that has to handle "no admin" on the read path is
/// a caller that is about to do something wrong.
pub fn current_admin(e: &Env) -> Address {
    require_initialized(e);
    load_admin(e)
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
use crate::storage_types::DataKey;
use soroban_sdk::{Address, Env};

/// True once `initialize` has stored an admin address.
pub fn is_initialized(e: &Env) -> bool {
    e.storage().persistent().has(&DataKey::Admin)
}

/// Panics while the contract is still uninitialized.
pub fn require_initialized(e: &Env) {
    if !is_initialized(e) {
        panic!("NotInitialized: call initialize before using the contract");
    }
}

/// The address that currently holds admin authority.
pub fn admin(e: &Env) -> Address {
    e.storage()
        .persistent()
        .get(&DataKey::Admin)
        .expect("NotInitialized: admin not set")
}

/// True when `caller` is the stored admin.
pub fn is_admin(e: &Env, caller: &Address) -> bool {
    e.storage()
        .persistent()
        .get::<_, Address>(&DataKey::Admin)
        == Some(*caller)
}

/// Requires that `caller` is the stored admin and authorized this invocation.
///
/// Admin-gated entry points call this first so an unauthorized address can never
/// reach the state transition behind it.
pub fn check_admin(e: &Env, caller: &Address) {
    require_initialized(e);
    if !is_admin(e, caller) {
        panic!("Unauthorized: caller is not the contract admin");
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

/// Set the initial admin and a hard supply cap in one call.
///
/// Equivalent to [`initialize`] plus a cap that can never be changed, for
/// deployments that want a fixed ceiling from the first ledger. There is
/// deliberately no `set_max_supply` anywhere in this contract: a cap that can
/// be raised later is not a cap, and the guard that would stop a future
/// contributor from adding one is the absence of the function.
///
/// [`initialize`] remains available for deployments that want no cap at all.
///
/// # Panics
///
/// With `"max supply must be positive"` if `max_supply` is not strictly
/// positive, or `"contract already initialized"` if an admin is already set.
pub fn initialize_with_max_supply(e: Env, admin: Address, max_supply: i128) {
    // Validate the cap before touching state, so a rejected call leaves nothing
    // behind even in a host that does not roll back on panic.
    if max_supply <= 0 {
        panic!("max supply must be positive");
    }
    bump_instance(&e);
    // Delegates the "exactly one admin" rule to the single initialization path
    // rather than reimplementing it, so the two cannot drift apart.
    initialize(e.clone(), admin.clone());
    e.storage().instance().set(&DataKey::MaxSupply, &max_supply);
    MaxSupplyInitialized { admin, max_supply }.publish(&e);
}

/// The nominated next admin, if a transfer is in flight.
///
/// `None` means no transfer is outstanding — either none was ever proposed, or
/// the last one completed and was cleared.
///
/// Reads state without authorizing: an operator watching for an in-flight
/// rotation has no admin key and still needs to see it.
pub fn pending_admin(e: &Env) -> Option<Address> {
    e.storage().instance().get(&DataKey::PendingAdmin)
}

/// The ledger from which the most recent accepted admin is authoritative.
///
/// `0` means no admin has taken over yet. Recorded on both propose and accept
/// so an operator can tell a stale proposal from a fresh one.
pub fn admin_active_after_ledger(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get(&DataKey::AdminActiveAfterLedger)
        .unwrap_or(0)
}

/// The hard supply cap fixed at initialization, if this deployment has one.
///
/// `None` for a contract initialized with plain [`initialize`].
pub fn max_supply(e: &Env) -> Option<i128> {
    e.storage().instance().get(&DataKey::MaxSupply)
}

/// Set or replace the address that must co-sign every clawback.
///
/// Clawback is the most dangerous power an admin holds, and a single
/// compromised key should not be enough to exercise it. Naming a co-signer
/// raises that bar from one key to two, which limits the blast radius of that
/// compromise to *disabling* clawback rather than *using* it.
///
/// Passing the current co-signer again is how it is replaced. There is
/// deliberately no `clear_clawback_cosigner`: silently dropping back to a single
/// key would reintroduce exactly the risk this guards against, so removing a
/// co-signer means rotating the admin to a contract that never had one.
///
/// # Panics
///
/// With `"contract not initialized"` or `"caller is not the admin"` if `admin`
/// is not authorized, or `"co-signer must differ from the admin"` if the
/// co-signer is the admin itself. A co-signer equal to the admin would satisfy
/// the dual-authorization requirement with one signature.
pub fn set_clawback_cosigner(e: &Env, admin: &Address, cosigner: &Address) {
    bump_instance(e);
    check_admin(e, admin);

    if admin == cosigner {
        panic!("co-signer must differ from the admin");
    }

    e.storage()
        .instance()
        .set(&DataKey::ClawbackCosigner, cosigner);
    ClawbackCosignerSet {
        admin: admin.clone(),
        cosigner: cosigner.clone(),
    }
    .publish(e);
}

/// The configured clawback co-signer, if any.
pub fn read_clawback_cosigner(e: &Env) -> Option<Address> {
    e.storage().instance().get(&DataKey::ClawbackCosigner)
}

/// Authorize a clawback: the admin always, plus the co-signer when one is set.
///
/// Clawback entry points call this rather than `check_admin` directly, so that
/// adding a co-signer later cannot leave some existing clawback path
/// single-signed. Falls back to admin-only authorization when no co-signer is
/// configured, which is what makes the feature opt-in.
///
/// # Panics
///
/// With `"contract not initialized"` or `"caller is not the admin"` if `admin`
/// is not authorized. When a co-signer is configured, a missing co-signer
/// signature surfaces as an authorization failure from `require_auth` rather
/// than a panic message of our own.
pub fn require_clawback_auth(e: &Env, admin: &Address) {
    check_admin(e, admin);
    if let Some(cosigner) = read_clawback_cosigner(e) {
        cosigner.require_auth();
    }
/// The second address required to approve a clawback, when one is configured.
pub fn co_signer(e: &Env) -> Option<Address> {
    e.storage().persistent().get(&DataKey::CoSigner)
}

/// Records the optional clawback co-signer during initialization.
pub fn store_co_signer(e: &Env, co_signer: &Option<Address>) {
    if let Some(signer) = co_signer {
        e.storage().persistent().set(&DataKey::CoSigner, signer);
    }
}

/// Requires the co-signer's authorization when one is configured.
///
/// A deployment without a co-signer must not be blocked, so an absent co-signer
/// is a no-op rather than a failure: single-admin deployments claw back exactly
/// as they would without this check.
pub fn require_co_signer(e: &Env) {
    if let Some(signer) = co_signer(e) {
        signer.require_auth();
    }
}

/// Ledger the contract was initialized on, or 0 when it never was.
pub fn initialized_at_ledger(e: &Env) -> u32 {
    e.storage()
        .persistent()
        .get(&DataKey::InitializedAtLedger)
        .unwrap_or(0)
}
}
