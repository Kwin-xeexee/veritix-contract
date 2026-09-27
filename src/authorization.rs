use crate::storage_types::DataKey;
use soroban_sdk::{Address, Env};

/// Whether `account` is authorized to move tokens.
///
/// Accounts are authorized by default: a fresh deployment with no
/// `set_authorized` call behaves like an ordinary SEP-41 token, and
/// `set_authorized(account, false)` is how the admin revokes a specific
/// account. The reverse default would make every transfer fail until the admin
/// walked the holder set, which is a footgun rather than a safety property.
pub fn is_authorized(e: &Env, account: &Address) -> bool {
    e.storage()
        .persistent()
        .get(&DataKey::Authorized(account.clone()))
        .unwrap_or(true)
}

/// Panics when `account` is not authorized to move tokens.
pub fn require_authorized(e: &Env, account: &Address) {
    if !is_authorized(e, account) {
        panic!("NotAuthorized: account is not authorized to move tokens");
    }
}

/// Writes the authorization flag for `account`.
///
/// Only the false case is stored: an absent key already means authorized, and
/// storing `true` would add a write to the common revoke-then-restore cycle for
/// no observable gain.
pub fn set_authorized(e: &Env, account: &Address, authorized: bool) {
    if authorized {
        e.storage()
            .persistent()
            .remove(&DataKey::Authorized(account.clone()));
    } else {
        e.storage()
            .persistent()
            .set(&DataKey::Authorized(account.clone()), &false);
    }
}
