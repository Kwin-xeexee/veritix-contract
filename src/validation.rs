/// Largest memo accepted by a memo-carrying transfer, in bytes.
///
/// Memos are opaque to the contract — only a ticketing order reference is ever
/// stored — but they are paid for by every reader of the ledger, so the size is
/// capped rather than left to the ledger's limits. 64 bytes is enough for a
/// reference plus a short JSON payload while keeping the data entry small
/// enough that indexing stays cheap.
pub const MAX_MEMO_BYTES: u32 = 64;

/// Panics unless `amount` is strictly positive.
///
/// Amount validation is repeated across the token, escrow, splitter and
/// recurring paths. Routing every one of them through a single helper keeps the
/// rejection message byte-for-byte identical, which is what indexers and
/// clients match on when they classify a failed transaction.
///
/// The `-> ()` return type is written out on purpose: this is a guard, so the
/// only failure channel is the panic, and spelling that out in the signature
/// keeps callers from expecting a value back.
///
/// # Panics
///
/// Panics with an `InvalidAmount` message naming `amount` when `amount <= 0`.
/// Zero is rejected as well: every caller in this contract either credits or
/// debits a ledger, and a zero movement would burn gas to change nothing.
pub fn require_positive_amount(amount: i128) -> () {
    assert!(
        amount > 0,
        "InvalidAmount: amount must be strictly positive, got {}",
        amount
    );
}

/// Panics when `memo` is longer than [`MAX_MEMO_BYTES`].
///
/// # Panics
///
/// Panics with a `MemoTooLarge` message reporting both the actual length and the
/// limit, so a client that overshoots can see by how much without reading the
/// contract source.
pub fn require_memo_within_limit(memo_len: u32) -> () {
    assert!(
        memo_len <= MAX_MEMO_BYTES,
        "MemoTooLarge: memo is {} bytes, limit is {}",
        memo_len,
        MAX_MEMO_BYTES
    );
}
