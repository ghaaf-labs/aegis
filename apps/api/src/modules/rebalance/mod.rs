//! Cross-chain rebalance execution (Sprint 3).
//!
//! The planner is pure; the executor orchestrates a plan, dispatching legs
//! (`local_swap`, `cross_chain_burn`, `cross_chain_mint`, `park_usyc`,
//! `redeem_usyc`, `fx_stablefx`), updating DB state, and broadcasting
//! per-leg SSE events. Handlers expose the user-facing
//! `plan → review → execute → poll` flow.

pub mod adapters;
pub mod cross_chain;
pub mod defensive_plan;
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

/// Shared sell-balance haircut used before planning/executing token sells.
/// Keeping the planning-time assessment and dispatch-time clamp on one constant
/// prevents a plan from passing live checks with one spend floor and failing at
/// submit with another.
pub const LIVE_TOKEN_SPEND_MARGIN_BPS: u32 = 9_950;

#[allow(unused_imports)]
pub use models::{ChainKey, LegKind, PlanInput, PlannedLeg, Rebalance, RebalanceLeg, TokenClass};
