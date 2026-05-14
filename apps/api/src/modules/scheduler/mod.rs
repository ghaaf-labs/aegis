//! Proactive scheduler — the difference between "an AI that answers" and
//! "an autonomous agent." One Tokio task per active portfolio polls drift,
//! regime, and harvest signals every `SCHEDULER_TICK_SECS` (default 300s)
//! and triggers `analyze_portfolio` when any threshold is breached.

pub mod outcome_compressor;
pub mod tick;

pub use outcome_compressor::spawn_outcome_compressor;
pub use tick::spawn_portfolio_scheduler;
