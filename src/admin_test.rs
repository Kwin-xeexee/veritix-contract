//! Tests for initialization, the admin authorization check, the two-step
//! ownership transfer, the rotation views, the clawback co-signer, and the
//! capped initialization path.

use soroban_sdk::{
    testutils::{Address as _, MockAuth, MockAuthInvoke, Register},
    Address, Env, Vec,
};

use crate::{
    admin::{
        accept_admin, admin_active_after_ledger, check_admin, current_admin, initialize,
        initialize_with_max_supply, is_initialized, max_supply, pending_admin,
        read_clawback_cosigner, require_initialized, set_clawback_cosigner, transfer_ownership,
    },
    contract::VeritixToken,
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

// ---------------------------------------------------------------------------
// #855 — rotation views
// ---------------------------------------------------------------------------

#[test]
fn pending_admin_is_none_when_no_rotation_is_in_flight() {
    let (e, _admin, _other) = setup();
    assert_eq!(pending_admin(&e), None);
}

#[test]
fn pending_admin_exposes_an_in_flight_rotation() {
    let (e, _admin, other) = setup();

    transfer_ownership(&e, other.clone());

    // The whole reason for the view: an operator with no admin key can still
    // see that a rotation is pending and to whom.
    assert_eq!(pending_admin(&e), Some(other));
    assert_eq!(admin_active_after_ledger(&e), e.ledger().sequence());
}

#[test]
fn admin_active_after_ledger_is_zero_before_any_rotation() {
    let e = Env::default();
    e.mock_all_auths();
    initialize(e.clone(), Address::generate(&e));

    assert_eq!(admin_active_after_ledger(&e), 0);
}

#[test]
fn pending_admin_clears_once_the_rotation_completes() {
    let (e, _admin, other) = setup();

    transfer_ownership(&e, other.clone());
    accept_admin(&e, other);

    assert_eq!(pending_admin(&e), None);
    assert_eq!(admin_active_after_ledger(&e), e.ledger().sequence());
}

// ---------------------------------------------------------------------------
// #857 — rotation authorization edge cases
// ---------------------------------------------------------------------------

/// Authorize exactly these addresses for one entry point, and nothing else.
///
/// `mock_all_auths` would defeat the point of these tests: it signs for
/// everyone, so every authorization check trivially passes and the test proves
/// nothing. Here the mock list *is* the claim being tested.
///
/// Built on fixed-size arrays and `Default` rather than `Vec`, because this is a
/// `no_std` crate that deliberately does not use `alloc`.
fn mock_signers<const N: usize>(
    e: &Env,
    contract: &Address,
    fn_name: &str,
    signers: [&Address; N],
) {
    let invocations: [MockAuthInvoke; N] = core::array::from_fn(|_| MockAuthInvoke {
        contract,
        fn_name,
        args: Vec::new(e),
        sub_invokes: &[],
    });
    let auths: [MockAuth; N] = core::array::from_fn(|i| MockAuth {
        address: signers[i],
        invoke: &invocations[i],
    });
    e.mock_auths(&auths);
}

#[test]
fn a_non_admin_cannot_propose_a_rotation() {
    let e = Env::default();
    let contract = VeritixToken.register(&e, None, ());
    let admin = Address::generate(&e);
    let non_admin = Address::generate(&e);
    let nominee = Address::generate(&e);
    initialize(e.clone(), admin);

    // Only the non-admin signs. The admin's own signature is the missing one.
    mock_signers(&e, &contract, "transfer_ownership", [&non_admin]);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        transfer_ownership(&e, nominee)
    }));

    assert!(result.is_err(), "a non-admin must not be able to propose");
    assert_eq!(
        pending_admin(&e),
        None,
        "the failed proposal must not leave a pending admin behind"
    );
}

#[test]
fn the_admin_can_propose_with_only_its_own_signature() {
    let e = Env::default();
    let contract = VeritixToken.register(&e, None, ());
    let admin = Address::generate(&e);
    let nominee = Address::generate(&e);
    initialize(e.clone(), admin.clone());

    mock_signers(&e, &contract, "transfer_ownership", [&admin]);
    transfer_ownership(&e, nominee.clone());

    assert_eq!(pending_admin(&e), Some(nominee));
}

#[test]
fn an_address_other_than_the_proposed_one_cannot_accept() {
    let e = Env::default();
    let contract = VeritixToken.register(&e, None, ());
    let admin = Address::generate(&e);
    let nominee = Address::generate(&e);
    let interloper = Address::generate(&e);
    initialize(e.clone(), admin.clone());
    transfer_ownership(&e, nominee.clone());

    // The interloper signs; the nominee does not.
    mock_signers(&e, &contract, "accept_admin", [&interloper]);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        accept_admin(&e, interloper)
    }));

    assert!(result.is_err(), "only the nominee may accept");
    // Control has not moved, and the proposal is still live for the real nominee.
    assert_eq!(current_admin(&e), admin);
    assert_eq!(pending_admin(&e), Some(nominee));
}

