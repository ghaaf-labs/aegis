//! Wallet module — email-code identity plus Circle wallet provisioning.
//!
//! Public auth flow:
//!
//! 1. `POST /auth/email/start { email }` — server sends a short-lived
//!    verification code. Localhost mock runs may return `devCode` for tests.
//! 2. `POST /auth/email/verify { challengeId, code, consent }` —
//!    server creates or restores the account, opens a session cookie, and
//!    returns wallet readiness.
//! 3. `GET /auth/session` is the authenticated gate and retries pending wallet
//!    provisioning before responding.
//!
//! `MockProvider` returns deterministic synthetic data so local dev works when
//! `MOCK_CIRCLE=true`.

pub mod circle_exec;
pub mod handlers;
pub mod models;
pub mod provider;
pub mod reconciler;
pub mod service;
pub mod sse;

#[allow(unused_imports)]
pub use models::WalletInfo;
#[allow(unused_imports)]
pub use provider::{CircleProvider, MockProvider, WalletProvider};
#[allow(unused_imports)]
pub use service::WalletService;
