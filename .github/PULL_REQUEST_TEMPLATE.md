# Pull request

## Summary

<!-- What this change does and why. Two or three sentences is enough. -->

Closes #

## Linked issues

<!-- List every issue this PR closes, one per line, as `Closes #N`.
     GitHub only auto-closes an issue when the PR that references it is
     merged into the default branch. -->

Closes #

## Type of change

- [ ] Bug fix
- [ ] New feature
- [ ] Refactor with no behaviour change
- [ ] Documentation
- [ ] Build, CI, or tooling

- [ ] 

## What changed

<!-- File-by-file, or behaviour-by-behaviour. Call out anything that changes
     storage layout, the public interface, or authorisation. -->

## Security and authorisation

<!-- Who can call the new entry points, and what guards protect them.
     Required for anything touching escrow, clawback, admin, or fees. -->

## Storage and TTL

<!-- New `DataKey` variants and the TTL strategy for each. State explicitly if
     a key layout change requires an upgrade-guide entry. -->

## Interface changes

<!-- New or changed public entry points. Breaking changes need the upgrade
     guide updated in the same PR. -->

## Testing

- [ ] Unit tests added for the new behaviour
- [ ] Negative tests cover the failure paths
- [ ] `cargo test` passes
- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes
- [ ] `cargo build --release --target wasm32-unknown-unknown` passes and the
      artifact is under the Soroban size limit

## Screenshots or logs

<!-- Test output or a transaction result, where it helps. -->

## Checklist

- [ ] I did not add a second `DataKey` enum or a duplicate storage key.
- [ ] I did not add a blanket `#![allow(dead_code)]`; unused code is removed.
- [ ] I added any new module declaration to `src/lib.rs`, with test modules
      behind `#[cfg(test)]`.
- [ ] I checked the new entry points against `docs/abi-reference.md`.
