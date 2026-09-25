#![no_std]
// The contract is being rebuilt module by module, so a few storage helpers
// land ahead of the contract entry points that will eventually call them.
#![allow(dead_code)]

// Host-side tests need to observe state after a rejected call, which means
// catching the panic a guard raises, and they build expected topic lists with
// `std::vec!`. The crate is `no_std` so the contract never links std, but the
// test build links it here.
#[cfg(test)]
extern crate std;

mod admin;
mod authorization;
mod balance;
mod contract;
mod events;
mod metadata;
mod storage_types;
mod validation;
