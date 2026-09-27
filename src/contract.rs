use crate::metadata::{self, TokenMetadata};
use crate::storage_types::DataKey;
use crate::{admin, authorization, balance};
use soroban_sdk::{contract, contractimpl, Address, Bytes, Env, String};
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
use crate::metadata::{self, TokenMetadata};
use crate::storage_types::DataKey;
use crate::{admin, balance, control};
use crate::{admin, balance};
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
    /// The admin address is the root of trust for every privileged entry point,
    /// so initialization is deliberately one-shot: a second call is rejected
    /// rather than silently replacing the admin.
    pub fn initialize(e: Env, admin_addr: Address) {
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
    ///
    /// The `0` sentinel means "no cap has been configured", not "no tokens may
    /// exist". It is the same value [`mint`] reads, so an integrator can compare
    /// `total_supply()` against this to report remaining headroom, and `0`
    /// means there is no headroom to report.
    pub fn max_supply(e: Env) -> i128 {
        balance::max_supply(&e)
    }

    /// Whether `account` may move tokens. True unless the admin has revoked it.
    pub fn is_authorized(e: Env, account: Address) -> bool {
        authorization::is_authorized(&e, &account)
    }

    /// Number of accounts currently holding a positive balance.
    pub fn holder_count(e: Env) -> u32 {
        balance::holder_count(&e)
    }

    /// Grants or revokes `account`'s authorization to move tokens.
    ///
    /// Independent of any future freeze: this flag gates whether an account may
    /// move tokens at all, while a freeze holds a balance in place. A revoked
    /// account keeps its balance and its place on the holder set.
    pub fn set_authorized(e: Env, admin_addr: Address, account: Address, authorized: bool) {
        admin::check_admin(&e, &admin_addr);
        authorization::set_authorized(&e, &account, authorized);
    }

    /// Mints `amount` new tokens to `to`.
    ///
    /// Admin-only and supply-capped. This is the prerequisite the rest of the
    /// token module is built on: without a way to bring supply into
    /// circulation there is nothing to transfer, burn or enumerate.
    pub fn mint(e: Env, admin_addr: Address, to: Address, amount: i128) {
        admin::check_admin(&e, &admin_addr);
        balance::mint(&e, &to, amount);
    }

    /// Destroys `amount` of the caller's own tokens.
    pub fn burn(e: Env, from: Address, amount: i128) {
        from.require_auth();
        balance::burn(&e, &from, amount);
    }

    /// Moves `amount` of tokens from `from` to `to`.
    ///
    /// `from` must sign the call and must still be authorized.
    pub fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        balance::transfer(&e, &from, &to, amount);
    }

    /// Moves `amount` of tokens from `from` to `to` tagged with an opaque memo.
    ///
    /// Identical to [`transfer`] apart from the memo, which is carried in the
    /// emitted event and never written to storage. Memos longer than 64 bytes
    /// are rejected.
    pub fn transfer_with_memo(e: Env, from: Address, to: Address, amount: i128, memo: Bytes) {
        from.require_auth();
        balance::transfer_with_memo(&e, &from, &to, amount, memo);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Events as _};
    use soroban_sdk::{symbol_short, vec, xdr, FromVal, Vec};

    fn setup() -> (Env, VeriTixPayClient<'static>, Address) {
        let e = Env::default();
        e.mock_all_auths();
        let contract_id = e.register_contract(None, VeriTixPay);
        let client = VeriTixPayClient::new(&e, &contract_id);
        let admin = Address::generate(&e);
        client.initialize(&admin);
        (e, client, admin)
    }

    fn funded(client: &VeriTixPayClient<'static>, admin: &Address, holder: &Address, amount: i128) {
        client.mint(admin, holder, &amount);
    }

    fn memo(e: &Env, text: &str) -> Bytes {
        Bytes::from_slice(e, text.as_bytes())
    }

    /// Installs a supply cap the way the initializer would.
    fn install_cap(e: &Env, contract_id: &Address, cap: i128) {
        e.as_contract(contract_id, || {
            e.storage().persistent().set(&DataKey::MaxSupply, &cap);
        });
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
    }

    #[test]
    #[should_panic(expected = "AlreadyInitialized")]
    fn test_initialize_twice_panics() {
        let (e, client, _admin) = setup();
        client.initialize(&Address::generate(&e));
    }

    #[test]
    fn test_initialize_with_metadata_overrides_defaults() {
        let e = Env::default();
        e.mock_all_auths();
        let contract_id = e.register_contract(None, VeriTixPay);
        let client = VeriTixPayClient::new(&e, &contract_id);
        let admin = Address::generate(&e);

        client.initialize_with_metadata(
            &admin,
            &String::from_str(&e, "Event Tickets"),
            &String::from_str(&e, "VTIX"),
            &2,
        );

        assert_eq!(client.name(), String::from_str(&e, "Event Tickets"));
        assert_eq!(client.decimals(), 2);
    }

    #[test]
    fn test_accounts_are_authorized_by_default() {
        let (e, client, _admin) = setup();
        let user = Address::generate(&e);
        assert!(client.is_authorized(&user));
    }

    #[test]
    fn test_set_authorized_revokes_and_restores() {
        let (e, client, admin) = setup();
        let user = Address::generate(&e);

        client.set_authorized(&admin, &user, &false);
        assert!(!client.is_authorized(&user));

        client.set_authorized(&admin, &user, &true);
        assert!(client.is_authorized(&user));
    }

    #[test]
    #[should_panic(expected = "Unauthorized: caller is not the contract admin")]
    fn test_set_authorized_is_admin_only() {
        let (e, client, _admin) = setup();
        let stranger = Address::generate(&e);
        let victim = Address::generate(&e);

        client.set_authorized(&stranger, &victim, &false);
    }

    #[test]
    fn test_revoking_authorization_keeps_the_balance() {
        let (e, client, admin) = setup();
        let user = Address::generate(&e);
        funded(&client, &admin, &user, 1_000);

        client.set_authorized(&admin, &user, &false);

        assert_eq!(client.balance(&user), 1_000);
        assert_eq!(client.holder_count(), 1);
    }

    #[test]
    #[should_panic(expected = "NotAuthorized")]
    fn test_transfer_rejects_a_revoked_sender() {
        let (e, client, admin) = setup();
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        funded(&client, &admin, &from, 1_000);
        client.set_authorized(&admin, &from, &false);

        client.transfer(&from, &to, &100);
    }

    #[test]
    fn test_transfer_succeeds_after_authorization_is_restored() {
        let (e, client, admin) = setup();
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        funded(&client, &admin, &from, 1_000);
        client.set_authorized(&admin, &from, &false);
        client.set_authorized(&admin, &from, &true);

        client.transfer(&from, &to, &100);

        assert_eq!(client.balance(&to), 100);
    }

    #[test]
    #[should_panic(expected = "NotAuthorized")]
    fn test_transfer_with_memo_consults_the_same_flag() {
        let (e, client, admin) = setup();
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        funded(&client, &admin, &from, 1_000);
        client.set_authorized(&admin, &from, &false);

        client.transfer_with_memo(&from, &to, &100, &memo(&e, "order-1"));
    }

    #[test]
    fn test_transfer_with_memo_moves_the_balance() {
        let (e, client, admin) = setup();
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        funded(&client, &admin, &from, 1_000);

        client.transfer_with_memo(&from, &to, &250, &memo(&e, "order-1"));

        assert_eq!(client.balance(&from), 750);
        assert_eq!(client.balance(&to), 250);
        assert_eq!(client.total_supply(), 1_000);
    }

    #[test]
    fn test_transfer_with_memo_emits_the_memo_in_the_event() {
        let (e, client, admin) = setup();
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        funded(&client, &admin, &from, 1_000);
        let note = memo(&e, "order-1");

        client.transfer_with_memo(&from, &to, &250, &note);

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
        let xdr::ScVal::Vec(items) = &body.data else {
            panic!("expected vec data");
        };
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], xdr::ScVal::from_val(&e, &250i128.to_val()));
        assert_eq!(items[1], xdr::ScVal::from_val(&e, &note.to_val()));
    }

    #[test]
    #[should_panic(expected = "MemoTooLarge")]
    fn test_transfer_with_memo_rejects_an_oversized_memo() {
        let (e, client, admin) = setup();
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        funded(&client, &admin, &from, 1_000);
        let long = Bytes::from_slice(&e, &[7u8; 65]);

        client.transfer_with_memo(&from, &to, &100, &long);
    }

    #[test]
    fn test_transfer_with_memo_accepts_a_memo_at_the_limit() {
        let (e, client, admin) = setup();
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        funded(&client, &admin, &from, 1_000);
        let at_limit = Bytes::from_slice(&e, &[7u8; 64]);

        client.transfer_with_memo(&from, &to, &100, &at_limit);

        assert_eq!(client.balance(&to), 100);
    }

    #[test]
    #[should_panic(expected = "InvalidAmount")]
    fn test_transfer_with_memo_still_rejects_a_non_positive_amount() {
        let (e, client, admin) = setup();
        let from = Address::generate(&e);
        let to = Address::generate(&e);
        funded(&client, &admin, &from, 1_000);

        client.transfer_with_memo(&from, &to, &0, &memo(&e, "order-1"));
    }

    #[test]
    fn test_max_supply_is_zero_when_uncapped() {
        let (_e, client, _admin) = setup();
        assert_eq!(client.max_supply(), 0);
    }

    #[test]
    fn test_max_supply_returns_the_configured_cap() {
        let e = Env::default();
        e.mock_all_auths();
        let contract_id = e.register_contract(None, VeriTixPay);
        let client = VeriTixPayClient::new(&e, &contract_id);
        let admin = Address::generate(&e);
        client.initialize(&admin);
        install_cap(&e, &contract_id, 10_000);

        assert_eq!(client.max_supply(), 10_000);
    }

    #[test]
    fn test_holder_set_starts_empty() {
        let (e, client, _admin) = setup();
        let user = Address::generate(&e);
        assert_eq!(client.holder_count(), 0);
        assert_eq!(balance::holders(&e), Vec::new(&e));
        assert_eq!(client.balance(&user), 0);
    }

    #[test]
    fn test_first_credit_joins_the_holder_set() {
        let (e, client, admin) = setup();
        let user = Address::generate(&e);

        client.mint(&admin, &user, &1_000);

        assert_eq!(client.holder_count(), 1);
        assert_eq!(balance::holders(&e), vec![&e, user.clone()]);
    }

    #[test]
    fn test_a_second_credit_does_not_duplicate_the_holder() {
        let (e, client, admin) = setup();
        let user = Address::generate(&e);

        client.mint(&admin, &user, &1_000);
        client.mint(&admin, &user, &500);

        assert_eq!(client.holder_count(), 1);
        assert_eq!(balance::holders(&e), vec![&e, user.clone()]);
    }

    #[test]
    fn test_crediting_an_existing_holder_keeps_the_set_unique() {
        let (e, client, admin) = setup();
        let alice = Address::generate(&e);
        let bob = Address::generate(&e);
        funded(&client, &admin, &alice, 1_000);
        funded(&client, &admin, &bob, 1_000);

        client.mint(&admin, &alice, &100);

        assert_eq!(client.holder_count(), 2);
        assert_eq!(balance::holders(&e), vec![&e, alice.clone(), bob.clone()]);
    }

    #[test]
    fn test_burning_the_whole_balance_leaves_the_holder_set() {
        let (e, client, admin) = setup();
        let user = Address::generate(&e);
        funded(&client, &admin, &user, 1_000);

        client.burn(&user, &1_000);

        assert_eq!(client.holder_count(), 0);
        assert_eq!(balance::holders(&e), Vec::new(&e));
    }

    #[test]
    fn test_a_partial_burn_keeps_the_holder() {
        let (e, client, admin) = setup();
        let user = Address::generate(&e);
        funded(&client, &admin, &user, 1_000);

        client.burn(&user, &400);

        assert_eq!(client.holder_count(), 1);
        assert_eq!(balance::holders(&e), vec![&e, user.clone()]);
    }

    #[test]
    fn test_draining_the_holder_set_empties_it() {
        let (e, client, admin) = setup();
        let alice = Address::generate(&e);
        let bob = Address::generate(&e);
        funded(&client, &admin, &alice, 1_000);
        funded(&client, &admin, &bob, 500);

        client.burn(&alice, &1_000);
        client.burn(&bob, &500);

        assert_eq!(client.holder_count(), 0);
        assert_eq!(balance::holders(&e), Vec::new(&e));
    }

    #[test]
    fn test_a_drained_holder_can_come_back() {
        let (e, client, admin) = setup();
        let user = Address::generate(&e);
        funded(&client, &admin, &user, 1_000);
        client.burn(&user, &1_000);

        client.mint(&admin, &user, &250);

        assert_eq!(client.holder_count(), 1);
        assert_eq!(balance::holders(&e), vec![&e, user.clone()]);
    }

    #[test]
    fn test_transfer_joins_and_removes_holders() {
        let (e, client, admin) = setup();
        let alice = Address::generate(&e);
        let bob = Address::generate(&e);
        funded(&client, &admin, &alice, 1_000);

        client.transfer(&alice, &bob, &400);
        assert_eq!(client.holder_count(), 2);
        assert_eq!(balance::holders(&e), vec![&e, alice.clone(), bob.clone()]);

        client.transfer(&alice, &bob, &600);
        assert_eq!(client.holder_count(), 1);
        assert_eq!(balance::holders(&e), vec![&e, bob.clone()]);
    }

    #[test]
    fn test_holder_set_mirrors_balances_after_a_rejected_transfer() {
        let (e, client, admin) = setup();
        let alice = Address::generate(&e);
        let bob = Address::generate(&e);
        funded(&client, &admin, &alice, 100);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.transfer(&alice, &bob, &101);
        }));

        assert!(result.is_err());
        assert_eq!(client.holder_count(), 1);
        assert_eq!(balance::holders(&e), vec![&e, alice.clone()]);
        assert_eq!(client.balance(&bob), 0);
    }
}
