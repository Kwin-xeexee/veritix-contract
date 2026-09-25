use crate::metadata::{self, TokenMetadata};
use crate::storage_types::DataKey;
use crate::{admin, balance, control};
use crate::{admin, balance};
use soroban_sdk::{contract, contractimpl, Address, Env, String};

#[contract]
pub struct VeriTixPay;

/// Stores the admin, an optional co-signer, and the token metadata, refusing a
/// second call.
fn initialize_state(
    e: &Env,
    admin_addr: &Address,
    co_signer: &Option<Address>,
    meta: &TokenMetadata,
) {
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
    admin::store_co_signer(e, co_signer);
    metadata::store(e, meta);
}

#[contractimpl]
impl VeriTixPay {
    /// Sets the admin and the default token metadata.
    ///
    /// The admin address is the root of trust for every privileged entry point,
    /// so initialization is deliberately one-shot: a second call is rejected
    /// rather than silently replacing the admin.
    pub fn initialize(e: Env, admin_addr: Address) {
        initialize_state(&e, &admin_addr, &None, &metadata::defaults(&e));
    }

    /// Sets the admin, an optional clawback co-signer, and caller-supplied
    /// token metadata.
    ///
    /// Same one-shot rule as [`initialize`]; use this when a regulated
    /// deployment needs a second approval key on clawback, or a token name,
    /// symbol or precision other than the defaults.
    pub fn initialize_with_co_signer(
        e: Env,
        admin_addr: Address,
        co_signer: Option<Address>,
        initialize_state(&e, &admin_addr, &metadata::defaults(&e));
    }

    /// Sets the admin and caller-supplied token metadata.
    ///
    /// Same one-shot rule as [`initialize`]; use this when the deployment needs
    /// a token name, symbol or precision other than the defaults.
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
            &co_signer,
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

    /// The co-signer that must approve clawbacks, when one is configured.
    pub fn co_signer(e: Env) -> Option<Address> {
        admin::co_signer(&e)
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

    /// Tokens of `account` currently held in active escrows.
    pub fn escrow_locked(e: Env, account: Address) -> i128 {
        balance::escrow_locked(&e, &account)
    }

    /// Tokens `account` may move right now: the balance minus escrow locks, or
    /// 0 while the account is frozen.
    pub fn spendable_balance(e: Env, account: Address) -> i128 {
        balance::spendable_balance(&e, &account)
    }

    /// Tokens in circulation.
    pub fn total_supply(e: Env) -> i128 {
        balance::total_supply(&e)
    }

    /// Whether the contract is paused.
    pub fn is_paused(e: Env) -> bool {
        control::is_paused(&e)
    }

    /// Whether `account` is frozen.
    pub fn is_frozen(e: Env, account: Address) -> bool {
        control::is_frozen(&e, &account)
    }

    /// Mints `amount` new tokens to `to`.
    ///
    /// Admin-only and supply-capped. This is the prerequisite the rest of the
    /// token module is built on: without a way to bring supply into
    /// circulation there is nothing to transfer, burn or claw back.
    /// Mints `amount` new tokens to `to`.
    ///
    /// Admin-only and supply-capped: the cap is the first place a supply limit
    /// has to hold, so the check lives in the balance module next to the
    /// arithmetic it protects rather than being repeated by every caller.
    pub fn mint(e: Env, admin_addr: Address, to: Address, amount: i128) {
        admin::check_admin(&e, &admin_addr);
        balance::mint(&e, &to, amount);
    }

    /// Destroys `amount` of the caller's own tokens.
    ///
    /// Holder-authorized: `from` must sign the call. Burning reduces the balance
    /// and total supply together, so the two ledgers stay consistent.
    pub fn burn(e: Env, from: Address, amount: i128) {
        from.require_auth();
        balance::burn(&e, &from, amount);
    }

    /// Moves `amount` of tokens from `from` to `to`.
    ///
    /// The central token operation, and therefore the path every compliance
    /// control is enforced on: the contract must not be paused, and neither
    /// party may be frozen. `from` must sign the call.
    pub fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        balance::transfer(&e, &from, &to, amount);
    }

