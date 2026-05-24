//! Platform domain vocabulary shared across every layer (config, prices,
//! rebalance, agent): the settlement chains and the canonical token table.
//!
//! This module sits *below* `modules`, so both `modules::prices` (a leaf) and
//! `modules::rebalance` can depend on the one token table without a cyclic
//! module dependency. The previous homes (`rebalance::models` for `ChainKey`/
//! `TokenClass`, `rebalance::registry::tokens` for the registry) re-export from
//! here, so existing import paths keep compiling.

pub mod chain;
pub mod token;

pub use chain::ChainKey;
pub use token::TokenClass;
