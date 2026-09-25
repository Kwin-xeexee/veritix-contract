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
//!
//! # Storage durability
//!
//! Two tiers are in use, and every key in `DataKey` belongs to exactly one:
//!
//! - **Instance** storage is the contract's own configuration — who the admin
//!   is, total supply, the supply cap. It is cheap to read and is bounded by
//!   the contract footprint, so it holds only a small fixed set of values.
//! - **Persistent** storage holds per-account and per-record data whose count
//!   grows with usage — balances, allowances, counters, escrows, splits,
//!   schedules, disputes. Call `bump_persistent` on every read and write so
//!   entries are not archived out from under their owners.

use soroban_sdk::{contracttype, Address, Env, Vec};

/// How many ledgers an entry must still have before a bump is worth doing.
///
/// Roughly 30 days at Stellar's 5-second ledger close. Below this threshold
/// the entry is comfortably far from expiry, so extending it is wasted work.
pub const INSTANCE_LIFETIME_THRESHOLD: u32 = 518_400;

/// How far to extend instance storage when bumping, in ledgers.
///
/// Roughly 15 days. Comfortably above `INSTANCE_LIFETIME_THRESHOLD`, so a
/// contract that is never called again still keeps its configuration for a
/// meaningful window before it can be restored.
pub const INSTANCE_BUMP_AMOUNT: u32 = 2_592_000;

/// How many ledgers a persistent entry must still have before a bump is
/// worth doing.
///
/// Roughly 120 days at Stellar's 5-second ledger close.
pub const PERSISTENT_LIFETIME_THRESHOLD: u32 = 2_073_600;

/// How far to extend persistent storage when bumping, in ledgers.
///
/// Roughly 180 days. Long-lived records such as escrows and recurring
/// schedules can legitimately sit untouched for over a year, so the window is
/// generous on purpose.
pub const PERSISTENT_BUMP_AMOUNT: u32 = 4_752_000;

/// Extend the contract's instance storage so its configuration does not expire.
///
/// Call this at the top of any entry point that reads or writes instance
/// storage — most importantly at the start of every administrative function,
/// since the admin key and the supply counters live there. An archived
/// instance is far more serious than an archived balance: the contract can be
/// restored from a backup, but until it is, the contract does not exist.
///
/// # Panics
///
/// Propagates the host's storage error if the extension is refused, which
/// happens when `INSTANCE_BUMP_AMOUNT` exceeds the network's maximum entry
/// TTL. The constants above are sized for current mainnet settings.
pub fn bump_instance(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
}

/// Extend a single persistent entry so it is not archived.
///
/// Call this on **every** read and every write of a persistent key, not only
/// on writes. A read that returns a live value is exactly the case where the
/// owner still cares, and skipping the bump on reads is how long-dormant
/// escrows and schedules get archived: nothing touches them until someone acts
/// on them, and by then the entry is gone and the contract cannot distinguish
/// "never existed" from "expired".
///
/// # Panics
///
/// Propagates the host's storage error if `key` is not a persistent key. It
/// is a programming error to call this with an instance key, and silently
/// succeeding would hide the bug.
pub fn bump_persistent(e: &Env, key: &DataKey) {
    e.storage()
        .persistent()
        .extend_ttl(key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
}

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

/// Lifecycle state of a dispute over an escrow.
///
/// The order is the progression: `Open` may become `Appealed`, and `Appealed`
/// may settle into `Resolved`. `Expired` is the alternative terminal state for
/// a dispute nobody acted on.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisputeStatus {
    /// Raised and awaiting a ruling. The escrow stays frozen.
    Open,
    /// A ruling was handed down and the escrow settled. Terminal.
    Resolved,
    /// The losing party escalated after a ruling. The escrow stays frozen.
    Appealed,
    /// Nobody acted before the deadline, so the dispute was closed out.
    /// Terminal.
    Expired,
}

/// A dispute raised over a single escrow.
///
/// While a dispute is `Open` or `Appealed` the underlying escrow must not be
/// released or refunded; that is the whole point of raising one.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeRecord {
    /// Who raised the dispute.
    pub claimant: Address,
    /// The escrow this dispute is over.
    pub escrow_id: u64,
    /// Current lifecycle state.
    pub status: DisputeStatus,
    /// Ledger at which the dispute was raised, for expiry and age checks.
    pub opened_ledger: u32,
    /// The arbiter assigned to settle it, if one has been set.
    pub resolver: Option<Address>,
}
