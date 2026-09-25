//! Tests for initialization, the admin authorization check, and the two-step
//! ownership transfer.

use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::{
    admin::{
        accept_admin, check_admin, initialize, is_initialized, require_initialized,
        transfer_ownership,
    },
    storage_types::DataKey,
};

/// An initialized contract with a known admin.
fn setup() -> (Env, Address, Address) {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let other = Address::generate(&e);
    initialize(e.clone(), admin.clone());
    (e, admin, other)
}

#[test]
fn initialize_sets_the_admin() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);

    assert!(!is_initialized(&e));
    initialize(e.clone(), admin.clone());

    assert!(is_initialized(&e));
    assert_eq!(
        e.storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::Admin),
        Some(admin)
    );
}

#[test]
#[should_panic(expected = "contract already initialized")]
fn initialize_twice_panics() {
    // The second call must not silently reassign control to whoever invoked
    // it last: a re-initializable contract has no admin at all.
    let e = Env::default();
    e.mock_all_auths();
    let first = Address::generate(&e);
    let second = Address::generate(&e);

    initialize(e.clone(), first);
    initialize(e, second);
}

#[test]
#[should_panic(expected = "contract already initialized")]
fn initialize_twice_keeps_the_first_admin() {
    let e = Env::default();
    e.mock_all_auths();
    let first = Address::generate(&e);
    let second = Address::generate(&e);

    initialize(e.clone(), first.clone());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        initialize(e.clone(), second)
    }));

    assert!(result.is_err());
    assert_eq!(
        e.storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::Admin),
        Some(first)
    );
}

#[test]
fn require_initialized_panics_before_initialization() {
    let e = Env::default();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| require_initialized(&e)));
    assert!(result.is_err());
}

#[test]
fn require_initialized_is_a_no_op_after_initialization() {
    let (e, _admin, _other) = setup();
    require_initialized(&e);
}

#[test]
fn check_admin_accepts_the_admin() {
    let (e, admin, _other) = setup();
    check_admin(&e, &admin);
}

#[test]
#[should_panic(expected = "caller is not the admin")]
fn check_admin_rejects_a_non_admin() {
    let (e, _admin, other) = setup();
    check_admin(&e, &other);
}

#[test]
#[should_panic(expected = "contract not initialized")]
fn check_admin_rejects_any_caller_before_initialization() {
    let e = Env::default();
    let caller = Address::generate(&e);
    check_admin(&e, &caller);
}

#[test]
fn transfer_ownership_records_a_proposal_without_changing_the_admin() {
    let (e, admin, other) = setup();

    transfer_ownership(&e, other.clone());

    // The admin is unchanged until the nominee accepts.
    assert_eq!(
        e.storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::Admin),
        Some(admin)
    );
    assert_eq!(
        e.storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::PendingAdmin),
        Some(other)
    );
    assert_eq!(
        e.storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::AdminActiveAfterLedger),
        Some(e.ledger().sequence())
    );
}

#[test]
#[should_panic(expected = "cannot transfer ownership to the current admin")]
fn transfer_ownership_rejects_the_current_admin() {
    let (e, admin, _other) = setup();
    transfer_ownership(&e, admin);
}

#[test]
#[should_panic(expected = "contract not initialized")]
fn transfer_ownership_before_initialization_panics() {
    let e = Env::default();
    e.mock_all_auths();
    let new_admin = Address::generate(&e);
    transfer_ownership(&e, new_admin);
}

#[test]
fn accept_admin_promotes_the_nominee_and_clears_the_proposal() {
    let (e, _admin, other) = setup();

    transfer_ownership(&e, other.clone());
    accept_admin(&e, other.clone());

    assert_eq!(
        e.storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::Admin),
        Some(other)
    );
    // A completed transfer must not be replayable.
    assert_eq!(
        e.storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::PendingAdmin),
        None
    );
    assert_eq!(
        e.storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::AdminActiveAfterLedger),
        Some(e.ledger().sequence())
    );
}

#[test]
#[should_panic(expected = "no pending admin transfer")]
fn accept_admin_without_a_proposal_panics() {
    let (e, _admin, other) = setup();
    accept_admin(&e, other);
}

#[test]
#[should_panic(expected = "caller is not the proposed admin")]
fn accept_admin_rejects_an_address_that_was_not_proposed() {
    let (e, _admin, other) = setup();
    let stranger = Address::generate(&e);

    transfer_ownership(&e, other);
    accept_admin(&e, stranger);
}

#[test]
fn a_mistyped_transfer_leaves_the_admin_in_control() {
    // The whole point of the two-step transfer: a proposal to an address
    // nobody can sign never completes, and the existing admin keeps control.
    let (e, admin, other) = setup();
    let mistyped = Address::generate(&e);

    transfer_ownership(&e, mistyped);
    // The nominee never accepts.
    assert_eq!(
        e.storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::Admin),
        Some(admin)
    );

    // The admin can propose again, and the real nominee can then take over.
    transfer_ownership(&e, other.clone());
    accept_admin(&e, other.clone());
    assert_eq!(
        e.storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::Admin),
        Some(other)
    );
}
