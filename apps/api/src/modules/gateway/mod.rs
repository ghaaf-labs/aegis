//! Gateway — Circle's unified USDC balance across Arc + Base.
//!
//! Sprint 2: a ticker polls Circle Gateway for the authenticated wallet's
//! unified balance and broadcasts a `gateway.balance` SSE event. UI shows
//! one big USDC number summed across chains.

pub mod handlers;
pub mod service;
