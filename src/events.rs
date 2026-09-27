use soroban_sdk::{contractevent, Address, Bytes};

/// New tokens were credited to `to`.
///
/// Topics `["mint", to: Address]`, data `[amount: i128]`.
#[contractevent(data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mint {
    #[topic]
    pub to: Address,
    pub amount: i128,
}

/// `from` destroyed `amount` of their own tokens.
///
/// Topics `["burn", from: Address]`, data `[amount: i128]`.
#[contractevent(data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Burn {
    #[topic]
    pub from: Address,
    pub amount: i128,
}

/// Tokens moved from `from` to `to`.
///
/// Topics `["transfer", from: Address, to: Address]`, data `[amount: i128]`.
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
/// Topics are the standard `["transfer", from, to]` so a wallet that only parses
/// `transfer` still shows the movement, and the data is
/// `[amount: i128, memo: Bytes]`.
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

/// `from` authorized `spender` to move up to `amount` until
/// `expiration_ledger`.
///
/// Topics `["approve", from: Address, spender: Address]`, data
/// `[amount: i128, expiration_ledger: u32]`.
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

/// An admin recovered `amount` from `from`.
///
/// Topics `["clawback", admin: Address, from: Address]`, data
/// `[amount: i128]`. The admin is a topic so an auditor can filter clawbacks by
/// the account that performed them.
#[contractevent(data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Clawback {
    #[topic]
    pub admin: Address,
    #[topic]
    pub from: Address,
    pub amount: i128,
}
