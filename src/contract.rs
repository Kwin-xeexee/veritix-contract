use crate::metadata::{self, TokenMetadata};
use crate::storage_types::DataKey;
use crate::{admin, allowance, balance};
use soroban_sdk::{contract, contractimpl, Address, Env, String};

#[contract]
pub struct VeriTixPay;

/// Stores the admin and the token metadata, refusing a second call.
fn initialize_state(e: &Env, admin_addr: &Address, meta: &TokenMetadata) {
    if admin::is_initialized(e) {
        panic!("AlreadyInitialized: contract state is locked");
    }
    admin_addr.require_auth();
    e.storage().persistent().set(&DataKey::Admin, admin_addr);
    e.storage()
        .persistent()
        .set(&DataKey::InitializedAtLedger, &e.ledger().sequence());
    metadata::store(e, meta);
}

#[contractimpl]
impl VeriTixPay {
    /// Sets the admin and the default token metadata.
    ///
    /// One-shot: a second call is rejected rather than silently replacing the
    /// admin, because the admin is the root of trust for every privileged entry
    /// point below.
    pub fn initialize(e: Env, admin_addr: Address) {
        initialize_state(&e, &admin_addr, &metadata::defaults(&e));
    }

    /// Sets the admin and caller-supplied token metadata.
    pub fn initialize_with_metadata(
        e: Env,
        admin_addr: Address,
        name: String,
        symbol: String,
        decimals: u32,
    ) {
        initialize_state(
            &e,
            &admin_addr,
            &TokenMetadata {
                name,
                symbol,
                decimals,
            },
        );
    }

    /// True once an admin has been stored.
    pub fn is_initialized(e: Env) -> bool {
        admin::is_initialized(&e)
    }

    /// The address currently holding admin authority.
    pub fn admin(e: Env) -> Address {
        admin::admin(&e)
    }

    /// Ledger on which the contract was initialized, 0 when it never was.
    pub fn initialized_at_ledger(e: Env) -> u32 {
        admin::initialized_at_ledger(&e)
    }

    /// The token name.
    pub fn name(e: Env) -> String {
        metadata::load(&e).name
    }

    /// The token symbol.
    pub fn symbol(e: Env) -> String {
        metadata::load(&e).symbol
    }

    /// The number of decimals the token is scaled by.
    pub fn decimals(e: Env) -> u32 {
        metadata::load(&e).decimals
    }

    /// Tokens held by `account`; 0 for an address that has never been credited.
    pub fn balance(e: Env, account: Address) -> i128 {
        balance::balance_of(&e, &account)
    }

    /// Tokens in circulation.
    pub fn total_supply(e: Env) -> i128 {
        balance::total_supply(&e)
    }

    /// The hard cap on total supply, or 0 when supply is unlimited.
    pub fn max_supply(e: Env) -> i128 {
        balance::max_supply(&e)
    }

    /// Mints `amount` new tokens to `to`. Admin-only and supply-capped.
    pub fn mint(e: Env, admin_addr: Address, to: Address, amount: i128) {
        admin::check_admin(&e, &admin_addr);
        balance::mint(&e, &to, amount);
    }

