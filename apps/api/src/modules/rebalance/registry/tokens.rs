//! The token registry moved to `crate::domain::token` (the one shared table,
//! below `modules`, so `prices` and `rebalance` both depend on it). This shim
//! re-exports it so the existing `rebalance::registry::tokens::*` import paths
//! (symbol consts, `TokenSpec`, `TOKEN_REGISTRY`, `token`, `is_real_addr`, …)
//! keep compiling unchanged.

pub use crate::domain::token::*;
