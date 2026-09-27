use soroban_sdk::{contractevent, Address, Bytes};

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

/// Tokens moved from `from` to `to` carrying an opaque memo.
///
/// Topics are the same `["transfer", from, to]` as [`Transfer`] so a wallet
/// still recognizes the movement, and the data is `[amount: i128,
/// memo: Bytes]`. Reusing the `transfer` topic rather than inventing a new one
/// is deliberate: a memo is extra information about a transfer, not a
/// different kind of transfer, and a wallet that only knows `transfer` should
/// still show the ticket moving.
#[contractevent(data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferWithMemo {
    #[topic]
    pub from: Address,
    #[topic]
    pub to: Address,
    pub amount: i128,
    pub memo: Bytes,
}
