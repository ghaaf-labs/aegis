//! Wallet module — Circle Wallets (WaaS) integration.
//!
//! Every user authenticates by creating or signing-in to a Circle Wallet.
//! Two ceremonies are supported:
//!
//! - **Passkey (WebAuthn)** — primary path. Browser does the credential
//!   ceremony; we forward to Circle WaaS to bind the credential to a wallet.
//! - **Email-OTP** — fallback. Circle WaaS emails a 6-digit code; we verify.
//!
//! Both terminate in the same `WalletInfo { wallet_id, arc_address,
//! base_address }` and the same JWT shape (`Claims { sub, wallet_id }`).
//!
//! The provider is behind a trait so a `MockProvider` keeps local dev moving
//! when the sandbox is flaky. `Config::circle_mock` (env `MOCK_CIRCLE=true`)
//! flips between Mock and Live.

pub mod handlers;
pub mod models;
pub mod provider;
pub mod service;
pub mod sse;

#[allow(unused_imports)]
pub use models::{LoginPasskeyRequest, OtpVerifyRequest, RegisterPasskeyRequest, WalletInfo};
#[allow(unused_imports)]
pub use provider::{CircleProvider, MockProvider, WalletProvider};
#[allow(unused_imports)]
pub use service::WalletService;
