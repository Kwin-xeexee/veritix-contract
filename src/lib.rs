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

// A Soroban contract is a `no_std` wasm module with no unwinding runtime, so it
// must supply its own panic handler. Hitting it executes `unreachable`, which
// traps the contract and reverts the whole transaction — the intended outcome
// for a failed call, and the reason a partial state change can never persist.
//
// Only the wasm target gets a handler, and that is deliberate. Defining one
// forces `panic = "abort"` semantics, which cannot be reconciled with the
// unwinding dev profile, and a host-side handler would collide with the one
// `std` provides under `cfg(test)`. The consequence for tooling is that the
// host library target is never built directly: lint the contract with
// `cargo clippy --lib --target wasm32v1-none` and lint the tests with
// `cargo clippy --tests`. See .github/workflows/ci.yml.
#[cfg(target_family = "wasm")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

// Module declarations are added here as each module lands in the backlog, so
// that every link in the stack compiles on its own. Test modules stay behind
// `#[cfg(test)]` and are never compiled into the wasm artifact.
