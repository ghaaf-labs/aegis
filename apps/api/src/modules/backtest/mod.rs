//! Backtest preview module — Sprint 4.
//!
//! Replays the last N days of `market_snapshots` with the user's current
//! allocation **and** the strategist's proposed allocation, then surfaces
//! delta PnL, Sharpe ratio, and max-drawdown so the user sees "would this
//! recommendation have helped over the recent past?" inline on the
//! approval modal.
//!
//! The backtest is intentionally lightweight: 30 daily snapshots,
//! buy-and-hold of each weight set, no rebalancing within the window. It's
//! a quick sanity check for the strategist's proposal — not a substitute
//! for proper backtesting on real fills.

pub mod handlers;
pub mod service;

#[allow(unused_imports)]
pub use service::{run_backtest, BacktestResult};
