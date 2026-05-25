//! Cross-chain rebalance execution (Sprint 3).
//!
//! The planner is pure; the executor orchestrates a plan, dispatching legs
//! (`local_swap`, `cross_chain_burn`, `cross_chain_mint`, `park_usyc`,
//! `redeem_usyc`, `fx_stablefx`), updating DB state, and broadcasting
//! per-leg SSE events. Handlers expose the user-facing
//! `plan → review → execute → poll` flow.

pub mod adapters;
pub mod cross_chain;
pub mod executor;
pub mod handlers;
pub mod models;
pub mod planner;
pub mod quote;
pub mod registry;
pub mod reservations;
pub mod route_assessment;
pub mod routing;
pub mod snapshot;

#[allow(unused_imports)]
pub use models::{ChainKey, LegKind, PlanInput, PlannedLeg, Rebalance, RebalanceLeg, TokenClass};
