use soroban_sdk::{contractevent, Address};

/// New tokens were credited to `to`.
///
/// Topics are `["mint", to: Address]` and the data is `[amount: i128]`, the
/// shape indexers subscribe to for a supply increase.
#[contractevent(data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mint {
    #[topic]
    pub to: Address,
    pub amount: i128,
}

/// Tokens moved from `from` to `to`.
///
/// Topics are `["transfer", from: Address, to: Address]` and the data is
/// `[amount: i128]`, the shape every SEP-41 wallet already understands.
#[contractevent(data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transfer {
    #[topic]
    pub from: Address,
    #[topic]
    pub to: Address,
    pub amount: i128,
}

/// `from` authorized `spender` to move up to `amount` until
/// `expiration_ledger`.
///
/// Topics are `["approve", from: Address, spender: Address]` and the data is
/// `[amount: i128, expiration_ledger: u32]`. The amount in the data is the
/// allowance's new total, not the delta, so an indexer replaying the log
/// reconstructs the current grant without replaying arithmetic.
#[contractevent(data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Approve {
    #[topic]
    pub from: Address,
    #[topic]
    pub spender: Address,
    pub amount: i128,
    pub expiration_ledger: u32,
}

/// `owner` cleared every approval they had granted.
///
/// Topics are `["allowances_revoked", owner: Address]` and the data is
/// `[count: u32]`. The count is in the event rather than only in the return
/// value so the sweep is auditable from the log, even for a caller that
/// discarded the return.
#[contractevent(data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllowancesRevoked {
    #[topic]
    pub owner: Address,
    pub count: u32,
}

/// A new escrow was created.
///
/// Topics are `["escrow_created", depositor: Address, beneficiary: Address]`
/// and the data is `[id: u32, token: Address, amount: i128, deadline_ledger:
/// u32]`. Both parties are topics so an operator can filter escrows by either
/// side without decoding the payload.
#[contractevent(data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowCreated {
    #[topic]
    pub depositor: Address,
    #[topic]
    pub beneficiary: Address,
    pub id: u32,
    pub token: Address,
    pub amount: i128,
    pub deadline_ledger: u32,
}
