//! Self-hosted analytics — events land in our own `analytics_events` table.
//! No third-party SDK, no PostHog, no Cloud. Traction queries live in
//! `docs/queries/traction.sql`.

pub mod handlers;
pub mod service;

#[allow(unused_imports)]
pub use service::emit;