    /// Recovers `amount` from `from` on the admin's authority.
    ///
    /// For recovering tokens from sanctioned accounts. The admin signs, and the
    /// co-signer signs too when one is configured at initialization. The holder
    /// does not sign: that is the whole point of a clawback, which is why it
    /// emits its own event rather than reusing the burn event.
    pub fn clawback(e: Env, admin_addr: Address, from: Address, amount: i128) {
        admin::check_admin(&e, &admin_addr);
        admin::require_co_signer(&e);
        balance::clawback(&e, &admin_addr, &from, amount);
    }

    /// Freezes or thaws `account`. A frozen account keeps its balance but
    /// cannot send or receive tokens.
    pub fn set_frozen(e: Env, admin_addr: Address, account: Address, frozen: bool) {
        admin::check_admin(&e, &admin_addr);
        control::set_frozen(&e, &account, frozen);
    }

    /// Pauses or resumes the contract. While paused, no tokens move.
    pub fn set_paused(e: Env, admin_addr: Address, paused: bool) {
        admin::check_admin(&e, &admin_addr);
        control::set_paused(&e, paused);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Events as _};
    use soroban_sdk::{symbol_short, vec, xdr, FromVal};

    fn setup() -> (Env, VeriTixPayClient<'static>, Address) {
        let e = Env::default();
        e.mock_all_auths();
        let contract_id = e.register_contract(None, VeriTixPay);
        let client = VeriTixPayClient::new(&e, &contract_id);
        let admin = Address::generate(&e);
        client.initialize(&admin);
        (e, client, admin)
    }

    /// Mirrors the escrow module: escrowing records a lock against the holder
    /// without moving tokens. The escrow feature that will own this key is not
    /// part of this branch, so the test writes the aggregate directly.
    fn set_escrow_lock(e: &Env, contract_id: &Address, account: &Address, locked: i128) {
        e.as_contract(contract_id, || {
            e.storage()
                .persistent()
                .set(&DataKey::EscrowLocked(account.clone()), &locked);
        });
    }

