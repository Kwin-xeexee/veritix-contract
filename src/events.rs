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

/// An admin recovered `amount` from `from` without the holder spending it.
///
/// Topics are `["clawback", admin: Address, from: Address]` and the data is
/// `[amount: i128]`. The admin is a topic rather than a data field so an
/// auditor can filter clawbacks by the account that performed them.
#[contractevent(data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Clawback {
    #[topic]
    pub admin: Address,
    #[topic]
    pub from: Address,
    pub amount: i128,
}
