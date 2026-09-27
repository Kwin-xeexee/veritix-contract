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

/// `from` destroyed `amount` of their own tokens.
///
/// Topics are `["burn", from: Address]` and the data is `[amount: i128]`.
#[contractevent(data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Burn {
    #[topic]
    pub from: Address,
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
