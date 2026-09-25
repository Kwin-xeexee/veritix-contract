#![no_std]
// The contract is being rebuilt module by module, so a few storage helpers
// land ahead of the contract entry points that will eventually call them.
#![allow(dead_code)]

// The host-side tests build expected event topic lists with `std::vec!`. The
// crate is `no_std` so the contract never links std, but the test build does.
#[cfg(test)]
extern crate std;

mod admin;
mod balance;
mod contract;
mod control;
mod events;
mod metadata;
mod storage_types;
mod validation;
