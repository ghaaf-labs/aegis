//! `ChainKey` — the canonical settlement-chain enum used across config
//! indexing, CCTP, wallet routes, the route registry, and SSE. It lives in
//! `domain` (below `modules`) so the shared token table and `Config` can depend
//! on it without reaching into `rebalance`. `rebalance::models` re-exports it,
//! so existing `rebalance::models::ChainKey` import paths keep working.

use serde::{Deserialize, Serialize};

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
    /// Every variant, in declaration (index) order — for iterating per-chain
    /// env/config without a separate crate.
    pub const ALL: [ChainKey; 6] = [
        Self::Arc,
        Self::Base,
        Self::EthSepolia,
        Self::ArbSepolia,
        Self::AvaxFuji,
        Self::OpSepolia,
    ];

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
