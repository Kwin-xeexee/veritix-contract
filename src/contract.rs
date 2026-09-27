use crate::escrow;
use crate::metadata::{self, TokenMetadata};
use crate::storage_types::DataKey;
use crate::{admin, allowance, balance};
use soroban_sdk::{contract, contractimpl, Address, Env, String, Vec};

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
    /// Kept separate from [`allowance`] because the amount alone cannot
    /// distinguish "expired at ledger 500" from "never granted" — both read `0`.
    pub fn allowance_expiration(e: Env, from: Address, spender: Address) -> u32 {
        allowance::allowance_expiration(&e, &from, &spender)
    }

    /// Every spender `from` has granted an allowance to, in grant order.
    ///
    /// A lapsed grant stays in the index until it is revoked or overwritten, so
    /// this is a superset of the currently spendable pairs: call
    /// [`allowance`] per spender to find out which are still live. The
    /// superset is deliberate — the index exists so nothing granted can hide
    /// from a bulk revocation, and a grant that quietly dropped out of the list
    /// on expiry is the one a user auditing their approvals would never think
    /// to look for.
    pub fn get_allowances_for_spender(e: Env, from: Address) -> Vec<Address> {
        allowance::spenders_for(&e, &from)
    }

    /// Authorizes `spender` to move up to `amount` of `from`'s tokens until
    /// `expiration_ledger`, overwriting any previous grant to the same spender.
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

    /// Clears every approval `from` has granted and returns how many were
    /// revoked.
    pub fn revoke_all_allowances(e: Env, from: Address) -> u32 {
        from.require_auth();
        allowance::revoke_all_allowances(&e, &from)
    }

    /// Moves `amount` of `from`'s tokens to `to` on `spender`'s authority,
    /// drawing the amount down from their allowance.
    pub fn transfer_from(
        e: Env,
        spender: Address,
        from: Address,
        to: Address,
        amount: i128,
    ) {
        spender.require_auth();
        balance::transfer_from(&e, &spender, &from, &to, amount);
    }

    // ---- Escrow ---------------------------------------------------------

    /// Holds `amount` of `token` for `beneficiary` until the event settles, and
    /// returns the new escrow's id.
    ///
    /// The funds leave the depositor immediately and sit in the contract, so a
    /// buyer cannot walk away after a ticket is sold.
    pub fn create_escrow(
        e: Env,
        depositor: Address,
        beneficiary: Address,
        token: Address,
        amount: i128,
        deadline_ledger: u32,
    ) -> u32 {
        escrow::create(&e, &depositor, &beneficiary, &token, amount, deadline_ledger)
    }
}

