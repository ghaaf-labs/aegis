//! Library entry point — re-exports the same modules `main.rs` uses so
//! auxiliary binaries (e.g. `regime_backtest`) can reuse the Config, Db,
//! AppState wiring, modules and middleware without duplicating code.

pub mod config;
pub mod db;
pub mod env;
pub mod error;
pub mod middleware;
pub mod modules;
pub mod router;
