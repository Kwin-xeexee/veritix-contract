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
    /// Escrow record keyed by its id.
    EscrowRecord(u32),
    /// Number of escrows ever created; also the next escrow id.
    EscrowCount,
    /// Total token amount held across all active escrows.
    EscrowValueLocked,
}

/// Where an escrow is in its lifecycle.
///
/// `Active` is the only state an escrow can be created in; the other two are
/// terminal and mutually exclusive, and neither ever returns to `Active`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscrowStatus {
    /// Funds are still held and the escrow can still be settled.
    Active,
    /// Settled in the beneficiary's favour.
    Released,
    /// Settled in the depositor's favour.
    Refunded,
}

/// One escrow: who deposited, who is owed, and under what deadline.
///
/// `amount` is the amount still held, not the amount originally deposited. A
/// partial settlement reduces it, and an escrow is settled only once it reaches
/// zero, so a record never has to be read alongside the event log to tell how
/// much is still owed.
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
