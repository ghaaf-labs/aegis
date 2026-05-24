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

/// Economic class of a token, used by the route registry to decide which
/// adapter (and capability checks) a leg touching it must clear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenClass {
    /// USDC — the settlement unit; bridged via CCTP, never swapped.
    Stable,
    /// USYC — yield sleeve, minted/redeemed via the Hashnote Teller.
    Yield,
    /// EURC — FX sleeve. Economically an FX stablecoin, but now executed via
    /// the permissionless USDC/EURC pool on Base (the gated Arc StableFX rail
    /// is superseded). The route registry routes an `FxStable` token with a
    /// Base ERC-20 through the swap adapter.
    FxStable,
    /// BTC/ETH/SOL/… — market exposure acquired via a per-chain AMM swap.
    Volatile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ChainKey {
    Arc,
    Base,
    EthSepolia,
    ArbSepolia,
    AvaxFuji,
    OpSepolia,
}

impl ChainKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Arc => "arc",
            Self::Base => "base",
            Self::EthSepolia => "eth_sepolia",
            Self::ArbSepolia => "arb_sepolia",
            Self::AvaxFuji => "avax_fuji",
            Self::OpSepolia => "op_sepolia",
        }
    }
    /// Dense index into a per-chain collection (e.g. `Config`'s `[ChainConfig; 6]`).
    /// The ordering matches the variant declaration order and is the single
    /// source of truth for indexing per-chain config.
    pub fn index(self) -> usize {
        match self {
            Self::Arc => 0,
            Self::Base => 1,
            Self::EthSepolia => 2,
            Self::ArbSepolia => 3,
            Self::AvaxFuji => 4,
            Self::OpSepolia => 5,
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        // Accept both the canonical snake_case `as_str()` form and the
        // hyphenated Circle wallet `blockchain` form (e.g. "eth-sepolia"),
        // so a chain stamped by either layer round-trips.
        match s.to_ascii_lowercase().replace('-', "_").as_str() {
            "arc" => Some(Self::Arc),
            "base" => Some(Self::Base),
            "eth_sepolia" => Some(Self::EthSepolia),
            "arb_sepolia" => Some(Self::ArbSepolia),
            "avax_fuji" => Some(Self::AvaxFuji),
            "op_sepolia" => Some(Self::OpSepolia),
            _ => None,
        }
    }
    /// Circle CCTP V2 domain id. Source domain for the attestation URL.
    /// Mirrors `CHAIN_DOMAINS` in `packages/shared/src/constants.ts`.
    /// Verified against the deployed Arc testnet MessageTransmitter
    /// (`localDomain() = 26`); 13 was a stale guess that silently
    /// passed CI (no on-chain test) and surfaced only when an
    /// attested message reverted with "Invalid destination domain".
    pub fn domain_id(&self) -> u32 {
        match self {
            Self::EthSepolia => 0,
            Self::AvaxFuji => 1,
            Self::OpSepolia => 2,
            Self::ArbSepolia => 3,
            Self::Base => 6,
            Self::Arc => 26,
        }
    }

    /// Whether this chain is wired for live rebalance execution. The execution
    /// set is exactly the chains where a Circle wallet is provisioned
    /// (`wallet_routes::SUPPORTED_WALLET_BLOCKCHAINS`): Arc/Base run the full
    /// path (deployed RebalanceExecutor + swap venue); Eth/Arb/Avax are CCTP
    /// source/dest chains for the plain-USDC consolidation baseline. Membership
    /// here is necessary but not sufficient — the route registry then validates
    /// each leg's CCTP/swap config *per chain* (`adapters::cctp::capability_for_route`
    /// / `adapters::swap::capability_for`), so an execution chain still fails
    /// closed if its USDC/messenger/venue/executor is unset. OP-Sepolia is
    /// excluded: it has no provisioned wallet route, so funds can never land
    /// there. An unparsable / non-EVM chain also fails closed (`parse` → `None`).
    pub fn is_execution(&self) -> bool {
        matches!(
            self,
            Self::Arc | Self::Base | Self::EthSepolia | Self::ArbSepolia | Self::AvaxFuji
        )
    }
}

/// Symbols whose canonical residency is Arc. Anything not in here defaults to
/// Base for cross-chain planning. USYC lives on Arc (Hashnote Teller), and
/// USDC is multi-chain so it stays where the user holds it. EURC moved to Base
/// once the EUR sleeve switched to the permissionless USDC/EURC DEX pool.
pub const ARC_NATIVE_SYMBOLS: &[&str] = &["USYC"];

/// Symbols whose canonical residency is Base (Uniswap V3 / Aerodrome venue).
/// EURC settles here via the permissionless USDC/EURC pool (supersedes the
/// KYB-gated Arc StableFX rail).
pub const BASE_NATIVE_SYMBOLS: &[&str] = &[
    "BTC", "ETH", "SOL", "BNB", "AVAX", "MATIC", "LINK", "UNI", "EURC",
];

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