/// The shared test harness, so `allowance_test` can drive the same contract the
/// tests below do rather than standing up a second one.
#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use crate::storage_types::EscrowRecord;
    use crate::storage_types::EscrowStatus;
    use soroban_sdk::token::StellarAssetClient;
    use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _};
    use soroban_sdk::{vec, xdr, FromVal, IntoVal};

    pub struct Fixture {
        pub e: Env,
        pub client: VeriTixPayClient<'static>,
        pub contract_id: Address,
        pub contract: Address,
        pub admin: Address,
    }

    impl Fixture {
        pub fn new() -> Self {
            let e = Env::default();
            e.mock_all_auths();
            let contract_id = e.register_contract(None, VeriTixPay);
            let client = VeriTixPayClient::new(&e, &contract_id);
            let admin = Address::generate(&e);
            client.initialize(&admin);
            let contract = contract_id.clone();
            Fixture {
                e,
                client,
                contract_id,
                contract,
                admin,
            }
        }

        pub fn fund(&self, amount: i128) -> Address {
            let holder = Address::generate(&self.e);
            self.client.mint(&self.admin, &holder, &amount);
            holder
        }

        /// A SEP-41 asset with `holder` holding `amount` of it.
        pub fn asset(
            &self,
            holder: &Address,
            amount: i128,
        ) -> (Address, StellarAssetClient<'static>) {
            let address = self
                .e
                .register_stellar_asset_contract_v2(self.admin.clone())
                .address();
            let token = StellarAssetClient::new(&self.e, &address);
            token.mint(holder, &amount);
            (address, token)
        }

        pub fn spender_index(&self, owner: &Address) -> Vec<Address> {
            self.client.get_allowances_for_spender(owner)
        }

        /// One escrow record, read through the contract's own storage frame.
        pub fn escrow(&self, id: u32) -> EscrowRecord {
            self.e.as_contract(&self.contract_id, || crate::escrow::record(&self.e, id))
        }

        /// Whether an escrow record exists at `id`.
        pub fn has_escrow(&self, id: u32) -> bool {
            self.e.as_contract(&self.contract_id, || {
                self.e
                    .storage()
                    .persistent()
                    .has(&DataKey::EscrowRecord(id))
            })
        }

        /// The number of escrows the contract has created.
        pub fn escrow_count(&self) -> u32 {
            self.e
                .as_contract(&self.contract_id, || crate::escrow::count(&self.e))
        }

        /// The total token amount the contract reports as held in escrow.
        pub fn escrow_value_locked(&self) -> i128 {
            self.e
                .as_contract(&self.contract_id, || crate::escrow::value_locked(&self.e))
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
        body.data.clone().expect("an event with no data")
    }

    // ---- #879: the spender index ----------------------------------------

    #[test]
    fn test_the_index_starts_empty() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        assert!(f.spender_index(&holder).is_empty());
    }

    #[test]
    fn test_an_approval_indexes_the_spender() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client
            .approve(&holder, &spender, &500, &1_000_000);

        assert_eq!(f.spender_index(&holder), vec![&f.e, spender]);
    }

    #[test]
    fn test_reapproving_the_same_spender_does_not_duplicate_it() {
        // The index is walked by revoke_all_allowances, so a second entry would
        // make it report and rewrite the same spender twice.
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client.approve(&holder, &spender, &500, &1_000_000);
        f.client.approve(&holder, &spender, &200, &1_000_000);

        assert_eq!(f.spender_index(&holder), vec![&f.e, spender]);
    }

    #[test]
    fn test_several_spenders_are_indexed_in_grant_order() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let a = Address::generate(&f.e);
        let b = Address::generate(&f.e);
        let c = Address::generate(&f.e);
        f.client.approve(&holder, &a, &100, &1_000_000);
        f.client.approve(&holder, &b, &100, &1_000_000);
        f.client.approve(&holder, &c, &100, &1_000_000);

        assert_eq!(f.spender_index(&holder), vec![&f.e, a, b, c]);
    }

    #[test]
    fn test_exhausting_an_allowance_unindexes_the_spender() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let receiver = Address::generate(&f.e);
        let spender = Address::generate(&f.e);
        f.client.approve(&holder, &spender, &500, &1_000_000);
        f.client.transfer_from(&spender, &holder, &receiver, &500);

        assert_eq!(f.client.allowance(&holder, &spender), 0);
        assert!(f.spender_index(&holder).is_empty());
    }

    #[test]
    fn test_a_partial_spend_keeps_the_spender_indexed() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let receiver = Address::generate(&f.e);
        let spender = Address::generate(&f.e);
        f.client.approve(&holder, &spender, &500, &1_000_000);
        f.client.transfer_from(&spender, &holder, &receiver, &200);

        assert_eq!(f.client.allowance(&holder, &spender), 300);
        assert_eq!(f.spender_index(&holder), vec![&f.e, spender]);
    }

    #[test]
    fn test_reapproving_after_exhaustion_reindexes_the_spender() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let receiver = Address::generate(&f.e);
        let spender = Address::generate(&f.e);
        f.client.approve(&holder, &spender, &500, &1_000_000);
        f.client.transfer_from(&spender, &holder, &receiver, &500);
        assert!(f.spender_index(&holder).is_empty());

        f.client.approve(&holder, &spender, &100, &1_000_000);

        assert_eq!(f.spender_index(&holder), vec![&f.e, spender]);
    }

    #[test]
    fn test_revoking_one_spender_unindexes_only_that_spender() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let a = Address::generate(&f.e);
        let b = Address::generate(&f.e);
        f.client.approve(&holder, &a, &100, &1_000_000);
        f.client.approve(&holder, &b, &100, &1_000_000);

        f.client.approve(&holder, &a, &0, &0);

        assert_eq!(f.spender_index(&holder), vec![&f.e, b]);
    }

    #[test]
    fn test_the_index_is_per_owner() {
        let f = Fixture::new();
        let one = f.fund(1_000);
        let two = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client.approve(&one, &spender, &100, &1_000_000);
        f.client.approve(&two, &spender, &100, &1_000_000);

        assert_eq!(f.spender_index(&one), vec![&f.e, spender.clone()]);
        assert_eq!(f.spender_index(&two), vec![&f.e, spender]);
    }

    #[test]
    fn test_a_lapsed_grant_stays_in_the_index_until_revoked() {
        // The index is the list of everything ever granted and not yet
        // revoked. Keeping a lapsed grant on it is what guarantees a bulk
        // revocation can still find it.
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client.approve(&holder, &spender, &500, &1_000);
        f.e.ledger().set_sequence_number(1_001);

        assert_eq!(f.client.allowance(&holder, &spender), 0);
        assert_eq!(f.spender_index(&holder), vec![&f.e, spender]);
    }

    // ---- #880: revoke_all_allowances ------------------------------------

    #[test]
    fn test_revoke_all_clears_every_allowance() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let a = Address::generate(&f.e);
        let b = Address::generate(&f.e);
        f.client.approve(&holder, &a, &100, &1_000_000);
        f.client.approve(&holder, &b, &250, &1_000_000);

        let revoked = f.client.revoke_all_allowances(&holder);

        assert_eq!(revoked, 2);
        assert_eq!(f.client.allowance(&holder, &a), 0);
        assert_eq!(f.client.allowance(&holder, &b), 0);
        assert!(f.spender_index(&holder).is_empty());
    }

    #[test]
    fn test_revoke_all_on_an_owner_with_no_allowances_is_a_no_op() {
        let f = Fixture::new();
        let holder = f.fund(1_000);

        assert_eq!(f.client.revoke_all_allowances(&holder), 0);
        assert!(f.spender_index(&holder).is_empty());
    }

    #[test]
    fn test_a_revoked_spender_cannot_move_tokens() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let receiver = Address::generate(&f.e);
        let spender = Address::generate(&f.e);
        f.client.approve(&holder, &spender, &500, &1_000_000);
        f.client.revoke_all_allowances(&holder);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f.client.transfer_from(&spender, &holder, &receiver, &100);
        }));

        assert!(result.is_err());
        assert_eq!(f.client.balance(&holder), 1_000);
        assert_eq!(f.client.balance(&receiver), 0);
    }

    #[test]
    fn test_revoke_all_leaves_other_owners_alone() {
        let f = Fixture::new();
        let one = f.fund(1_000);
        let two = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client.approve(&one, &spender, &100, &1_000_000);
        f.client.approve(&two, &spender, &100, &1_000_000);

        assert_eq!(f.client.revoke_all_allowances(&one), 1);

        assert_eq!(f.client.allowance(&one, &spender), 0);
        assert_eq!(f.client.allowance(&two, &spender), 100);
    }

    #[test]
    fn test_revoke_all_also_clears_lapsed_grants() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client.approve(&holder, &spender, &500, &1_000);
        f.e.ledger().set_sequence_number(1_001);

        assert_eq!(f.client.revoke_all_allowances(&holder), 1);

        assert!(f.spender_index(&holder).is_empty());
    }

    #[test]
    fn test_a_spender_can_be_granted_again_after_a_bulk_revoke() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let receiver = Address::generate(&f.e);
        let spender = Address::generate(&f.e);
        f.client.approve(&holder, &spender, &500, &1_000_000);
        f.client.revoke_all_allowances(&holder);

        f.client.approve(&holder, &spender, &500, &1_000_000);
        f.client
            .transfer_from(&spender, &holder, &receiver, &200);

        assert_eq!(f.client.allowance(&holder, &spender), 300);
    }

    #[test]
    fn test_revoke_all_emits_one_event_naming_the_owner_and_the_count() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client.approve(&holder, &spender, &100, &1_000_000);

        f.client.revoke_all_allowances(&holder);

        assert_eq!(
            last_topics(&f.e),
            std::vec![
                xdr::ScVal::Symbol("allowances_revoked".try_into().unwrap()),
                xdr::ScVal::from_val(&f.e, &holder.to_val()),
            ]
        );
        assert_eq!(
            last_data(&f.e),
            xdr::ScVal::from_val(&f.e, &vec![&f.e, 1u32.to_val()])
        );
    }

    #[test]
    fn test_revoke_all_is_idempotent() {
        let f = Fixture::new();
        let holder = f.fund(1_000);
        let spender = Address::generate(&f.e);
        f.client.approve(&holder, &spender, &100, &1_000_000);

        assert_eq!(f.client.revoke_all_allowances(&holder), 1);
        assert_eq!(f.client.revoke_all_allowances(&holder), 0);
        assert!(f.spender_index(&holder).is_empty());
    }

    // ---- #882: create_escrow -------------------------------------------

    #[test]
    fn test_create_escrow_returns_the_first_id() {
        let f = Fixture::new();
        let depositor = Address::generate(&f.e);
        let beneficiary = Address::generate(&f.e);
        let (token, _) = f.asset(&depositor, 1_000);

        let id = f
            .client
            .create_escrow(&depositor, &beneficiary, &token, &500, &2_000);

        assert_eq!(id, 0);
    }

    #[test]
    fn test_create_escrow_moves_the_tokens_to_the_contract() {
        let f = Fixture::new();
        let depositor = Address::generate(&f.e);
        let beneficiary = Address::generate(&f.e);
        let (token, asset) = f.asset(&depositor, 1_000);

        f.client
            .create_escrow(&depositor, &beneficiary, &token, &500, &2_000);

        assert_eq!(asset.balance(&depositor), 500);
        assert_eq!(asset.balance(&f.contract), 500);
        // The beneficiary is owed, not paid.
        assert_eq!(asset.balance(&beneficiary), 0);
    }

    #[test]
    fn test_create_escrow_stores_an_active_record() {
        let f = Fixture::new();
        let depositor = Address::generate(&f.e);
        let beneficiary = Address::generate(&f.e);
        let (token, _) = f.asset(&depositor, 1_000);

        let id = f
            .client
            .create_escrow(&depositor, &beneficiary, &token, &500, &2_000);

        let record = f.escrow(id);
        assert_eq!(record.depositor, depositor);
        assert_eq!(record.beneficiary, beneficiary);
        assert_eq!(record.token, token);
        assert_eq!(record.amount, 500);
        assert_eq!(record.deadline_ledger, 2_000);
        assert_eq!(record.status, EscrowStatus::Active);
    }

    #[test]
    fn test_escrow_ids_are_handed_out_in_order() {
        let f = Fixture::new();
        let depositor = Address::generate(&f.e);
        let beneficiary = Address::generate(&f.e);
        let (token, _) = f.asset(&depositor, 5_000);

        let first = f
            .client
            .create_escrow(&depositor, &beneficiary, &token, &100, &2_000);
        let second = f
            .client
            .create_escrow(&depositor, &beneficiary, &token, &200, &3_000);

        assert_eq!(first, 0);
        assert_eq!(second, 1);
    }

    #[test]
    fn test_create_escrow_accumulates_the_locked_value() {
        let f = Fixture::new();
        let depositor = Address::generate(&f.e);
        let beneficiary = Address::generate(&f.e);
        let (token, _) = f.asset(&depositor, 5_000);

        f.client
            .create_escrow(&depositor, &beneficiary, &token, &100, &2_000);
        f.client
            .create_escrow(&depositor, &beneficiary, &token, &250, &3_000);

        let locked = f.e.as_contract(&f.contract_id, || {
            escrow::value_locked(&f.e)
        });
        assert_eq!(locked, 350);
    }

    #[test]
    fn test_create_escrow_rejects_a_zero_amount() {
        let f = Fixture::new();
        let depositor = Address::generate(&f.e);
        let beneficiary = Address::generate(&f.e);
        let (token, _) = f.asset(&depositor, 1_000);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f.client
                .create_escrow(&depositor, &beneficiary, &token, &0, &2_000);
        }));

        assert!(result.is_err());
        assert_eq!(f.escrow_count(), 0);
        assert_eq!(f.escrow_value_locked(), 0);
    }

    #[test]
    fn test_create_escrow_rejects_a_negative_amount() {
        let f = Fixture::new();
        let depositor = Address::generate(&f.e);
        let beneficiary = Address::generate(&f.e);
        let (token, _) = f.asset(&depositor, 1_000);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f.client
                .create_escrow(&depositor, &beneficiary, &token, &-100, &2_000);
        }));

        assert!(result.is_err());
        assert_eq!(f.escrow_count(), 0);
    }

    #[test]
    fn test_create_escrow_fails_when_the_depositor_cannot_cover_it() {
        let f = Fixture::new();
        let depositor = Address::generate(&f.e);
        let beneficiary = Address::generate(&f.e);
        let (token, _) = f.asset(&depositor, 100);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f.client
                .create_escrow(&depositor, &beneficiary, &token, &500, &2_000);
        }));

        assert!(result.is_err());
        assert_eq!(f.escrow_count(), 0);
        assert_eq!(f.escrow_value_locked(), 0);
    }

    #[test]
    fn test_a_failed_escrow_records_nothing() {
        // The transfer runs before the record is written, so a token that
        // rejects the move cannot leave an escrow pointing at funds that never
        // arrived.
        let f = Fixture::new();
        let depositor = Address::generate(&f.e);
        let beneficiary = Address::generate(&f.e);
        let (token, _) = f.asset(&depositor, 100);

        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f.client
                .create_escrow(&depositor, &beneficiary, &token, &500, &2_000);
        }));

        assert!(!f.has_escrow(0));
    }

    #[test]
    fn test_create_escrow_emits_the_depositor_beneficiary_and_amount() {
        let f = Fixture::new();
        let depositor = Address::generate(&f.e);
        let beneficiary = Address::generate(&f.e);
        let (token, _) = f.asset(&depositor, 1_000);

        let id = f
            .client
            .create_escrow(&depositor, &beneficiary, &token, &500, &2_000);

        assert_eq!(
            last_topics(&f.e),
            std::vec![
                xdr::ScVal::Symbol("escrow_created".try_into().unwrap()),
                xdr::ScVal::from_val(&f.e, &depositor.to_val()),
                xdr::ScVal::from_val(&f.e, &beneficiary.to_val()),
            ]
        );
        assert_eq!(
            last_data(&f.e),
            xdr::ScVal::from_val(
                &f.e,
                &vec![
                    &f.e,
                    id.to_val(),
                    token.to_val(),
                    500i128.to_val(),
                    2_000u32.to_val(),
                ]
            )
        );
    }

    #[test]
    fn test_two_escrows_hold_funds_separately() {
        let f = Fixture::new();
        let one = Address::generate(&f.e);
        let two = Address::generate(&f.e);
        let beneficiary = Address::generate(&f.e);
        let (token, asset) = f.asset(&one, 1_000);
        asset.mint(&two, &1_000);

        f.client
            .create_escrow(&one, &beneficiary, &token, &300, &2_000);
        f.client
            .create_escrow(&two, &beneficiary, &token, &700, &3_000);

        assert_eq!(asset.balance(&f.contract), 1_000);
        assert_eq!(f.escrow_value_locked(), 1_000);
    }
}
