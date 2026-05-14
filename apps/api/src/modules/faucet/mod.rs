//! USDC testnet faucet — proxies Circle's faucet for the authenticated
//! wallet, with a per-wallet 24h rate limit enforced in our DB.
//!
//! In mock mode (`MOCK_CIRCLE=true`) the faucet just logs the intent and
//! returns success — lets the demo work without a live testnet.

pub mod handlers;
pub mod service;
