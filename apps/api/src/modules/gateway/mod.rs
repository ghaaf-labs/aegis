//! Gateway — Circle's unified USDC balance across Arc + Base.
//!
//! Sprint 2: a ticker polls Circle Gateway for every authenticated wallet's
//! unified balance and broadcasts a `gateway.balance` SSE event. UI shows
//! one big USDC number summed across chains.

pub mod handlers;
pub mod service;
pub mod ticker;

pub use ticker::spawn_balance_ticker;