    /// Moves `amount` of tokens from `from` to `to`.
    pub fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        balance::transfer(&e, &from, &to, amount);
    }

    /// Destroys `amount` of the caller's own tokens.
    pub fn burn(e: Env, from: Address, amount: i128) {
        from.require_auth();
        balance::burn(&e, &from, amount);
    }

    // ---- Allowances -----------------------------------------------------

    /// The amount `spender` may still move on `from`'s behalf.
    ///
    /// Reads `0` once the expiration ledger has passed, so a UI polling this
    /// view gets a number it can act on rather than a failure to special-case.
    pub fn allowance(e: Env, from: Address, spender: Address) -> i128 {
        allowance::allowance(&e, &from, &spender)
    }

    /// The ledger at which the `from`/`spender` allowance expires.
    ///
    /// Reported even after the allowance has lapsed, so a caller can tell
    /// "expired at ledger 500" apart from "never granted"; the amount alone
    /// reads as `0` for both.
    pub fn allowance_expiration(e: Env, from: Address, spender: Address) -> u32 {
        allowance::allowance_expiration(&e, &from, &spender)
    }

    /// Authorizes `spender` to move up to `amount` of `from`'s tokens until
    /// `expiration_ledger`, replacing any existing allowance.
    pub fn approve(
        e: Env,
        from: Address,
        spender: Address,
        amount: i128,
        expiration_ledger: u32,
    ) {
        from.require_auth();
        allowance::approve(&e, &from, &spender, amount, expiration_ledger);
    }

    /// Raises the `from`/`spender` allowance by `amount`, keeping its
    /// expiration.
    ///
    /// Adjusting by a delta rather than overwriting is what avoids the
    /// re-approval race: two clients changing the same grant at once cannot
    /// lose an update, because neither has to read the current value and write
    /// it back.
    pub fn increase_allowance(e: Env, from: Address, spender: Address, amount: i128) {
        from.require_auth();
        allowance::increase_allowance(&e, &from, &spender, amount);
    }

    /// Lowers the `from`/`spender` allowance by `amount`, keeping its
    /// expiration. Saturates at zero rather than underflowing.
    pub fn decrease_allowance(e: Env, from: Address, spender: Address, amount: i128) {
        from.require_auth();
        allowance::decrease_allowance(&e, &from, &spender, amount);
    }

    /// Moves `amount` from `from` to `to` on `spender`'s authority, reducing
    /// the spender's allowance.
    ///
    /// This is how a marketplace moves a buyer's tokens: the buyer signs one
    /// `approve`, and the marketplace spends from it.
    pub fn transfer_from(e: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();
        balance::transfer_from(&e, &spender, &from, &to, amount);
    }

    /// Destroys `amount` of `from`'s tokens on `spender`'s authority, reducing
    /// the spender's allowance.
    ///
    /// For settlement flows that burn a user's tokens against a prior approval.
    pub fn burn_from(e: Env, spender: Address, from: Address, amount: i128) {
        spender.require_auth();
        balance::burn_from(&e, &spender, &from, amount);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _};
    use soroban_sdk::{symbol_short, vec, xdr, FromVal};

    struct Fixture {
        e: Env,
        client: VeriTixPayClient<'static>,
        contract_id: Address,
        admin: Address,
    }

    impl Fixture {
        fn new() -> Self {
            let e = Env::default();
            e.mock_all_auths();
            let contract_id = e.register_contract(None, VeriTixPay);
            let client = VeriTixPayClient::new(&e, &contract_id);
            let admin = Address::generate(&e);
            client.initialize(&admin);
            Fixture {
                e,
                client,
                contract_id,
                admin,
            }
        }

        fn fund(&self, amount: i128) -> Address {
            let holder = Address::generate(&self.e);
            self.client.mint(&self.admin, &holder, &amount);
            holder
        }

        fn set_max_supply(&self, cap: i128) {
            self.e.as_contract(&self.contract_id, || {
                self.e.storage().persistent().set(&DataKey::MaxSupply, &cap);
            });
        }
    }

    fn last_topics(e: &Env) -> std::vec::Vec<xdr::ScVal> {
        let events = e.events().all();
        let last = events.events().last().expect("no event was emitted");
        let xdr::ContractEventBody::V0(body) = &last.body else {
            panic!("expected a v0 contract event");
        };
        body.topics.clone()
    }

    fn last_data(e: &Env) -> xdr::ScVal {
        let events = e.events().all();
        let last = events.events().last().expect("no event was emitted");
        let xdr::ContractEventBody::V0(body) = &last.body else {
            panic!("expected a v0 contract event");
        };
        body.data.clone()
    }

    // ---- Initialization -------------------------------------------------

    #[test]
    fn test_initialize_stores_admin_and_defaults() {
        let f = Fixture::new();
        assert!(f.client.is_initialized());
        assert_eq!(f.client.admin(), f.admin);
        assert_eq!(f.client.name(), String::from_str(&f.e, "VeriTix"));
        assert_eq!(f.client.symbol(), String::from_str(&f.e, "VTX"));
        assert_eq!(f.client.decimals(), 7);
    }

    #[test]
    #[should_panic(expected = "AlreadyInitialized")]
    fn test_initialize_twice_panics() {
        let f = Fixture::new();
        f.client.initialize(&Address::generate(&f.e));
    }

    // ---- Allowance views ------------------------------------------------

    #[test]
    fn test_allowance_is_zero_before_any_approval() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        assert_eq!(f.client.allowance(&holder, &spender), 0);
    }

    #[test]
    fn test_approve_is_reflected_in_both_views() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        let expiry = f.e.ledger().sequence() + 1_000;

        f.client.approve(&holder, &spender, &500, &expiry);

        assert_eq!(f.client.allowance(&holder, &spender), 500);
        assert_eq!(f.client.allowance_expiration(&holder, &spender), expiry);
    }

    #[test]
    fn test_approve_overwrites_rather_than_accumulates() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        let expiry = f.e.ledger().sequence() + 1_000;

        f.client.approve(&holder, &spender, &500, &expiry);
        f.client.approve(&holder, &spender, &200, &expiry);

        assert_eq!(f.client.allowance(&holder, &spender), 200);
    }

    #[test]
    fn test_allowance_reads_zero_once_expired() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client.approve(&holder, &spender, &500, &1_000);

        f.e.ledger().set_sequence_number(1_000);
        assert_eq!(f.client.allowance(&holder, &spender), 500);

        f.e.ledger().set_sequence_number(1_001);
        assert_eq!(f.client.allowance(&holder, &spender), 0);
    }

    #[test]
    fn test_expiration_view_still_reports_after_lapse() {
        // The amount alone cannot distinguish "expired" from "never granted";
        // the expiration view is what makes the two tellable apart.
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client.approve(&holder, &spender, &500, &1_000);
        f.e.ledger().set_sequence_number(1_001);

        assert_eq!(f.client.allowance(&holder, &spender), 0);
        assert_eq!(f.client.allowance_expiration(&holder, &spender), 1_000);
        assert_eq!(f.client.allowance_expiration(&holder, &Address::generate(&f.e)), 0);
    }

    #[test]
    #[should_panic(expected = "ExpirationInPast")]
    fn test_approve_rejects_a_past_expiration_for_a_non_zero_amount() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.e.ledger().set_sequence_number(1_000);

        f.client.approve(&holder, &spender, &500, &999);
    }

    #[test]
    fn test_approve_zero_revokes_whatever_expiration_was_stored() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client
            .approve(&holder, &spender, &500, &(f.e.ledger().sequence() + 1_000));

        f.client.approve(&holder, &spender, &0, &0);

        assert_eq!(f.client.allowance(&holder, &spender), 0);
        assert_eq!(f.client.allowance_expiration(&holder, &spender), 0);
    }

    #[test]
    fn test_approve_emits_the_standard_event() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        let expiry = f.e.ledger().sequence() + 1_000;

        f.client.approve(&holder, &spender, &500, &expiry);

        assert_eq!(
            last_topics(&f.e),
            std::vec![
                xdr::ScVal::from_val(&f.e, &symbol_short!("approve").to_val()),
                xdr::ScVal::from_val(&f.e, &holder.to_val()),
                xdr::ScVal::from_val(&f.e, &spender.to_val()),
            ]
        );
        assert_eq!(
            last_data(&f.e),
            xdr::ScVal::from_val(&f.e, &vec![&f.e, 500i128, expiry].to_val())
        );
    }

    // ---- transfer_from --------------------------------------------------

    #[test]
    fn test_transfer_from_moves_the_balance_and_spends_the_allowance() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        let to = Address::generate(&f.e);
        f.client
            .approve(&holder, &spender, &500, &(f.e.ledger().sequence() + 1_000));

        f.client.transfer_from(&spender, &holder, &to, &200);

        assert_eq!(f.client.balance(&holder), 800);
        assert_eq!(f.client.balance(&to), 200);
        assert_eq!(f.client.allowance(&holder, &spender), 300);
        assert_eq!(f.client.total_supply(), 1_000);
    }

    #[test]
    fn test_transfer_from_can_be_called_repeatedly() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        let to = Address::generate(&f.e);
        f.client
            .approve(&holder, &spender, &500, &(f.e.ledger().sequence() + 1_000));

        f.client.transfer_from(&spender, &holder, &to, &200);
        f.client.transfer_from(&spender, &holder, &to, &200);

        assert_eq!(f.client.balance(&to), 400);
        assert_eq!(f.client.allowance(&holder, &spender), 100);
    }

    #[test]
    fn test_transfer_from_rejects_more_than_the_allowance() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        let to = Address::generate(&f.e);
        f.client
            .approve(&holder, &spender, &500, &(f.e.ledger().sequence() + 1_000));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f.client.transfer_from(&spender, &holder, &to, &501);
        }));

        assert!(result.is_err());
        assert_eq!(f.client.balance(&holder), 1_000);
        assert_eq!(f.client.allowance(&holder, &spender), 500);
    }

    #[test]
    fn test_transfer_from_rejects_more_than_the_balance() {
        let f = Fixture::new();
        let holder = f.fund(100);
        let spender = Address::generate(&f.e);
        let to = Address::generate(&f.e);
        f.client
            .approve(&holder, &spender, &500, &(f.e.ledger().sequence() + 1_000));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f.client.transfer_from(&spender, &holder, &to, &101);
        }));

        assert!(result.is_err());
        assert_eq!(f.client.balance(&holder), 100);
        assert_eq!(f.client.balance(&to), 0);
        assert_eq!(f.client.allowance(&holder, &spender), 500);
    }

    #[test]
    fn test_transfer_from_rejects_an_expired_allowance() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        let to = Address::generate(&f.e);
        f.client.approve(&holder, &spender, &500, &1_000);
        f.e.ledger().set_sequence_number(1_001);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f.client.transfer_from(&spender, &holder, &to, &100);
        }));

        assert!(result.is_err());
        assert_eq!(f.client.balance(&holder), 1_000);
        assert_eq!(f.client.balance(&to), 0);
    }

    #[test]
    fn test_transfer_from_needs_no_allowance() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        let to = Address::generate(&f.e);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f.client.transfer_from(&spender, &holder, &to, &1);
        }));

        assert!(result.is_err());
    }

    #[test]
    fn test_exhausted_allowance_is_revoked() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        let to = Address::generate(&f.e);
        let expiry = f.e.ledger().sequence() + 1_000;
        f.client.approve(&holder, &spender, &200, &expiry);

        f.client.transfer_from(&spender, &holder, &to, &200);

        assert_eq!(f.client.allowance(&holder, &spender), 0);
        assert_eq!(f.client.allowance_expiration(&holder, &spender), 0);
    }

    #[test]
    fn test_transfer_from_emits_the_standard_transfer_event() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        let to = Address::generate(&f.e);
        f.client
            .approve(&holder, &spender, &500, &(f.e.ledger().sequence() + 1_000));

        f.client.transfer_from(&spender, &holder, &to, &200);

        assert_eq!(
            last_topics(&f.e),
            std::vec![
                xdr::ScVal::from_val(&f.e, &symbol_short!("transfer").to_val()),
                xdr::ScVal::from_val(&f.e, &holder.to_val()),
                xdr::ScVal::from_val(&f.e, &to.to_val()),
            ]
        );
        assert_eq!(last_data(&f.e), xdr::ScVal::from_val(&f.e, &200i128.to_val()));
    }

    // ---- burn_from ------------------------------------------------------

    #[test]
    fn test_burn_from_reduces_balance_and_supply() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client
            .approve(&holder, &spender, &500, &(f.e.ledger().sequence() + 1_000));

        f.client.burn_from(&spender, &holder, &400);

        assert_eq!(f.client.balance(&holder), 600);
        assert_eq!(f.client.total_supply(), 600);
        assert_eq!(f.client.allowance(&holder, &spender), 100);
    }

    #[test]
    fn test_burn_from_rejects_more_than_the_allowance() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client
            .approve(&holder, &spender, &500, &(f.e.ledger().sequence() + 1_000));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f.client.burn_from(&spender, &holder, &501);
        }));

        assert!(result.is_err());
        assert_eq!(f.client.balance(&holder), 1_000);
        assert_eq!(f.client.total_supply(), 1_000);
        assert_eq!(f.client.allowance(&holder, &spender), 500);
    }

    #[test]
    fn test_burn_from_rejects_more_than_the_balance() {
        let f = Fixture::new();
        let holder = f.fund(100);
        let spender = Address::generate(&f.e);
        f.client
            .approve(&holder, &spender, &500, &(f.e.ledger().sequence() + 1_000));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f.client.burn_from(&spender, &holder, &101);
        }));

        assert!(result.is_err());
        assert_eq!(f.client.balance(&holder), 100);
        assert_eq!(f.client.total_supply(), 100);
    }

    #[test]
    fn test_burn_from_emits_the_standard_burn_event() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client
            .approve(&holder, &spender, &500, &(f.e.ledger().sequence() + 1_000));

        f.client.burn_from(&spender, &holder, &400);

        assert_eq!(
            last_topics(&f.e),
            std::vec![
                xdr::ScVal::from_val(&f.e, &symbol_short!("burn").to_val()),
                xdr::ScVal::from_val(&f.e, &holder.to_val()),
            ]
        );
        assert_eq!(last_data(&f.e), xdr::ScVal::from_val(&f.e, &400i128.to_val()));
    }

    // ---- increase_allowance / decrease_allowance ------------------------

    #[test]
    fn test_increase_allowance_preserves_the_expiration() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        let expiry = f.e.ledger().sequence() + 1_000;
        f.client.approve(&holder, &spender, &200, &expiry);

        f.client.increase_allowance(&holder, &spender, &300);

        assert_eq!(f.client.allowance(&holder, &spender), 500);
        assert_eq!(f.client.allowance_expiration(&holder, &spender), expiry);
    }

    #[test]
    fn test_increase_allowance_can_be_called_repeatedly() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client
            .approve(&holder, &spender, &100, &(f.e.ledger().sequence() + 1_000));

        f.client.increase_allowance(&holder, &spender, &50);
        f.client.increase_allowance(&holder, &spender, &25);

        assert_eq!(f.client.allowance(&holder, &spender), 175);
    }

    #[test]
    #[should_panic(expected = "NoAllowance")]
    fn test_increase_allowance_needs_an_existing_allowance() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);

        f.client.increase_allowance(&holder, &spender, &100);
    }

    #[test]
    fn test_decrease_allowance_preserves_the_expiration() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        let expiry = f.e.ledger().sequence() + 1_000;
        f.client.approve(&holder, &spender, &500, &expiry);

        f.client.decrease_allowance(&holder, &spender, &200);

        assert_eq!(f.client.allowance(&holder, &spender), 300);
        assert_eq!(f.client.allowance_expiration(&holder, &spender), expiry);
    }

    #[test]
    fn test_decrease_allowance_saturates_at_zero() {
        // Reducing by more than was granted must revoke the grant, not fail and
        // not wrap into a large positive number.
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client
            .approve(&holder, &spender, &200, &(f.e.ledger().sequence() + 1_000));

        f.client.decrease_allowance(&holder, &spender, &500);

        assert_eq!(f.client.allowance(&holder, &spender), 0);
    }

    #[test]
    fn test_a_saturated_decrease_revokes_the_entry_entirely() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client
            .approve(&holder, &spender, &200, &(f.e.ledger().sequence() + 1_000));

        f.client.decrease_allowance(&holder, &spender, &200);

        assert_eq!(f.client.allowance(&holder, &spender), 0);
        assert_eq!(f.client.allowance_expiration(&holder, &spender), 0);
    }

    #[test]
    fn test_a_decrease_to_zero_blocks_a_subsequent_spend() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        let to = Address::generate(&f.e);
        f.client
            .approve(&holder, &spender, &200, &(f.e.ledger().sequence() + 1_000));
        f.client.decrease_allowance(&holder, &spender, &200);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f.client.transfer_from(&spender, &holder, &to, &1);
        }));

        assert!(result.is_err());
        assert_eq!(f.client.balance(&to), 0);
    }

    #[test]
    fn test_decrease_allowance_on_an_absent_allowance_is_a_no_op() {
        // Pulling authority back is idempotent, so reducing something that is
        // already worth nothing succeeds rather than failing.
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);

        f.client.decrease_allowance(&holder, &spender, &100);

        assert_eq!(f.client.allowance(&holder, &spender), 0);
    }

    #[test]
    #[should_panic(expected = "InvalidAmount")]
    fn test_increase_allowance_rejects_a_zero_delta() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client
            .approve(&holder, &spender, &200, &(f.e.ledger().sequence() + 1_000));

        f.client.increase_allowance(&holder, &spender, &0);
    }

    #[test]
    #[should_panic(expected = "InvalidAmount")]
    fn test_decrease_allowance_rejects_a_negative_delta() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client
            .approve(&holder, &spender, &200, &(f.e.ledger().sequence() + 1_000));

        f.client.decrease_allowance(&holder, &spender, &(-1));
    }

    #[test]
    fn test_adjustments_emit_the_approve_event_with_the_new_total() {
        // The event carries the resulting allowance, not the delta, so an
        // indexer replaying the log reconstructs the grant without doing
        // arithmetic of its own.
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        let expiry = f.e.ledger().sequence() + 1_000;
        f.client.approve(&holder, &spender, &200, &expiry);

        f.client.increase_allowance(&holder, &spender, &300);

        assert_eq!(
            last_topics(&f.e),
            std::vec![
                xdr::ScVal::from_val(&f.e, &symbol_short!("approve").to_val()),
                xdr::ScVal::from_val(&f.e, &holder.to_val()),
                xdr::ScVal::from_val(&f.e, &spender.to_val()),
            ]
        );
        assert_eq!(
            last_data(&f.e),
            xdr::ScVal::from_val(&f.e, &vec![&f.e, 500i128, expiry].to_val())
        );
    }

    #[test]
    fn test_a_decreased_allowance_is_respected_by_a_spend() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        let to = Address::generate(&f.e);
        f.client
            .approve(&holder, &spender, &500, &(f.e.ledger().sequence() + 1_000));
        f.client.decrease_allowance(&holder, &spender, &400);

        f.client.transfer_from(&spender, &holder, &to, &100);

        assert_eq!(f.client.balance(&to), 100);
        assert_eq!(f.client.allowance(&holder, &spender), 0);
    }

    #[test]
    fn test_an_increased_allowance_permits_a_larger_spend() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        let to = Address::generate(&f.e);
        f.client
            .approve(&holder, &spender, &100, &(f.e.ledger().sequence() + 1_000));
        f.client.increase_allowance(&holder, &spender, &400);

        f.client.transfer_from(&spender, &holder, &to, &500);

        assert_eq!(f.client.balance(&to), 500);
        assert_eq!(f.client.allowance(&holder, &spender), 0);
    }

    #[test]
    fn test_allowances_are_per_pair() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let alice = Address::generate(&f.e);
        let bob = Address::generate(&f.e);
        let expiry = f.e.ledger().sequence() + 1_000;
        f.client.approve(&holder, &alice, &200, &expiry);
        f.client.approve(&holder, &bob, &300, &expiry);

        f.client.decrease_allowance(&holder, &alice, &50);

        assert_eq!(f.client.allowance(&holder, &alice), 150);
        assert_eq!(f.client.allowance(&holder, &bob), 300);
    }

    #[test]
    fn test_an_expired_allowance_cannot_be_increased() {
        // The stored entry still exists, but it is worth nothing, so there is
        // nothing to adjust and the caller must approve again.
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client.approve(&holder, &spender, &500, &1_000);
        f.e.ledger().set_sequence_number(1_001);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f.client.increase_allowance(&holder, &spender, &100);
        }));

        assert!(result.is_err());
        assert_eq!(f.client.allowance(&holder, &spender), 0);
    }

    #[test]
    fn test_decreasing_an_expired_allowance_is_a_no_op() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client.approve(&holder, &spender, &500, &1_000);
        f.e.ledger().set_sequence_number(1_001);

        f.client.decrease_allowance(&holder, &spender, &100);

        assert_eq!(f.client.allowance(&holder, &spender), 0);
    }

    // ---- Composition ----------------------------------------------------

    #[test]
    fn test_a_marketplace_settlement_flow() {
        let f = Fixture::new();
        let buyer = f.fund(1_000);
        let seller = Address::generate(&f.e);
        let marketplace = Address::generate(&f.e);
        let expiry = f.e.ledger().sequence() + 1_000;

        // The buyer signs one approval for the whole order.
        f.client.approve(&buyer, &marketplace, &300, &expiry);
        // The marketplace pays the seller, then burns its fee against the rest.
        f.client.transfer_from(&marketplace, &buyer, &seller, &250);
        f.client.burn_from(&marketplace, &buyer, &50);

        assert_eq!(f.client.balance(&buyer), 700);
        assert_eq!(f.client.balance(&seller), 250);
        assert_eq!(f.client.total_supply(), 950);
        assert_eq!(f.client.allowance(&buyer, &marketplace), 0);
    }

    #[test]
    fn test_the_supply_cap_holds_across_allowance_spends() {
        let f = Fixture::new();
        f.set_max_supply(1_000);
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client
            .approve(&holder, &spender, &1_000, &(f.e.ledger().sequence() + 1_000));

        f.client.burn_from(&spender, &holder, &400);

        assert_eq!(f.client.total_supply(), 600);
        assert_eq!(f.client.max_supply(), 1_000);
    }
}
