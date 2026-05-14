//! Trustability score — Sprint 4.
//!
//! Surfaces a single number per user: how reliably does the agent's
//! 24h-realized PnL beat its own counterfactual? Computed from the
//! `v_trustability_per_user` view (migration 0005) so the SQL stays in one
//! place — Rust just wraps the lookup and shapes the response.

pub mod handlers;
pub mod service;
