//! Execution adapters — the boundary between a validated `ExecutionTicket` and
//! the outside world (CCTP V2 bridge, per-chain swap, USYC Teller, Arc
//! StableFX). Each adapter knows its own capability and, in real mode, returns
//! a [`RealReceipt`] from a genuine on-chain transaction — never a synthetic
//! hash. Mock receipts ([`MockReceipt`]) exist only for the opt-in mock mode
//! used by tests/CI and are produced by a clearly-named helper here, not by any
//! real execution path.

pub mod cctp;
pub mod stablefx;
pub mod swap;
pub mod usyc;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::models::LegKind;

/// Receipt for a real, on-chain transaction. Only adapters' real paths build
/// this, and they only do so after submitting a transaction.
#[derive(Debug, Clone, PartialEq)]
pub struct RealReceipt {
    pub tx_hash: String,
    pub cctp_message_hash: Option<String>,
}

/// Receipt for a simulated transaction in opt-in mock mode. Distinct type from
/// [`RealReceipt`] so a mock hash cannot be returned where a real one is
/// expected.
#[derive(Debug, Clone, PartialEq)]
pub struct MockReceipt {
    pub tx_hash: String,
}

/// Deterministic simulated tx hash for mock mode. Reachable only when
/// `RuntimeCapabilities::real_mode` is false.
pub fn mock_receipt(kind: LegKind, leg_id: Uuid) -> MockReceipt {
    let mut h = Sha256::new();
    h.update(b"mock:");
    h.update(kind.as_str().as_bytes());
    h.update(b":");
    h.update(leg_id.as_bytes());
    MockReceipt {
        tx_hash: format!("0x{}", hex::encode(h.finalize())),
    }
}
