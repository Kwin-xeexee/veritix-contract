---
name: Feature request
about: Propose new contract functionality
title: "feat: "
labels: ["enhancement", "needs triage"]
assignees: ""
---

## Problem

<!-- What cannot be done today, and who is blocked by it. Describe the gap
     rather than the solution. -->

## Proposed change

<!-- What you would like the contract to be able to do. -->

## Which module

<!-- escrow, recurring, splitter, dispute, token, freeze, allowance, admin,
     balance, metadata, storage, or cross-cutting. -->

## Interface sketch

<!-- The proposed public entry points and their signatures. This is not a
     binding design, but it makes review much faster. -->

```rust
pub fn your_entrypoint(e: Env, arg: Address) -> Result<u32, Error>;
```

## Storage impact

<!-- New `DataKey` variants, TTL strategy, and whether the change affects an
     existing storage layout. Anything that moves or renames a key is an
     upgrade-guide concern, not a feature. -->

## Security and compatibility

<!-- Who is authorised to call this, what invariants must hold, and does it
     change behaviour that deployed contracts already rely on? -->

## Alternatives considered

<!-- What you rejected, and why. -->

## Checklist

- [ ] I searched the open issues and this is not a duplicate.
- [ ] I have read the relevant section of `docs/architecture.md`.
- [ ] This is on-chain contract behaviour, not an off-chain or frontend concern.
