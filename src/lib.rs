//! Veritix Pay — the on-chain payment module for the Veritix ticketing
//! platform, built on Soroban (Stellar).
//!
//! # Crate layout
//!
//! This crate is `no_std`: a Soroban contract is compiled to wasm and has no
//! access to an operating system, so every dependency is built without the
//! standard library. `alloc` is deliberately not used either, which keeps the
//! contract inside the Soroban wasm size limit.
//!
//! Each functional area lives in its own module and is re-exported from
//! `contract`, which is the only public entry surface. Storage keys live in
//! exactly one place — `storage_types` — so that two modules can never
//! disagree about where a value is kept.
//!
//! # Adding a module
//!
//! 1. Create `src/your_module.rs`.
//! 2. Declare it below with `mod your_module;`.
//! 3. Add any new `DataKey` variants to `storage_types.rs`.
//! 4. Add tests in `src/your_module_test.rs` and declare that module behind
//!    `#[cfg(test)]`.
//! 5. Expose the public entry points from `contract.rs`.
//!
//! Do not add a blanket `#![allow(dead_code)]`. Unused code is removed, not
//! silenced.

#![no_std]

// The wasm artifact has no standard library: it is linked for
// `wasm32v1-none`, where there is no `std` and no unwinding runtime. The unit
// tests, however, run on the host, and a `#![no_std]` crate built for a host
// target has no panic handler to link against. Pulling in `std` for test
// builds only gives those builds one. This is test-only — `std` is never
// linked into the deployed contract.
#[cfg(test)]
extern crate std;

// `soroban-sdk` supplies the wasm `#[panic_handler]`, so this crate must not
// declare a second one — two handlers in one link are a duplicate `panic_impl`
// lang item. But a handler is mandatory for a `no_std` wasm module, and the
// SDK's is only linked once something in the crate actually references it. The
// crate root is empty until the first module lands, so reference the SDK here
// to keep its panic handler present at every point in the build. This is an
// anonymous import: it pulls the dependency in without binding a name, and
// introduces no unused-import warning.
use soroban_sdk as _;

// The SDK's handler is gated on `target_family = "wasm"`, so the host library
// target still has no handler at all. A `#![no_std]` crate built for a host
// target therefore cannot be built directly, which is why tooling lints the
// contract with `cargo clippy --lib --target wasm32v1-none` and lints the tests
// with `cargo clippy --tests` on the host, where `extern crate std` above
// supplies a handler. See .github/workflows/ci.yml.

// Module declarations are added here as each module lands in the backlog, so
// that every link in the stack compiles on its own. Test modules stay behind
// `#[cfg(test)]` and are never compiled into the wasm artifact.
//
// `pub mod`, per CONTRIBUTING.md. It also keeps `dead_code` honest: helpers
// land here before the modules that call them, and a private module would
// report them as unused the moment they are added. On a `cdylib` contract
// crate `pub` does not widen the wasm export surface — only `#[contractimpl]`
// entry points are callable on-chain.
pub mod admin;
pub mod contract;
pub mod dispute;
pub mod storage_types;

#[cfg(test)]
mod admin_test;
#[cfg(test)]
mod dispute_test;
