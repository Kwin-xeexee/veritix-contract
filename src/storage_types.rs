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
    /// Optional second address that must co-sign every clawback.
    CoSigner,
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
    /// Whether an account is frozen and cannot move tokens.
    Frozen(Address),
    /// Whether the whole contract is paused.
    Paused,
    /// Total tokens held in active escrows for an account.
    EscrowLocked(Address),
}
