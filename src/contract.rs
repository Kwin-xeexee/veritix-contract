use crate::metadata::{self, TokenMetadata};
use crate::storage_types::DataKey;
use crate::{admin, allowance, balance, control};
use soroban_sdk::{contract, contractimpl, Address, Bytes, Env, String, Vec};

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
    // ---- Metadata -------------------------------------------------------

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

    // ---- Balances and supply --------------------------------------------

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

    /// Every account currently holding a positive balance.
    pub fn get_holders(e: Env) -> Vec<Address> {
        balance::holders(&e)
    }

    /// Number of accounts currently holding a positive balance.
    pub fn total_holders(e: Env) -> u32 {
        balance::holder_count(&e)
    }

    /// Tokens `account` may move right now: the balance minus escrow locks, or 0
    /// while the account is frozen.
    pub fn spendable_balance(e: Env, account: Address) -> i128 {
        balance::spendable_balance(&e, &account)
    }

    /// Tokens of `account` currently held in active escrows.
    pub fn escrow_locked(e: Env, account: Address) -> i128 {
        balance::escrow_locked(&e, &account)
    }

    // ---- Allowances -----------------------------------------------------

    /// Amount `spender` may still move on `from`'s behalf.
    ///
    /// An allowance at or past its expiration reads as `0`.
    pub fn allowance(e: Env, from: Address, spender: Address) -> i128 {
        allowance::allowance(&e, &from, &spender)
    }

    /// Ledger at which the `from`/`spender` allowance expires.
    pub fn allowance_expiration(e: Env, from: Address, spender: Address) -> u32 {
        allowance::allowance_expiration(&e, &from, &spender)
    }

    /// Authorizes `spender` to move up to `amount` of `from`'s tokens until
    /// `expiration_ledger`.
    ///
    /// Overwrites any existing allowance rather than adding to it, so a spender
    /// handed a new number cannot end up with the old one plus the new one. An
    /// `amount` of `0` revokes the allowance and is always accepted.
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

    // ---- Moving tokens --------------------------------------------------

    /// Moves `amount` of tokens from `from` to `to`.
    pub fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        balance::transfer(&e, &from, &to, amount);
    }

    /// Moves `amount` from `from` to `to` on `spender`'s authority, consuming
    /// the spender's allowance.
    pub fn transfer_from(e: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();
        balance::transfer_from(&e, &spender, &from, &to, amount);
    }

    /// Moves `amount` of tokens from `from` to `to` tagged with an opaque memo.
    ///
    /// The memo is carried in the emitted event and never written to storage.
    /// Memos longer than 64 bytes are rejected.
    pub fn transfer_with_memo(e: Env, from: Address, to: Address, amount: i128, memo: Bytes) {
        from.require_auth();
        balance::transfer_with_memo(&e, &from, &to, amount, memo);
    }

    /// Destroys `amount` of the caller's own tokens.
    pub fn burn(e: Env, from: Address, amount: i128) {
        from.require_auth();
        balance::burn(&e, &from, amount);
    }

    /// Destroys `amount` of `from`'s tokens on `spender`'s authority, consuming
    /// the spender's allowance.
    pub fn burn_from(e: Env, spender: Address, from: Address, amount: i128) {
        spender.require_auth();
        balance::burn_from(&e, &spender, &from, amount);
    }

    /// Recovers `amount` from `from` on the admin's authority.
    ///
    /// The holder does not sign: that is the point of a clawback, which is why
    /// it emits its own event rather than reusing the burn event.
    pub fn clawback(e: Env, admin_addr: Address, from: Address, amount: i128) {
        admin::check_admin(&e, &admin_addr);
        balance::clawback(&e, &admin_addr, &from, amount);
    }

    /// Mints `amount` new tokens to `to`. Admin-only and supply-capped.
    pub fn mint(e: Env, admin_addr: Address, to: Address, amount: i128) {
        admin::check_admin(&e, &admin_addr);
        balance::mint(&e, &to, amount);
    }

    // ---- Compliance controls -------------------------------------------

    /// Whether the contract is paused.
    pub fn is_paused(e: Env) -> bool {
        control::is_paused(&e)
    }

    /// Whether `account` is frozen.
    pub fn is_frozen(e: Env, account: Address) -> bool {
        control::is_frozen(&e, &account)
    }

    /// Freezes or thaws `account`. A frozen account keeps its balance but cannot
    /// send or receive tokens.
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
pub(crate) mod testing {
    use crate::contract::{VeriTixPay, VeriTixPayClient};
    use crate::storage_types::DataKey;
    use soroban_sdk::{Address, Bytes, Env, Vec};

    /// A deployed contract with an admin and some supply in circulation.
    pub struct Fixture {
        pub e: Env,
        pub client: VeriTixPayClient<'static>,
        pub contract_id: Address,
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
            Fixture {
                e,
                client,
                contract_id,
                admin,
            }
        }

        /// Mints `amount` to a fresh address and returns it.
        pub fn fund(&self, amount: i128) -> Address {
            let holder = Address::generate(&self.e);
            self.client.mint(&self.admin, &holder, &amount);
            holder
        }

        /// Writes the escrow lock total the escrow module would maintain.
        pub fn set_escrow_lock(&self, account: &Address, locked: i128) {
            self.e.as_contract(&self.contract_id, || {
                self.e
                    .storage()
                    .persistent()
                    .set(&DataKey::EscrowLocked(account.clone()), &locked);
            });
        }

        /// Writes a supply cap the way the initializer would.
        pub fn set_max_supply(&self, cap: i128) {
            self.e.as_contract(&self.contract_id, || {
                self.e.storage().persistent().set(&DataKey::MaxSupply, &cap);
            });
        }
    }

    /// A `Bytes` value of `len` bytes, all set to `fill`.
    pub fn bytes_of_len(e: &Env, len: u32, fill: u8) -> Bytes {
        Bytes::from_slice(e, &std::vec![fill; len as usize])
    }

    /// An empty holder list for the given environment.
    pub fn no_holders(e: &Env) -> Vec<Address> {
        Vec::new(e)
    }
}
