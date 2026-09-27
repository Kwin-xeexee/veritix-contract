#![no_std]
// The contract is being rebuilt module by module, so a few storage helpers
// land ahead of the contract entry points that will eventually call them.
#![allow(dead_code)]

// The host-side test modules build expected event topic lists with `std::vec!`
// and read raw XDR. The crate is `no_std` so the contract never links std, but
// the test build does.
#[cfg(test)]
extern crate std;

#[cfg(test)]
mod event_test;
#[cfg(test)]
mod sep41_test;

mod admin;
mod allowance;
mod balance;
mod contract;
mod control;
mod events;
mod metadata;
mod storage_types;
mod validation;
