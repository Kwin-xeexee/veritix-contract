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

/// Ledger the contract was initialized on, or 0 when it never was.
pub fn initialized_at_ledger(e: &Env) -> u32 {
    e.storage()
        .persistent()
        .get(&DataKey::InitializedAtLedger)
        .unwrap_or(0)
}