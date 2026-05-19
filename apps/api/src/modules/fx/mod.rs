//! FX — Arc StableFX integration for USDC ↔ EURC.
//!
//! Sprint 2: rate fetch only. On-chain swap lands with Sprint 3's
//! cross-chain executor. HS-6 (2026-05-17): the rate fetch is live —
//! pulls USDC + EURC spot from the platform price provider when
//! institutional StableFX access is unavailable (current default).

pub mod handlers;
pub mod prices;
pub mod service;
