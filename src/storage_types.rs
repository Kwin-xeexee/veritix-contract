use soroban_sdk::{contracttype, Address};

/// Every persistent storage key the contract owns.
///
/// Keeping the key set in one enum means a storage layout question only ever
/// has one answer, and the layout can be dumped as a whole when auditing.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Address holding admin authority.
    Admin,
    /// Ledger on which `initialize` ran.
    InitializedAtLedger,
    /// Token name reported by `name()`.
    Name,
    /// Token symbol reported by `symbol()`.
    Symbol,
    /// Token precision reported by `decimals()`.
    Decimals,
    /// Number of tokens in circulation.
    TotalSupply,
    /// Hard supply cap. Unset or 0 means supply is uncapped.
    MaxSupply,
    /// Balance held by a single account.
    BalanceOf(Address),
    /// Amount `spender` may move on `from`'s behalf.
    Allowance(Address, Address),
    /// Ledger at which `Allowance(from, spender)` expires.
    AllowanceExpiration(Address, Address),
    /// Every spender `owner` currently has a live allowance with.
    AllowanceSpenders(Address),
    /// Escrow record keyed by its id.
    EscrowRecord(u32),
    /// Number of escrows ever created; also the next escrow id.
    EscrowCount,
    /// Total token amount held across all active escrows.
    EscrowValueLocked,
}

/// Where an escrow is in its lifecycle.
///
/// Settlement outcomes land in later issues; `Active` is the only state
/// `create_escrow` can produce.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscrowStatus {
    /// Funds are held and awaiting settlement.
    Active,
}

/// One escrow: who deposited, who is owed, and under what deadline.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowRecord {
    pub depositor: Address,
    pub beneficiary: Address,
    pub token: Address,
    pub amount: i128,
    pub deadline_ledger: u32,
    pub status: EscrowStatus,
}
