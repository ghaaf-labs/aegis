use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    pub total_gas_usdc: Option<f64>,
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
    pub amount_usdc: f64,
    pub min_out: Option<f64>,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ChainKey {
    Arc,
    Base,
}

impl ChainKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Arc => "arc",
            Self::Base => "base",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "arc" | "ARC" => Some(Self::Arc),
            "base" | "BASE" => Some(Self::Base),
            _ => None,
        }
    }
    /// Circle CCTP V2 domain id. Source domain for the attestation URL.
    /// Mirrors `CHAIN_DOMAINS` in `packages/shared/src/constants.ts`.
    pub fn domain_id(&self) -> u32 {
        match self {
            Self::Arc => 13,
            Self::Base => 6,
        }
    }
}

/// Symbols whose canonical residency is Arc. Anything not in here defaults to
/// Base for cross-chain planning. USYC and EURC live on Arc (StableFX), and
/// USDC is multi-chain so it stays where the user holds it.
pub const ARC_NATIVE_SYMBOLS: &[&str] = &["USYC", "EURC"];

/// Symbols whose canonical residency is Base (Uniswap V3 venue).
pub const BASE_NATIVE_SYMBOLS: &[&str] =
    &["BTC", "ETH", "SOL", "BNB", "AVAX", "MATIC", "LINK", "UNI"];

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedLeg {
    pub leg_index: i32,
    pub kind: LegKind,
    pub src_chain: Option<ChainKey>,
    pub dest_chain: Option<ChainKey>,
    pub src_symbol: Option<String>,
    pub dest_symbol: Option<String>,
    pub amount_usdc: f64,
    pub min_out: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanInput {
    pub portfolio_value_usd: f64,
    /// Current allocation weights by symbol — what the user holds today.
    pub current_weights: std::collections::HashMap<String, f64>,
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
}
