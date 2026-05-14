//! Treasury — Circle USYC (Hashnote tokenized US T-bills) integration.
//!
//! Module name is `treasury` because `yield` is a Rust reserved keyword.
//!
//! Sprint 2 ships rate fetch + agent prompt integration. Actual park/redeem
//! is logged-only; on-chain execution waits for Sprint 3.

pub mod handlers;
pub mod service;
