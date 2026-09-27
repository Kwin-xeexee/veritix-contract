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

/// `amount` of escrow `id` was paid out to its beneficiary.
///
/// Topics are `["escrow_released", beneficiary: Address]` and the data is
/// `[id: u32, amount: i128, remaining: i128]`.
///
/// One event covers both full and partial settlement, and `remaining` is what
/// tells them apart: it is `0` on the call that closes the escrow and positive
/// on every call before it. A watcher therefore needs one rule — "settled when
/// `remaining` is 0" — instead of one per entry point, and a partial release is
/// still visible to anything tracking what the beneficiary has received.
#[contractevent(data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowReleased {
    #[topic]
    pub beneficiary: Address,
    pub id: u32,
    pub amount: i128,
    pub remaining: i128,
}

/// Escrow `id` was cancelled and `amount` returned to its depositor.
///
/// Topics are `["escrow_refunded", depositor: Address]` and the data is
/// `[id: u32, amount: i128]`. A refund is always terminal, so there is no
/// `remaining` to report: the escrow is closed by definition.
#[contractevent(data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowRefunded {
    #[topic]
    pub depositor: Address,
    pub id: u32,
    pub amount: i128,
}
