//! # aegis-routing
//!
//! The pure n→m liquidity-routing engine for Aegis. Given a graph of
//! `(chain, token)` assets connected by typed settlement rails — each priced by
//! a convex all-in [`cost`] curve — it finds the minimum-all-in-cost route for
//! any source→target pair, splits a trade across routes when convex price-impact
//! makes a split cheaper, and compiles the result into an executable leg
//! [`dag`].
//!
//! It is deliberately **pure**: no HTTP, database, Axum, SQLx, or Reqwest.
//! `apps/api` adapts its `ChainKey`/`TokenSpec`/`Config` into these value types
//! and implements [`provider::RouteProvider`]; the solver is rail-agnostic, so a
//! new venue or chain never touches the core.
//!
//! Module map:
//! - [`domain`] — `ChainId`, `Token`, `Asset` (node), `EdgeKind`, `ProviderId`.
//! - [`cost`] — `ValueUsd`, decomposed `EdgeCost`, the convex `CostCurve` trait.
//! - [`graph`] — `LiquidityGraph` + deterministic fingerprint, `GraphBuilder`.
//! - [`provider`] — the `RouteProvider` Strategy seam (open/closed).
//! - [`solver`] — Dijkstra route finder + SSP min-cost-flow splitter.
//! - [`dag`] — leg DAG with explicit deps + Kahn topological order.
//! - [`oracle`] — exhaustive brute-force optima for verification.

pub mod cost;
pub mod dag;
pub mod domain;
pub mod fixtures;
pub mod graph;
pub mod oracle;
pub mod provider;
pub mod solver;

pub use cost::{
    BridgeComponent, BridgeCurve, BucketedCurve, ConstProductCurve, CostCurve, CurveError,
    EdgeCost, ValueUsd,
};
pub use dag::{DagError, Leg, LegDag};
pub use domain::{Asset, ChainId, EdgeKind, ProviderId, Token};
pub use graph::{Edge, GraphBuilder, LiquidityGraph, NodeIdx};
pub use provider::{assemble, RouteProvider};
pub use solver::{
    find_route, min_cost_flow, FlowAllocation, FlowConfig, FlowPlan, Route, SolveError,
};
