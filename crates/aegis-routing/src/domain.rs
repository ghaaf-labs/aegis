//! Pure value types for the routing graph. These are the crate's own identity
//! types — apps/api maps its `ChainKey` / `Symbol` onto them, so the engine
//! never depends on `Config`, SQLx, or Axum.

use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Serialize an `Arc<str>` newtype as its plain string.
fn ser_str<S: Serializer>(s: &str, ser: S) -> Result<S::Ok, S::Error> {
    ser.serialize_str(s)
}
fn de_arc_str<'de, D: Deserializer<'de>>(de: D) -> Result<Arc<str>, D::Error> {
    let s = String::deserialize(de)?;
    Ok(Arc::from(s.as_str()))
}

/// A settlement chain, identified by its stable CCTP domain id (Arc = 26,
/// Base = 6, …). A plain `u32` newtype so the crate carries no chain enum of
/// its own; apps/api supplies the real id from `ChainKey::domain_id()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ChainId(pub u32);

/// A token symbol (e.g. "USDC", "ETH"). `Arc<str>` keeps node-key clones cheap
/// while staying `Ord`/`Hash` for deterministic map ordering. This is a typed
/// identity, never a stringly-typed money value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Token(#[serde(serialize_with = "ser_str", deserialize_with = "de_arc_str")] Arc<str>);

impl Token {
    pub fn new(symbol: &str) -> Self {
        Self(Arc::from(symbol))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Token {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

/// A graph node: one token on one chain. `(chain, token)` is the full identity,
/// so the same symbol on two chains is two distinct nodes — which is what makes
/// cross-chain routing a first-class path rather than a special case.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Asset {
    pub chain: ChainId,
    pub token: Token,
}

impl Asset {
    pub fn new(chain: ChainId, token: impl Into<Token>) -> Self {
        Self {
            chain,
            token: token.into(),
        }
    }
}

/// The settlement rail an edge represents. Rails are edges (spec §7.1): a
/// BTC→ETH cross-chain rebalance is just the path BTC→USDC · `CctpStandard` ·
/// USDC→ETH — USDC is a hub *node*, never a hard-coded funnel.
///
/// Only rails that are actually emitted today are modelled. New rails (Gateway
/// reserve moves, CCTP Fast finality, …) are added as a new [`crate::RouteProvider`]
/// plus the variant they emit — by design a near-zero-cost extension, so the
/// taxonomy stays free of unemitted/dead variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Same-chain swap on a DEX venue (Uniswap V3 / Aerodrome).
    AmmSwap,
    /// CCTP V2 burn+mint, standard finality (~13min, free).
    CctpStandard,
    /// Subscribe idle USDC into the USYC yield sleeve (Hashnote Teller).
    UsycSubscribe,
    /// Redeem USYC back to USDC.
    UsycRedeem,
}

/// Stable identifier for the provider that supplied an edge (e.g. "uniswap_v3",
/// "cctp_v2"). Used in the deterministic graph fingerprint and in plan audit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProviderId(
    #[serde(serialize_with = "ser_str", deserialize_with = "de_arc_str")] Arc<str>,
);

impl ProviderId {
    pub fn new(id: &str) -> Self {
        Self(Arc::from(id))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ProviderId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}