#[test]
fn the_old_admin_loses_power_immediately_after_acceptance() {
    let e = Env::default();
    let contract = VeritixToken.register(&e, None, ());
    let old_admin = Address::generate(&e);
    let new_admin = Address::generate(&e);
    initialize(e.clone(), old_admin.clone());
    transfer_ownership(&e, new_admin.clone());
    accept_admin(&e, new_admin.clone());

    // The old admin still signs, and is still rejected: authority follows the
    // accepted nomination, it is not merely stale but gone.
    mock_signers(&e, &contract, "transfer_ownership", [&old_admin]);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        transfer_ownership(&e, Address::generate(&e))
    }));
    assert!(result.is_err(), "the old admin must lose power immediately");

    let guard =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check_admin(&e, &old_admin)));
    assert!(guard.is_err());
    assert_eq!(current_admin(&e), new_admin);
}

#[test]
fn the_new_admin_can_act_immediately_after_acceptance() {
    let e = Env::default();
    let contract = VeritixToken.register(&e, None, ());
    let old_admin = Address::generate(&e);
    let new_admin = Address::generate(&e);
    let third = Address::generate(&e);
    initialize(e.clone(), old_admin);
    transfer_ownership(&e, new_admin.clone());
    accept_admin(&e, new_admin.clone());

    mock_signers(&e, &contract, "transfer_ownership", [&new_admin]);
    transfer_ownership(&e, third.clone());

    assert_eq!(pending_admin(&e), Some(third));
}

// ---------------------------------------------------------------------------
// #856 — clawback co-signer
// ---------------------------------------------------------------------------

#[test]
fn no_cosigner_is_configured_by_default() {
    let (e, _admin, _other) = setup();
    assert_eq!(read_clawback_cosigner(&e), None);
}

#[test]
fn the_admin_can_set_a_cosigner() {
    let (e, admin, other) = setup();

    set_clawback_cosigner(&e, &admin, &other);

    assert_eq!(read_clawback_cosigner(&e), Some(other));
}

#[test]
fn a_cosigner_equal_to_the_admin_is_rejected() {
    let (e, admin, _other) = setup();

    // Dual authorization is meant to raise the bar to two keys. A co-signer
    // equal to the admin would satisfy it with one.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        set_clawback_cosigner(&e, &admin, &admin)
    }));

    assert!(result.is_err());
    assert_eq!(read_clawback_cosigner(&e), None);
}

#[test]
#[should_panic(expected = "caller is not the admin")]
fn a_non_admin_cannot_set_the_cosigner() {
    let (e, _admin, other) = setup();
    let interloper = Address::generate(&e);

    set_clawback_cosigner(&e, &interloper, &other);
}

#[test]
fn setting_the_cosigner_replaces_the_previous_one() {
    let (e, admin, first) = setup();
    let second = Address::generate(&e);

    set_clawback_cosigner(&e, &admin, &first);
    set_clawback_cosigner(&e, &admin, &second);

    assert_eq!(read_clawback_cosigner(&e), Some(second));
}

#[test]
fn the_cosigner_does_not_share_admin_powers() {
    // Being able to block clawback is not the same as being able to use it.
    let (e, _admin, cosigner) = setup();
    set_clawback_cosigner(&e, &current_admin(&e), &cosigner);

    let guard =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check_admin(&e, &cosigner)));

    assert!(guard.is_err());
}

// ---------------------------------------------------------------------------
// #858 — capped initialization
// ---------------------------------------------------------------------------
#[test]
fn initialize_with_max_supply_sets_admin_and_cap() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);

    initialize_with_max_supply(e.clone(), admin.clone(), 1_000_000);

    assert!(is_initialized(&e));
    assert_eq!(current_admin(&e), admin);
    assert_eq!(max_supply(&e), Some(1_000_000));
}

#[test]
fn a_plain_initialization_has_no_cap() {
    let (e, _admin, _other) = setup();
    assert_eq!(max_supply(&e), None);
}

#[test]
#[should_panic(expected = "max supply must be positive")]
fn a_zero_cap_is_rejected() {
    let e = Env::default();
    e.mock_all_auths();
    initialize_with_max_supply(e.clone(), Address::generate(&e), 0);
}

#[test]
#[should_panic(expected = "max supply must be positive")]
fn a_negative_cap_is_rejected() {
    let e = Env::default();
    e.mock_all_auths();
    initialize_with_max_supply(e.clone(), Address::generate(&e), -1);
}

#[test]
fn a_rejected_cap_leaves_the_contract_uninitialized() {
    let e = Env::default();
    e.mock_all_auths();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        initialize_with_max_supply(e.clone(), Address::generate(&e), 0)
    }));

    assert!(result.is_err());
    // A failed capped init must not consume the one-shot initialization.
    assert!(!is_initialized(&e));
    initialize_with_max_supply(e.clone(), Address::generate(&e), 500);
    assert_eq!(max_supply(&e), Some(500));
}

#[test]
#[should_panic(expected = "contract already initialized")]
fn a_capped_init_cannot_rerun_a_plain_init() {
    let e = Env::default();
    e.mock_all_auths();

    initialize_with_max_supply(e.clone(), Address::generate(&e), 1_000);
    initialize(e.clone(), Address::generate(&e));
}

#[test]
#[should_panic(expected = "contract already initialized")]
fn a_plain_init_cannot_rerun_a_capped_init() {
    let e = Env::default();
    e.mock_all_auths();

    initialize(e.clone(), Address::generate(&e));
    initialize_with_max_supply(e.clone(), Address::generate(&e), 1_000);
}