    fn funded(client: &VeriTixPayClient<'static>, admin: &Address, holder: &Address, amount: i128) {
        client.mint(admin, holder, &amount);
    /// Installs a supply cap. Choosing the cap belongs to the initializer, so
    /// the test writes the storage key directly instead of adding a setter that
    /// no issue has asked for.
    fn install_cap(e: &Env, contract_id: &Address, cap: i128) {
        e.as_contract(contract_id, || {
            e.storage().persistent().set(&DataKey::MaxSupply, &cap);
        });
    }

    fn setup_capped(cap: i128) -> (Env, VeriTixPayClient<'static>, Address) {
        let e = Env::default();
        e.mock_all_auths();
        let contract_id = e.register_contract(None, VeriTixPay);
        let client = VeriTixPayClient::new(&e, &contract_id);
        let admin = Address::generate(&e);
        client.initialize(&admin);
        install_cap(&e, &contract_id, cap);
        (e, client, admin)
    }

    #[test]
    fn test_initialize_stores_admin_and_defaults() {
        let (e, client, admin) = setup();
        assert!(client.is_initialized());
        assert_eq!(client.admin(), admin);
        assert_eq!(client.initialized_at_ledger(), e.ledger().sequence());
        assert_eq!(client.name(), String::from_str(&e, "VeriTix"));
        assert_eq!(client.symbol(), String::from_str(&e, "VTX"));
        assert_eq!(client.decimals(), 7);
        assert_eq!(client.co_signer(), None);
    }

    #[test]
    #[should_panic(expected = "AlreadyInitialized")]
    fn test_initialize_twice_panics() {
        let (e, client, _admin) = setup();
        let other = Address::generate(&e);
        client.initialize(&other);
    }

    #[test]
    fn test_initialize_with_co_signer_stores_signer_and_metadata() {
    fn test_initialize_with_metadata_overrides_defaults() {
        let e = Env::default();
        e.mock_all_auths();
        let contract_id = e.register_contract(None, VeriTixPay);
        let client = VeriTixPayClient::new(&e, &contract_id);
        let admin = Address::generate(&e);
        let co_signer = Address::generate(&e);

        client.initialize_with_co_signer(
            &admin,
            &Some(co_signer.clone()),

        client.initialize_with_metadata(
            &admin,
            &String::from_str(&e, "Event Tickets"),
            &String::from_str(&e, "VTIX"),
            &2,
        );

        assert_eq!(client.admin(), admin);
        assert_eq!(client.co_signer(), Some(co_signer));
        assert_eq!(client.name(), String::from_str(&e, "Event Tickets"));
        assert_eq!(client.name(), String::from_str(&e, "Event Tickets"));
        assert_eq!(client.symbol(), String::from_str(&e, "VTIX"));
        assert_eq!(client.decimals(), 2);
    }

    #[test]
    fn test_burn_reduces_balance_and_supply() {
        let (e, client, admin) = setup();
        let holder = Address::generate(&e);
        funded(&client, &admin, &holder, 1_000);

        client.burn(&holder, &400);

        assert_eq!(client.balance(&holder), 600);
        assert_eq!(client.total_supply(), 600);
    }

    #[test]
    fn test_burn_of_the_entire_balance() {
        let (e, client, admin) = setup();
        let holder = Address::generate(&e);
        funded(&client, &admin, &holder, 1_000);

        client.burn(&holder, &1_000);

        assert_eq!(client.balance(&holder), 0);
    fn test_balance_and_supply_start_at_zero() {
        let (e, client, _admin) = setup();
        let user = Address::generate(&e);
        assert_eq!(client.balance(&user), 0);
        assert_eq!(client.total_supply(), 0);
    }

    #[test]
    #[should_panic(expected = "InsufficientBalance")]
    fn test_burn_rejects_amount_above_the_balance() {
        let (e, client, admin) = setup();
        let holder = Address::generate(&e);
        funded(&client, &admin, &holder, 1_000);

        client.burn(&holder, &1_001);
    }

    #[test]
    #[should_panic(expected = "InvalidAmount")]
    fn test_burn_rejects_zero_amount() {
        let (e, client, admin) = setup();
        let holder = Address::generate(&e);
        funded(&client, &admin, &holder, 1_000);

        client.burn(&holder, &0);
    }

    #[test]
    #[should_panic(expected = "InvalidAmount")]
    fn test_burn_rejects_negative_amount() {
        let (e, client, admin) = setup();
        let holder = Address::generate(&e);
        funded(&client, &admin, &holder, 1_000);

        client.burn(&holder, &(-1));
    }

    #[test]
    fn test_burn_emits_event_with_standard_topics_and_data() {
        let (e, client, admin) = setup();
        let holder = Address::generate(&e);
        funded(&client, &admin, &holder, 1_000);

        client.burn(&holder, &400);

        let events = e.events().all();
        let last = events.events().last().expect("no events recorded");
        let xdr::ContractEventBody::V0(body) = &last.body else {
            panic!("expected a v0 contract event");
        };
        assert_eq!(
            body.topics,
            std::vec![
                xdr::ScVal::from_val(&e, &symbol_short!("burn").to_val()),
                xdr::ScVal::from_val(&e, &holder.to_val()),
            ]
        );
        assert_eq!(body.data, xdr::ScVal::from_val(&e, &vec![&e, 400i128].to_val()));
    }

    #[test]
    fn test_transfer_moves_the_balance() {
        let (e, client, admin) = setup();
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        funded(&client, &admin, &from, 1_000);

        client.transfer(&from, &to, &250);

        assert_eq!(client.balance(&from), 750);
        assert_eq!(client.balance(&to), 250);
        assert_eq!(client.total_supply(), 1_000);
    }

    #[test]
    fn test_transfer_never_changes_total_supply() {
        let (e, client, admin) = setup();
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        funded(&client, &admin, &from, 1_000);

        client.transfer(&from, &to, &1_000);

        assert_eq!(client.balance(&from), 0);
    fn test_mint_credits_balance_and_grows_supply() {
        let (e, client, admin) = setup();
        let user = Address::generate(&e);

        client.mint(&admin, &user, &1_000);

        assert_eq!(client.balance(&user), 1_000);
        assert_eq!(client.total_supply(), 1_000);
    }

    #[test]
    #[should_panic(expected = "InsufficientBalance")]
    fn test_transfer_rejects_amount_above_the_balance() {
        let (e, client, admin) = setup();
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        funded(&client, &admin, &from, 100);

        client.transfer(&from, &to, &101);
    }

    #[test]
    #[should_panic(expected = "InvalidAmount")]
    fn test_transfer_rejects_non_positive_amount() {
        let (e, client, admin) = setup();
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        funded(&client, &admin, &from, 1_000);

        client.transfer(&from, &to, &0);
    }

    #[test]
    #[should_panic(expected = "Frozen")]
    fn test_transfer_rejects_a_frozen_sender() {
        let (e, client, admin) = setup();
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        funded(&client, &admin, &from, 1_000);
        client.set_frozen(&admin, &from, &true);

        client.transfer(&from, &to, &100);
    }

    #[test]
    #[should_panic(expected = "Frozen")]
    fn test_transfer_rejects_a_frozen_recipient() {
        let (e, client, admin) = setup();
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        funded(&client, &admin, &from, 1_000);
        client.set_frozen(&admin, &to, &true);

        client.transfer(&from, &to, &100);
    }

    #[test]
    #[should_panic(expected = "Paused")]
    fn test_transfer_rejects_while_paused() {
        let (e, client, admin) = setup();
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        funded(&client, &admin, &from, 1_000);
        client.set_paused(&admin, &true);

        client.transfer(&from, &to, &100);
    }

    #[test]
    fn test_unfreezing_and_resuming_restores_transfers() {
        let (e, client, admin) = setup();
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        funded(&client, &admin, &from, 1_000);
        client.set_frozen(&admin, &from, &true);
        client.set_paused(&admin, &true);

        client.set_frozen(&admin, &from, &false);
        client.set_paused(&admin, &false);
        client.transfer(&from, &to, &100);

        assert_eq!(client.balance(&to), 100);
        assert!(!client.is_frozen(&from));
        assert!(!client.is_paused());
    fn test_mint_accumulates_across_calls() {
        let (e, client, admin) = setup();
        let user = Address::generate(&e);

        client.mint(&admin, &user, &1_000);
        client.mint(&admin, &user, &500);

        assert_eq!(client.balance(&user), 1_500);
        assert_eq!(client.total_supply(), 1_500);
    }

    #[test]
    #[should_panic(expected = "Unauthorized: caller is not the contract admin")]
    fn test_set_frozen_is_admin_only() {
        let (e, client, _admin) = setup();
        let stranger = Address::generate(&e);
        let victim = Address::generate(&e);

        client.set_frozen(&stranger, &victim, &true);
    }

    #[test]
    #[should_panic(expected = "Unauthorized: caller is not the contract admin")]
    fn test_set_paused_is_admin_only() {
        let (e, client, _admin) = setup();
        let stranger = Address::generate(&e);

        client.set_paused(&stranger, &true);
    }

    #[test]
    fn test_transfer_emits_event_with_standard_topics_and_data() {
        let (e, client, admin) = setup();
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        funded(&client, &admin, &from, 1_000);

        client.transfer(&from, &to, &250);

        let events = e.events().all();
        let last = events.events().last().expect("no events recorded");
        let xdr::ContractEventBody::V0(body) = &last.body else {
            panic!("expected a v0 contract event");
        };
        assert_eq!(
            body.topics,
            std::vec![
                xdr::ScVal::from_val(&e, &symbol_short!("transfer").to_val()),
                xdr::ScVal::from_val(&e, &from.to_val()),
                xdr::ScVal::from_val(&e, &to.to_val()),
            ]
        );
        assert_eq!(body.data, xdr::ScVal::from_val(&e, &vec![&e, 250i128].to_val()));
    }

    #[test]
    fn test_clawback_debits_the_holder_and_reduces_supply() {
        let (e, client, admin) = setup();
        let holder = Address::generate(&e);
        funded(&client, &admin, &holder, 1_000);

        client.clawback(&admin, &holder, &400);

        assert_eq!(client.balance(&holder), 600);
        assert_eq!(client.total_supply(), 600);
    }

    #[test]
    #[should_panic(expected = "Unauthorized: caller is not the contract admin")]
    fn test_clawback_rejects_a_non_admin() {
        let (e, client, admin) = setup();
        let holder = Address::generate(&e);
        let stranger = Address::generate(&e);
        funded(&client, &admin, &holder, 1_000);

        client.clawback(&stranger, &holder, &400);
    }

    #[test]
    #[should_panic(expected = "InsufficientBalance")]
    fn test_clawback_rejects_amount_above_the_balance() {
        let (e, client, admin) = setup();
        let holder = Address::generate(&e);
        funded(&client, &admin, &holder, 100);

        client.clawback(&admin, &holder, &101);
    }

    #[test]
    fn test_clawback_works_without_a_co_signer() {
        let (e, client, admin) = setup();
        let holder = Address::generate(&e);
        funded(&client, &admin, &holder, 1_000);

        client.clawback(&admin, &holder, &1_000);

        assert_eq!(client.balance(&holder), 0);
    }

    #[test]
    fn test_clawback_emits_its_own_event() {
        let (e, client, admin) = setup();
        let holder = Address::generate(&e);
        funded(&client, &admin, &holder, 1_000);

        client.clawback(&admin, &holder, &400);

        let events = e.events().all();
        let last = events.events().last().expect("no events recorded");
        let xdr::ContractEventBody::V0(body) = &last.body else {
    fn test_mint_rejects_non_admin() {
        let (e, client, _admin) = setup();
        let stranger = Address::generate(&e);
        let user = Address::generate(&e);
        client.mint(&stranger, &user, &1_000);
    }

    #[test]
    #[should_panic(expected = "InvalidAmount")]
    fn test_mint_rejects_zero_amount() {
        let (e, client, admin) = setup();
        let user = Address::generate(&e);
        client.mint(&admin, &user, &0);
    }

    #[test]
    #[should_panic(expected = "InvalidAmount")]
    fn test_mint_rejects_negative_amount() {
        let (e, client, admin) = setup();
        let user = Address::generate(&e);
        client.mint(&admin, &user, &(-1));
    }

    #[test]
    #[should_panic(expected = "SupplyCapExceeded")]
    fn test_mint_rejects_amount_over_the_cap() {
        let (e, client, admin) = setup_capped(1_000);
        let user = Address::generate(&e);
        client.mint(&admin, &user, &1_001);
    }

    #[test]
    fn test_mint_allows_amount_exactly_at_the_cap() {
        let (e, client, admin) = setup_capped(1_000);
        let user = Address::generate(&e);
        client.mint(&admin, &user, &1_000);
        assert_eq!(client.total_supply(), 1_000);
    }

    #[test]
    #[should_panic(expected = "SupplyCapExceeded")]
    fn test_mint_counts_prior_mints_against_the_cap() {
        let (e, client, admin) = setup_capped(1_000);
        let user = Address::generate(&e);
        client.mint(&admin, &user, &600);
        client.mint(&admin, &user, &500);
    }

    #[test]
    fn test_mint_emits_event_with_standard_topics_and_data() {
        let (e, client, admin) = setup();
        let user = Address::generate(&e);

        client.mint(&admin, &user, &1_000);

        let events = e.events().all();
        assert_eq!(events.events().len(), 1);
        let xdr::ContractEventBody::V0(body) = &events.events()[0].body else {
            panic!("expected a v0 contract event");
        };
        assert_eq!(
            body.topics,
            std::vec![
                xdr::ScVal::from_val(&e, &symbol_short!("clawback").to_val()),
                xdr::ScVal::from_val(&e, &admin.to_val()),
                xdr::ScVal::from_val(&e, &holder.to_val()),
            ]
        );
        assert_eq!(body.data, xdr::ScVal::from_val(&e, &vec![&e, 400i128].to_val()));
    }

    #[test]
    fn test_spendable_balance_matches_the_balance_without_locks() {
        let (e, client, admin) = setup();
        let holder = Address::generate(&e);
        funded(&client, &admin, &holder, 1_000);

        assert_eq!(client.spendable_balance(&holder), 1_000);
        assert_eq!(client.escrow_locked(&holder), 0);
    }

    #[test]
    fn test_spendable_balance_excludes_escrow_locks() {
        let e = Env::default();
        e.mock_all_auths();
        let contract_id = e.register_contract(None, VeriTixPay);
        let client = VeriTixPayClient::new(&e, &contract_id);
        let admin = Address::generate(&e);
        let holder = Address::generate(&e);
        client.initialize(&admin);
        client.mint(&admin, &holder, &1_000);
        set_escrow_lock(&e, &contract_id, &holder, 400);

        assert_eq!(client.balance(&holder), 1_000);
        assert_eq!(client.escrow_locked(&holder), 400);
        assert_eq!(client.spendable_balance(&holder), 600);
    }

    #[test]
    fn test_spendable_balance_is_zero_for_a_frozen_account() {
        let e = Env::default();
        e.mock_all_auths();
        let contract_id = e.register_contract(None, VeriTixPay);
        let client = VeriTixPayClient::new(&e, &contract_id);
        let admin = Address::generate(&e);
        let holder = Address::generate(&e);
        client.initialize(&admin);
        client.mint(&admin, &holder, &1_000);
        client.set_frozen(&admin, &holder, &true);

        assert_eq!(client.balance(&holder), 1_000);
        assert_eq!(client.spendable_balance(&holder), 0);
    }

    #[test]
    fn test_spendable_balance_never_goes_negative() {
        let e = Env::default();
        e.mock_all_auths();
        let contract_id = e.register_contract(None, VeriTixPay);
        let client = VeriTixPayClient::new(&e, &contract_id);
        let admin = Address::generate(&e);
        let holder = Address::generate(&e);
        client.initialize(&admin);
        client.mint(&admin, &holder, &1_000);
        // A lock larger than the balance cannot happen through the escrow
        // module, which validates first, but a stale record must not make this
        // view report a negative amount.
        set_escrow_lock(&e, &contract_id, &holder, 5_000);

        assert_eq!(client.spendable_balance(&holder), 0);
    }

    #[test]
    fn test_spendable_balance_of_an_unknown_account_is_zero() {
        let (e, client, _admin) = setup();
        let stranger = Address::generate(&e);
        assert_eq!(client.spendable_balance(&stranger), 0);
                xdr::ScVal::from_val(&e, &symbol_short!("mint").to_val()),
                xdr::ScVal::from_val(&e, &user.to_val()),
            ]
        );
        assert_eq!(
            body.data,
            xdr::ScVal::from_val(&e, &vec![&e, 1_000i128].to_val())
        );
    }
}
