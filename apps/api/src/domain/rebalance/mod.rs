//! Canonical rebalance domain primitives, shared by the agent, planner, and
//! executor so value, target, and executability cannot drift apart.
//!
//! These are *pure* types (no IO) — the modules layer builds them from live
//! balances/prices. `ValueUsd` is the first piece: the single source of value
//! (INV-1/INV-2). `PortfolioState`, `TargetIntent`, and the snapshot follow as
//! later phases adopt the model.

mod value;

pub use value::ValueUsd;
