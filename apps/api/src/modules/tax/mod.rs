//! Tax-loss harvesting.
//!
//! Sprint 3 deliverable: surface every open lot that is currently sitting at
//! an unrealized loss, ordered FIFO. The strategist reads this signal and
//! decides whether to recommend realizing it. Wash-sale logic is a US-tax
//! concept that is explicitly out of scope for the hackathon.
//!
//! Per-user safety: every query joins through `allocations` → `portfolios`
//! → `users`. Callers must pass `user_id` (extracted from JWT) — the module
//! never trusts an unauthenticated portfolio_id.

pub mod export;
pub mod fifo;
pub mod handlers;
pub mod models;
pub mod service;

#[allow(unused_imports)]
pub use models::{HarvestableLoss, HarvestableLot};
