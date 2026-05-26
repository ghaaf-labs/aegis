use chrono::{DateTime, Utc};
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// `ChainKey` and `TokenClass` moved to `crate::domain`; re-exported here so the
// many `rebalance::models::{ChainKey, TokenClass}` import paths keep working.
pub use crate::domain::{ChainKey, TokenClass};

/// Top-level rebalance lifecycle row. Mirrors `rebalances` (migration 0004).
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Rebalance {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub decision_id: Uuid,
    pub status: String,
    pub total_legs: i32,
    pub completed_legs: i32,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub total_gas_usdc: Option<Decimal>,
    pub failure_reason: Option<String>,
    pub approved_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct RebalanceLeg {
    pub id: Uuid,
    pub rebalance_id: Uuid,
    pub leg_index: i32,
    pub kind: String,
    pub src_chain: Option<String>,
    pub dest_chain: Option<String>,
    pub src_symbol: Option<String>,
    pub dest_symbol: Option<String>,
    #[serde(with = "rust_decimal::serde::float")]
    pub amount_usdc: Decimal,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub min_out: Option<Decimal>,
    pub status: String,
    pub tx_hash: Option<String>,
    pub cctp_message_hash: Option<String>,
    pub failure_reason: Option<String>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegKind {
    LocalSwap,
    CrossChainBurn,
    CrossChainMint,
    ParkUsyc,
    RedeemUsyc,
    FxStablefx,
}

impl LegKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LocalSwap => "local_swap",
            Self::CrossChainBurn => "cross_chain_burn",
            Self::CrossChainMint => "cross_chain_mint",
            Self::ParkUsyc => "park_usyc",
            Self::RedeemUsyc => "redeem_usyc",
            Self::FxStablefx => "fx_stablefx",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "local_swap" => Self::LocalSwap,
            "cross_chain_burn" => Self::CrossChainBurn,
            "cross_chain_mint" => Self::CrossChainMint,
            "park_usyc" => Self::ParkUsyc,
            "redeem_usyc" => Self::RedeemUsyc,
            "fx_stablefx" => Self::FxStablefx,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedLeg {
    pub leg_index: i32,
    /// Explicit DAG dependencies: leg_index values that must confirm before
    /// this leg can be dispatched. Within a CCTP transfer the mint depends
    /// on the burn; a post-bridge swap depends on the mint. Empty means no
    /// prerequisite (the leg can start immediately).
    pub deps: Vec<i32>,
    pub kind: LegKind,
    pub src_chain: Option<ChainKey>,
    pub dest_chain: Option<ChainKey>,
    pub src_symbol: Option<String>,
    pub dest_symbol: Option<String>,
    pub amount_usdc: Decimal,
    pub min_out: Option<Decimal>,
}

pub fn decimal_usd(amount: f64) -> Decimal {
    Decimal::from_f64(amount).unwrap_or(Decimal::ZERO)
}

#[derive(Debug, Clone, PartialEq)]
pub enum SellSources {
    /// Mock/offline path: no live chain-level token balance is known, so route a
    /// sell from the token's canonical execution chain.
    CanonicalFallback,
    /// Real wallet path: sell only from the chains where live wallet value exists.
    ByChain(std::collections::HashMap<ChainKey, f64>),
    /// Live route assessment froze this symbol because every known source failed
    /// a quote/balance safety check. Do not fall back to the canonical chain.
    Frozen,
}

impl SellSources {
    pub fn by_chain(values: std::collections::HashMap<ChainKey, f64>) -> Self {
        if values.is_empty() {
            Self::Frozen
        } else {
            Self::ByChain(values)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanInput {
    pub portfolio_value_usd: f64,
    /// Current allocation weights by symbol — what the user holds today.
    pub current_weights: std::collections::HashMap<String, f64>,
    /// Sell source model by symbol. Absence is interpreted as
    /// `CanonicalFallback`; explicit `Frozen` means route assessment removed all
    /// safe sources and the planner must not invent a canonical-chain sell.
    pub sell_sources: std::collections::HashMap<String, SellSources>,
    /// Target allocation weights by symbol — from `portfolios.goal.targetAllocation`.
    pub target_weights: std::collections::HashMap<String, f64>,
    /// Unified USDC available across chains (from Gateway).
    pub usdc_per_chain: std::collections::HashMap<ChainKey, f64>,
    /// Drift threshold below which we no-op. Default `REBALANCE_DRIFT_THRESHOLD`.
    pub drift_threshold: f64,
    /// USD value below which we treat a delta as dust and skip.
    pub dust_threshold_usd: f64,
    /// Recent prices (USD) for symbols that may be involved in swaps.
    /// Used by the planner to compute realistic min_out for cross-chain hook legs
    /// and local swaps in real execution mode.
    pub prices: std::collections::HashMap<String, f64>,
    /// Latest classified market regime ("risk_on" | "neutral" | "risk_off").
    /// Drives the asymmetric "let winners run" drift bands: `risk_on` widens
    /// the band for trimming winners (sells), `risk_off` tightens it to
    /// de-risk sooner. `None` ⇒ symmetric `drift_threshold` (neutral).
    pub regime: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cctp_domain_ids_match_circle() {
        // Circle CCTP V2 domains. A wrong id reverts on-chain with
        // "Invalid destination domain" (see the Arc=26 history above).
        assert_eq!(ChainKey::EthSepolia.domain_id(), 0);
        assert_eq!(ChainKey::AvaxFuji.domain_id(), 1);
        assert_eq!(ChainKey::OpSepolia.domain_id(), 2);
        assert_eq!(ChainKey::ArbSepolia.domain_id(), 3);
        assert_eq!(ChainKey::Base.domain_id(), 6);
        assert_eq!(ChainKey::Arc.domain_id(), 26);
    }

    #[test]
    fn chain_key_round_trips_both_string_forms() {
        for k in [
            ChainKey::Arc,
            ChainKey::Base,
            ChainKey::EthSepolia,
            ChainKey::ArbSepolia,
            ChainKey::AvaxFuji,
            ChainKey::OpSepolia,
        ] {
            assert_eq!(ChainKey::parse(k.as_str()), Some(k));
        }
        // Hyphenated Circle wallet `blockchain` form is also accepted.
        assert_eq!(ChainKey::parse("eth-sepolia"), Some(ChainKey::EthSepolia));
        assert_eq!(ChainKey::parse("OP-SEPOLIA"), Some(ChainKey::OpSepolia));
        assert_eq!(ChainKey::parse("solana"), None);
    }

    #[test]
    fn provisioned_wallet_chains_are_execution_chains() {
        // The execution set is exactly the provisioned wallet chains: Arc/Base
        // run the full path; Eth/Arb/Avax are CCTP source/dest chains.
        for k in [
            ChainKey::Arc,
            ChainKey::Base,
            ChainKey::EthSepolia,
            ChainKey::ArbSepolia,
            ChainKey::AvaxFuji,
        ] {
            assert!(k.is_execution(), "{k:?} must be an execution chain");
        }
        // OP-Sepolia has no provisioned wallet route, so it is never an
        // execution chain (funds can't land there).
        assert!(
            !ChainKey::OpSepolia.is_execution(),
            "OP-Sepolia is not provisioned, so it must not be executable"
        );
        assert_eq!(
            ChainKey::parse("solana"),
            None,
            "non-EVM chains fail closed"
        );
    }
}
