//! Wallet module — Circle W3S User-Controlled Wallets integration.
//!
//! Signup flow (4 hops):
//!
//! 1. `POST /auth/wallet/create { email }` — server creates Circle user
//!    record, issues a `UserTokenBundle` and an initialize-challenge ID for
//!    the PIN ceremony, sets the JWT session cookie.
//! 2. Browser instantiates `@circle-fin/w3s-pw-web-sdk` with the bundle and
//!    runs `sdk.execute(challengeId)` — user sets their PIN.
//! 3. SDK signs the wallet-creation request; Circle provisions wallets on
//!    ARC-TESTNET and BASE-SEPOLIA.
//! 4. Browser polls `GET /auth/wallet/status` until the wallet IDs +
//!    addresses come back; server persists them to the `users` row and
//!    emits an SSE `wallet.created` event.
//!
//! Login flow is identical except `is_new_user=false` and `challenge_id=None`
//! (the SDK just authenticates the fresh token; no PIN re-setup).
//!
//! `MockProvider` returns deterministic synthetic data so local dev works
//! without hitting Circle when `MOCK_CIRCLE=true`.

pub mod handlers;
pub mod models;
pub mod provider;
pub mod service;
pub mod sse;

#[allow(unused_imports)]
pub use models::{InitWalletRequest, UserTokenBundle, WalletInfo, WalletStatusResponse};
#[allow(unused_imports)]
pub use provider::{CircleProvider, MockProvider, WalletProvider};
#[allow(unused_imports)]
pub use service::WalletService;
