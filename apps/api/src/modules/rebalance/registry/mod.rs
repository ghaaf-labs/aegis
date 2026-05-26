//! Execution route registry — the single source of truth for which routes,
//! tokens, and chains are genuinely executable.
//!
//! Consulted by the AI agent (before recommending), the planner, the approval
//! gate, and the executor. A route is executable only when it has a token
//! address, a live adapter, a configured signer, the required cargo feature,
//! and a fresh validated quote. Anything missing fails closed — it is never
//! faked. See `docs/03-circle-stack.md` for the Circle-product mapping.

pub mod capabilities;
pub mod route;
pub mod ticket;
pub mod tokens;

pub use capabilities::{AdapterCapability, RuntimeCapabilities};
pub use route::{
    allocation_target_symbols, designable_allocation_symbols, executable_chain_for_token,
    executable_token_symbols, route_state_for_token, validate_legs, BlockerCode, RouteBlocker,
    RouteState,
};
pub use ticket::ExecutionTicket;
pub use tokens::{token, TokenSpec, TOKEN_REGISTRY};
