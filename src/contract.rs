//! The contract's public Soroban interface.
//!
//! Every function an integrator can call lives here. The implementations sit in
//! the modules behind it, so this file stays a thin, auditable list of what
//! the contract exposes and which module enforces each rule.
//!
//! The name `VeritixToken` is retained from the pre-rebuild contract, which
//! `CONTRIBUTING.md` still describes.

use soroban_sdk::{contract, contractimpl, Address, Env};

use crate::admin;

/// The deployed contract.
#[contract]
pub struct VeritixToken;

#[contractimpl]
impl VeritixToken {
    /// Initialize with an admin and no supply cap.
    ///
    /// # Panics
    ///
    /// With `"contract already initialized"` if an admin is already set.
    pub fn initialize(e: Env, admin: Address) {
        admin::initialize(e, admin);
    }

    /// Initialize with an admin and a supply cap that can never be raised.
    ///
    /// # Panics
    ///
    /// With `"max supply must be positive"` if `max_supply` is not strictly
    /// positive, or `"contract already initialized"` if an admin is already set.
    pub fn initialize_with_max_supply(e: Env, admin: Address, max_supply: i128) {
        admin::initialize_with_max_supply(e, admin, max_supply);
    }

    /// The current admin.
    ///
    /// # Panics
    ///
    /// With `"contract not initialized"` if the contract is uninitialized.
    pub fn admin(e: &Env) -> Address {
        admin::current_admin(e)
    }

    /// Whether an admin has been set.
    pub fn is_initialized(e: &Env) -> bool {
        admin::is_initialized(e)
    }

    /// The nominated next admin, if a rotation is in flight.
    pub fn pending_admin(e: &Env) -> Option<Address> {
        admin::pending_admin(e)
    }

    /// The ledger from which the most recent accepted admin is authoritative,
    /// or `0` if no admin has taken over.
    pub fn admin_active_after_ledger(e: &Env) -> u32 {
        admin::admin_active_after_ledger(e)
    }

    /// The hard supply cap fixed at initialization, if this deployment has one.
    pub fn max_supply(e: &Env) -> Option<i128> {
        admin::max_supply(e)
    }

    /// Nominate a new admin. No control moves until they accept.
    pub fn transfer_ownership(e: &Env, new_admin: Address) {
        admin::transfer_ownership(e, new_admin);
    }

    /// Accept a pending rotation, becoming the admin.
    pub fn accept_admin(e: &Env, new_admin: Address) {
        admin::accept_admin(e, new_admin);
    }

    /// Set the address that must co-sign every clawback.
    pub fn set_clawback_cosigner(e: &Env, admin: Address, cosigner: Address) {
        admin::set_clawback_cosigner(e, &admin, &cosigner);
    }

    /// The configured clawback co-signer, if any.
    pub fn read_clawback_cosigner(e: &Env) -> Option<Address> {
        admin::read_clawback_cosigner(e)
    }
}
