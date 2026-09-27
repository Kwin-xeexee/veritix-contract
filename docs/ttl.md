TTL policy
Soroban entries expire. An entry that is never touched eventually falls below the network's minimum TTL and is archived, after which reading it returns nothing — the contract cannot tell "never existed" from "expired", and a balance that reads as zero is indistinguishable from a drained account.

Two helpers in src/storage_types.rs implement the policy. Both are the only supported way to extend an entry's life.

Helper	Applies to	Threshold	Extends by
bump_instance(e)	Instance storage	INSTANCE_LIFETIME_THRESHOLD (518,400 ledgers, ~30 days)	INSTANCE_BUMP_AMOUNT (2,592,000 ledgers, ~15 days)
bump_persistent(e, &key)	One persistent entry	PERSISTENT_LIFETIME_THRESHOLD (2,073,600 ledgers, ~120 days)	PERSISTENT_BUMP_AMOUNT (4,752,000 ledgers, ~180 days)
Ledger figures assume Stellar's 5-second ledger close.

When to call them
bump_instance at the top of any entry point that reads or writes instance storage — in practice every administrative function, since the admin key and the supply counters live there. An archived instance is more serious than an archived balance: the contract does not exist at all until it is restored.
bump_persistent on every read and every write of a persistent key, not only on writes.
That second point is the one most easily got wrong. Bumping only on writes looks sufficient until you consider a long-dormant escrow: nothing writes to it between creation and release, so at release time the entry is already archived and the contract reads a missing record. A read that returns a live value is exactly the case where the owner still cares, which is why reads bump too.

Adding a key
Add the variant to DataKey in src/storage_types.rs, grouped by concern.
Decide instance or persistent. If the number of entries grows with usage, it is persistent.
Update the key reference table above.
Use bump_persistent on every read and write of the new key, or bump_instance if it is instance storage.
If the key moves or renames an existing value, that is an upgrade-guide concern, not a feature — see docs/upgrade-guide.md.
