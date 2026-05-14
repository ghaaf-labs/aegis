//! Daily digest via Resend.
//!
//! Per-user per-portfolio summary delivered at `DIGEST_HOUR_UTC` each day:
//! current regime, decisions in last 24h, harvestable losses, what the agent
//! would do today. Unsubscribe is a signed HMAC token — no auth required.

pub mod handlers;
pub mod service;

pub use service::spawn_digest_worker;
