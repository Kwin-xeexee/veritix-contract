//! Shared storage types.
//!
//! This module is the single source of truth for every storage key in the
//! contract. A previous implementation carried two separate `DataKey` enums
//! side by side, and the two silently disagreed about where an account's
//! balance lived. **Exactly one `DataKey` enum may exist in this crate**, and
//! later modules extend it rather than declaring another.
//!
//! Amounts are `i128` because that is the type Stellar uses for token amounts
//! (stroops). Ledger numbers are `u32`, matching Soroban's ledger sequence.

use soroban_sdk::{contracttype, Address, Vec};

/// Every storage key used by the contract.
///
/// Keys are grouped by concern. A variant is added here when the module that
/// needs it lands; the enum is never redefined.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    // --- Admin and governance ------------------------------------------------
    /// The address that may call administrative entry points.
    Admin,
    /// The administrator nominated by a two-step ownership transfer, if any.
    PendingAdmin,
    /// The ledger from which `PendingAdmin` becomes the active `Admin`.
    AdminActiveAfterLedger,
    /// The second signer required alongside the admin for a clawback.
    ClawbackCosigner,

    // --- Balances ------------------------------------------------------------
    /// Token balance of a single account.
    Balance(Address),
    /// Spending allowance granted by `owner` to `spender`.
    Allowance(Address, Address),

    // --- Supply --------------------------------------------------------------
    /// Total tokens in circulation.
    TotalSupply,
    /// The hard supply cap fixed at initialization, if one was set.
    MaxSupply,

    // --- Counters ------------------------------------------------------------
    /// A monotonically increasing per-address counter.
    Counter(Address),
}

/// Lifecycle state of an escrow.
///
/// `Active` is the only state from which a release or a refund may proceed;
/// `Disputed` additionally blocks them until the dispute is resolved.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscrowStatus {
    /// Funds are held and may be released or refunded.
    Active,
    /// Funds were paid to the beneficiary. Terminal.
    Released,
    /// Funds were returned to the depositor. Terminal.
    Refunded,
    /// A dispute is open over the escrow. Release and refund are blocked.
    Disputed,
}

/// A single escrow: funds held by the contract on a depositor's behalf.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowRecord {
    /// Who funded the escrow.
    pub depositor: Address,
    /// Who is to be paid on release.
    pub beneficiary: Address,
    /// The token being held.
    pub token: Address,
    /// The amount held, in the token's base unit.
    pub amount: i128,
    /// Current lifecycle state.
    pub status: EscrowStatus,
    /// Ledger at which the escrow was created, for age and expiry checks.
    pub created_ledger: u32,
}

/// A payment split: one sender, many recipients, shares in basis points.
///
/// `shares_bps` is parallel to `recipients`: `shares_bps[i]` is the share owed
/// to `recipients[i]`, and the shares must total 10_000.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitRecord {
    /// Who created the split and receives the surplus on cancellation.
    pub sender: Address,
    /// The token being distributed.
    pub token: Address,
    /// The full amount the split covers.
    pub total_amount: i128,
    /// Who is to be paid, in share order.
    pub recipients: Vec<Address>,
    /// Basis-point share per recipient, parallel to `recipients`. Totals 10_000.
    pub shares_bps: Vec<u32>,
    /// Whether distribution has already happened. Guards against paying twice.
    pub distributed: bool,
}

/// A recurring charge authorised between a payer and a payee.
///
/// `next_execution` is the ledger at which the next charge becomes due;
/// `interval_ledgers` is the gap between successive charges.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecurringPayment {
    /// Who is charged.
    pub payer: Address,
    /// Who receives the charge.
    pub payee: Address,
    /// The token charged.
    pub token: Address,
    /// The amount charged on each execution, in the token's base unit.
    pub amount: i128,
    /// Ledgers between successive charges. Must be greater than zero.
    pub interval_ledgers: u32,
    /// Ledger at which the next charge becomes due.
    pub next_execution: u32,
    /// Whether the schedule is still authorised. Cleared on cancellation.
    pub active: bool,
    /// Whether the schedule is temporarily suspended. Survives resume.
    pub paused: bool,
}
