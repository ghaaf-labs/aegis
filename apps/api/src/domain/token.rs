//! The canonical token table — the single source of truth for every symbol
//! Aegis prices, holds, or settles. Adding a token is one `TokenSpec` entry
//! here (plus its `{PREFIX}_{CHAIN}` env var); the route engine, planner,
//! approval gate, executor, agent, price provider, and the FE contract all
//! derive from this table.
//!
//! Symbols are typed `&'static str` consts (the [`Symbol`] alias) — never bare
//! string literals — so a typo is a compile error and "find usages" works. Each
//! token's per-chain ERC-20 is resolved from `Config` via its [`Residency`]
//! list + [`AddrSource`] (no hand-written `match`, no flat per-token `Config`
//! field). Its canonical settlement chain and supported chains are derived from
//! the same residencies, and its price-feed keys live here too.

use serde::{Deserialize, Serialize};

use crate::config::Config;

use super::chain::ChainKey;

/// A token identifier. A typed alias over `&'static str` (deliberately not an
/// enum — symbols cross DB rows, agent JSON, SSE, and LLM output as strings, so
/// the registry stays the typed source while the boundaries keep strings).
pub type Symbol = &'static str;

pub const USDC: Symbol = "USDC";
pub const USYC: Symbol = "USYC";
pub const EURC: Symbol = "EURC";
pub const ETH: Symbol = "ETH";
/// Coinbase Wrapped BTC — 1:1 BTC, the real executable BTC sleeve on Base.
pub const CBBTC: Symbol = "cbBTC";
/// Coinbase Wrapped Staked ETH — staked-ETH sleeve on Base.
pub const CBETH: Symbol = "cbETH";
/// Aerodrome — Base's native DEX token; the flagship high-volume Base sleeve.
pub const AERO: Symbol = "AERO";
pub const LINK: Symbol = "LINK";
pub const UNI: Symbol = "UNI";
/// Sky sUSDS — freely-transferable savings-yield token (DEX-swappable).
pub const SUSDS: Symbol = "sUSDS";
/// Wrapped BTC — the canonical BTC ERC-20 on Eth/Arb (8 decimals).
pub const WBTC: Symbol = "WBTC";
/// BTC spot — price reference for the `cbBTC` sleeve (no ERC-20 of its own).
pub const BTC: Symbol = "BTC";
pub const SOL: Symbol = "SOL";
pub const BNB: Symbol = "BNB";
pub const AVAX: Symbol = "AVAX";
pub const MATIC: Symbol = "MATIC";
pub const USDT: Symbol = "USDT";
pub const DAI: Symbol = "DAI";
pub const USDS: Symbol = "USDS";
pub const FRAX: Symbol = "FRAX";

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

/// A deployment-level switch that gates whether a sleeve is offered as a
/// *designable* target — distinct from execution readiness (addresses / adapters
/// / signers), which the route engine decides separately. Lets a token declare
/// its gate as data instead of the allocator special-casing a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleeveGate {
    /// Gated behind the `USYC_ENABLED` runtime flag — coming-soon until set.
    UsycEnabled,
}

impl SleeveGate {
    /// Whether this gate is open (the sleeve may be designed) under `cfg`.
    fn is_open(self, cfg: &Config) -> bool {
        match self {
            Self::UsycEnabled => cfg.usyc_enabled,
        }
    }
}

/// How a token's ERC-20 address on one chain is sourced from `Config`. This is
/// the data that replaces the old hand-written `address_for` match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddrSource {
    /// `Config.chain(chain).usdc` — the per-`ChainConfig` USDC slot. USDC only.
    ChainUsdc,
    /// The flat env-backed token map, keyed by this env *prefix*: the resolver
    /// reads `{PREFIX}_{CHAIN}` (e.g. `Env("WETH")` on Base → `WETH_BASE`),
    /// which is how the symbol→env exceptions (ETH↔WETH) are expressed as data.
    Env(&'static str),
    /// `Config.usyc_token_arc` — USYC has Teller/oracle siblings kept flat.
    UsycToken,
}

/// One chain a token resides on: how to resolve its address there, and whether
/// this is its canonical settlement chain for cross-chain planning.
#[derive(Debug, Clone, Copy)]
pub struct Residency {
    pub chain: ChainKey,
    pub addr: AddrSource,
    /// At most one residency per token is `canonical`. Multi-chain USDC sets
    /// `false` on all of them (no single home → `canonical_chain()` is `None`).
    pub canonical: bool,
}

/// Static metadata for one token — the one place each symbol is declared.
#[derive(Debug, Clone, Copy)]
pub struct TokenSpec {
    pub symbol: Symbol,
    /// FE-facing friendly name (e.g. "Bitcoin", "Cash (USDC)").
    pub label: &'static str,
    /// Whether the AI allocator may propose a target weight in this sleeve.
    ///
    /// **Invariant: `designable: true` ⟹ routable-in-principle** — it has a
    /// `Residency` with an `Env`/`ChainUsdc`/`UsycToken` source on a supported
    /// chain, so once that chain's ERC-20 + venue are configured a real route
    /// exists. Independent of *rail liveness* (cargo features / funded signers),
    /// which the route engine gates separately — so the allocator never collapses
    /// a target to USDC merely because a rail is down. But NOT independent of
    /// whether a route can exist at all: the other-L1-natives (`SOL`/`BNB`/`AVAX`/
    /// `MATIC`) and pure price-refs (`BTC`) have no residency and are not
    /// designable, so the agent can never propose an un-routable target. A guard
    /// test enforces this.
    pub designable: bool,
    /// Runtime gate that must be open for a `designable` sleeve to be offered.
    /// `None` ⇒ always available. USYC carries `Some(SleeveGate::UsycEnabled)`.
    pub gate: Option<SleeveGate>,
    pub decimals: u8,
    pub class: TokenClass,
    /// The chains this token lives on + how to resolve each address. Empty for a
    /// pure price-reference (`priced_only`).
    pub residencies: &'static [Residency],
    /// DefiLlama coin id (`coingecko:<id>` form) — the primary price key.
    pub defillama_key: &'static str,
    /// Pyth Hermes feed id (32-byte hex). `""` ⇒ no Pyth feed (provider skips it).
    pub pyth_feed_id: &'static str,
    /// Legacy CoinGecko `ids` value — kept as a rollback lever for that provider.
    pub cg_id_legacy: &'static str,
    /// Priced/tracked but never an allocation target and with no execution
    /// residency — the spot refs (`BTC`/`SOL`/…) and the bridged stables
    /// (`USDT`/`DAI`/…). A guard test asserts `priced_only ⟹ residencies.is_empty()`.
    pub priced_only: bool,
}

impl TokenSpec {
    /// Whether the AI allocator may propose a target weight in this sleeve under
    /// `cfg`: it must be `designable` AND its runtime [`SleeveGate`] (if any) must
    /// be open. Independent of execution readiness (gated at approval time).
    pub fn is_designable_target(&self, cfg: &Config) -> bool {
        self.designable && self.gate.is_none_or(|gate| gate.is_open(cfg))
    }

    /// The token's canonical settlement chain for cross-chain planning, or
    /// `None` for a multi-chain token (USDC) / pure price-reference.
    pub fn canonical_chain(&self) -> Option<ChainKey> {
        self.residencies
            .iter()
            .find(|r| r.canonical)
            .map(|r| r.chain)
    }

    /// The chains this token resides on.
    pub fn supported_chains(&self) -> impl Iterator<Item = ChainKey> + '_ {
        self.residencies.iter().map(|r| r.chain)
    }

    /// The concrete ERC-20 address for this token on `chain`, or `None` when it
    /// is unconfigured / a zero placeholder / the token does not live there.
    /// Callers MUST treat `None` as non-executable. Signature is unchanged from
    /// the previous registry so every adapter/route call site is untouched.
    pub fn address_for<'a>(&self, cfg: &'a Config, chain: ChainKey) -> Option<&'a str> {
        let res = self.residencies.iter().find(|r| r.chain == chain)?;
        let raw = match res.addr {
            AddrSource::ChainUsdc => cfg.chain(chain).usdc.as_str(),
            AddrSource::UsycToken => cfg.usyc_token_arc.as_str(),
            AddrSource::Env(_) => cfg.token_address_raw(self.symbol, chain)?,
        };
        normalize_addr(raw)
    }
}

// Shared residency tables for the multi-chain tokens (single-chain tokens
// inline their one residency in the registry below).
const USDC_RESIDENCIES: &[Residency] = &[
    Residency {
        chain: ChainKey::Arc,
        addr: AddrSource::ChainUsdc,
        canonical: false,
    },
    Residency {
        chain: ChainKey::Base,
        addr: AddrSource::ChainUsdc,
        canonical: false,
    },
    Residency {
        chain: ChainKey::EthSepolia,
        addr: AddrSource::ChainUsdc,
        canonical: false,
    },
    Residency {
        chain: ChainKey::ArbSepolia,
        addr: AddrSource::ChainUsdc,
        canonical: false,
    },
    Residency {
        chain: ChainKey::AvaxFuji,
        addr: AddrSource::ChainUsdc,
        canonical: false,
    },
    Residency {
        chain: ChainKey::OpSepolia,
        addr: AddrSource::ChainUsdc,
        canonical: false,
    },
];
// WETH (the "ETH" sleeve's ERC-20) on every V3-compatible execution chain;
// Base is canonical (the live volatile venue). Env prefix "WETH" → WETH_BASE/…
const ETH_RESIDENCIES: &[Residency] = &[
    Residency {
        chain: ChainKey::Base,
        addr: AddrSource::Env("WETH"),
        canonical: true,
    },
    Residency {
        chain: ChainKey::EthSepolia,
        addr: AddrSource::Env("WETH"),
        canonical: false,
    },
    Residency {
        chain: ChainKey::ArbSepolia,
        addr: AddrSource::Env("WETH"),
        canonical: false,
    },
    Residency {
        chain: ChainKey::OpSepolia,
        addr: AddrSource::Env("WETH"),
        canonical: false,
    },
];
// WBTC — canonical wrapped-BTC ERC-20 on Eth/Arb (Eth canonical).
const WBTC_RESIDENCIES: &[Residency] = &[
    Residency {
        chain: ChainKey::EthSepolia,
        addr: AddrSource::Env("WBTC"),
        canonical: true,
    },
    Residency {
        chain: ChainKey::ArbSepolia,
        addr: AddrSource::Env("WBTC"),
        canonical: false,
    },
];

/// Every token Aegis prices, tracks, or settles. Decimals match each token's
/// on-chain ERC-20 (USDC/USYC/EURC/USDT = 6; ETH/most ERC-20s = 18; BTC/cbBTC/
/// WBTC = 8; SOL = 9). The single source of truth.
pub const TOKEN_REGISTRY: &[TokenSpec] = &[
    TokenSpec {
        symbol: USDC,
        label: "Cash (USDC)",
        designable: true,
        gate: None,
        decimals: 6,
        class: TokenClass::Stable,
        residencies: USDC_RESIDENCIES,
        defillama_key: "coingecko:usd-coin",
        pyth_feed_id: "0xeaa020c61cc479712813461ce153894a96a6c00b21ed0cfc2798d1f9a9e9c94a",
        cg_id_legacy: "usd-coin",
        priced_only: false,
    },
    // USYC — yield sleeve. Designable, but gated on `USYC_ENABLED`; while disabled
    // it stays coming-soon / context-only. Address via the flat usyc_token_arc.
    TokenSpec {
        symbol: USYC,
        label: "US Yield Coin",
        designable: true,
        gate: Some(SleeveGate::UsycEnabled),
        decimals: 6,
        class: TokenClass::Yield,
        residencies: &[Residency {
            chain: ChainKey::Arc,
            addr: AddrSource::UsycToken,
            canonical: true,
        }],
        defillama_key: "coingecko:hashnote-us-yield-coin",
        pyth_feed_id: "",
        cg_id_legacy: "hashnote-us-yield-coin",
        priced_only: false,
    },
    // EURC — EUR sleeve. FX stablecoin economically, but settles via the
    // permissionless USDC/EURC pool on Base (Aerodrome / Uniswap V3).
    TokenSpec {
        symbol: EURC,
        label: "Euro Coin",
        designable: true,
        gate: None,
        decimals: 6,
        class: TokenClass::FxStable,
        residencies: &[Residency {
            chain: ChainKey::Base,
            addr: AddrSource::Env("EURC"),
            canonical: true,
        }],
        defillama_key: "coingecko:euro-coin",
        pyth_feed_id: "",
        cg_id_legacy: "euro-coin",
        priced_only: false,
    },
    TokenSpec {
        symbol: ETH,
        label: "Ethereum",
        designable: true,
        gate: None,
        decimals: 18,
        class: TokenClass::Volatile,
        residencies: ETH_RESIDENCIES,
        defillama_key: "coingecko:ethereum",
        pyth_feed_id: "0xff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace",
        cg_id_legacy: "ethereum",
        priced_only: false,
    },
    TokenSpec {
        symbol: CBBTC,
        label: "Bitcoin",
        designable: true,
        gate: None,
        decimals: 8,
        class: TokenClass::Volatile,
        residencies: &[Residency {
            chain: ChainKey::Base,
            addr: AddrSource::Env("CBBTC"),
            canonical: true,
        }],
        defillama_key: "coingecko:coinbase-wrapped-btc",
        pyth_feed_id: "",
        cg_id_legacy: "coinbase-wrapped-btc",
        priced_only: false,
    },
    TokenSpec {
        symbol: CBETH,
        label: "Staked ETH",
        designable: true,
        gate: None,
        decimals: 18,
        class: TokenClass::Volatile,
        residencies: &[Residency {
            chain: ChainKey::Base,
            addr: AddrSource::Env("CBETH"),
            canonical: true,
        }],
        defillama_key: "coingecko:coinbase-wrapped-staked-eth",
        pyth_feed_id: "",
        cg_id_legacy: "coinbase-wrapped-staked-eth",
        priced_only: false,
    },
    // High-volume, real Base ERC-20 sleeves (Aerodrome / Chainlink / Uniswap).
    // Designable + routable-in-principle; track-only at execution until their
    // Base address is wired for mainnet (Base Sepolia has no liquidity).
    TokenSpec {
        symbol: AERO,
        label: "Aerodrome",
        designable: true,
        gate: None,
        decimals: 18,
        class: TokenClass::Volatile,
        residencies: &[Residency {
            chain: ChainKey::Base,
            addr: AddrSource::Env("AERO"),
            canonical: true,
        }],
        defillama_key: "coingecko:aerodrome-finance",
        pyth_feed_id: "",
        cg_id_legacy: "aerodrome-finance",
        priced_only: false,
    },
    TokenSpec {
        symbol: LINK,
        label: "Chainlink",
        designable: true,
        gate: None,
        decimals: 18,
        class: TokenClass::Volatile,
        residencies: &[Residency {
            chain: ChainKey::Base,
            addr: AddrSource::Env("LINK"),
            canonical: true,
        }],
        defillama_key: "coingecko:chainlink",
        pyth_feed_id: "0x8ac0c70fff57e9aefdf5edf44b51d62c2d433653cbb2cf5cc06bb115af04d221",
        cg_id_legacy: "chainlink",
        priced_only: false,
    },
    TokenSpec {
        symbol: UNI,
        label: "Uniswap",
        designable: true,
        gate: None,
        decimals: 18,
        class: TokenClass::Volatile,
        residencies: &[Residency {
            chain: ChainKey::Base,
            addr: AddrSource::Env("UNI"),
            canonical: true,
        }],
        defillama_key: "coingecko:uniswap",
        pyth_feed_id: "0x78d185a741d07edb3412b09008b7c5cfb9bbbd7d568bf00ba737b456ba171501",
        cg_id_legacy: "uniswap",
        priced_only: false,
    },
    // sUSDS — DEX-executable on Base, but stays non-designable until its class is
    // corrected (Volatile today; it is a savings-yield token — proposing it as a
    // volatile would mis-risk it).
    TokenSpec {
        symbol: SUSDS,
        label: "Savings USDS",
        designable: false,
        gate: None,
        decimals: 18,
        class: TokenClass::Volatile,
        residencies: &[Residency {
            chain: ChainKey::Base,
            addr: AddrSource::Env("SUSDS"),
            canonical: true,
        }],
        defillama_key: "coingecko:susds",
        pyth_feed_id: "",
        cg_id_legacy: "susds",
        priced_only: false,
    },
    // WBTC — canonical wrapped-BTC ERC-20 on Eth/Arb. Non-designable, track-only
    // until its per-chain address is configured.
    TokenSpec {
        symbol: WBTC,
        label: "Wrapped BTC",
        designable: false,
        gate: None,
        decimals: 8,
        class: TokenClass::Volatile,
        residencies: WBTC_RESIDENCIES,
        defillama_key: "coingecko:wrapped-bitcoin",
        pyth_feed_id: "",
        cg_id_legacy: "wrapped-bitcoin",
        priced_only: false,
    },
    // ── Priced/track-only: no execution residency. Spot refs + bridged stables.
    // BTC is the spot reference for the `cbBTC` sleeve; SOL/BNB/AVAX/MATIC are
    // other-L1-native context; USDT/DAI/USDS/FRAX are tracked stablecoins.
    TokenSpec {
        symbol: BTC,
        label: "Bitcoin (spot)",
        designable: false,
        gate: None,
        decimals: 8,
        class: TokenClass::Volatile,
        residencies: &[],
        defillama_key: "coingecko:bitcoin",
        pyth_feed_id: "0xe62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43",
        cg_id_legacy: "bitcoin",
        priced_only: true,
    },
    TokenSpec {
        symbol: SOL,
        label: "Solana",
        designable: false,
        gate: None,
        decimals: 9,
        class: TokenClass::Volatile,
        residencies: &[],
        defillama_key: "coingecko:solana",
        pyth_feed_id: "0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d",
        cg_id_legacy: "solana",
        priced_only: true,
    },
    TokenSpec {
        symbol: BNB,
        label: "BNB",
        designable: false,
        gate: None,
        decimals: 18,
        class: TokenClass::Volatile,
        residencies: &[],
        defillama_key: "coingecko:binancecoin",
        pyth_feed_id: "0x2f95862b045670cd22bee3114c39763a4a08beeb663b145d283c31d7d1101c4f",
        cg_id_legacy: "binancecoin",
        priced_only: true,
    },
    TokenSpec {
        symbol: AVAX,
        label: "Avalanche",
        designable: false,
        gate: None,
        decimals: 18,
        class: TokenClass::Volatile,
        residencies: &[],
        defillama_key: "coingecko:avalanche-2",
        pyth_feed_id: "0x93da3352f9f1d105fdfe4971cfa80e9dd777bfc5d0f683ebb6e1294b92137bb7",
        cg_id_legacy: "avalanche-2",
        priced_only: true,
    },
    TokenSpec {
        symbol: MATIC,
        label: "Polygon",
        designable: false,
        gate: None,
        decimals: 18,
        class: TokenClass::Volatile,
        residencies: &[],
        defillama_key: "coingecko:matic-network",
        pyth_feed_id: "",
        cg_id_legacy: "matic-network",
        priced_only: true,
    },
    TokenSpec {
        symbol: USDT,
        label: "Tether",
        designable: false,
        gate: None,
        decimals: 6,
        class: TokenClass::Stable,
        residencies: &[],
        defillama_key: "coingecko:tether",
        pyth_feed_id: "0x2b89b9dc8fdf9f34709a5b106b472f0f39bb6ca9ce04b0fd7f2e971688e2e53b",
        cg_id_legacy: "tether",
        priced_only: true,
    },
    TokenSpec {
        symbol: DAI,
        label: "Dai",
        designable: false,
        gate: None,
        decimals: 18,
        class: TokenClass::Stable,
        residencies: &[],
        defillama_key: "coingecko:dai",
        pyth_feed_id: "0xb0948a5e5313200c632b51bb5ca32f6de0d36e9950a942d19751e833f70dabfd",
        cg_id_legacy: "dai",
        priced_only: true,
    },
    TokenSpec {
        symbol: USDS,
        label: "USDS",
        designable: false,
        gate: None,
        decimals: 18,
        class: TokenClass::Stable,
        residencies: &[],
        defillama_key: "coingecko:usds",
        pyth_feed_id: "",
        cg_id_legacy: "usds",
        priced_only: true,
    },
    TokenSpec {
        symbol: FRAX,
        label: "Frax",
        designable: false,
        gate: None,
        decimals: 18,
        class: TokenClass::Stable,
        residencies: &[],
        defillama_key: "coingecko:frax",
        pyth_feed_id: "",
        cg_id_legacy: "frax",
        priced_only: true,
    },
];

/// Look up a token by symbol (case-sensitive — symbols are canonical-cased).
pub fn token(symbol: &str) -> Option<&'static TokenSpec> {
    TOKEN_REGISTRY.iter().find(|t| t.symbol == symbol)
}

/// The chain a symbol settles on for cross-chain planning. USDC (multi-chain)
/// and pure price-references return `None`.
pub fn canonical_chain(symbol: &str) -> Option<ChainKey> {
    token(symbol).and_then(TokenSpec::canonical_chain)
}

/// The chain a symbol lands on for planning + network gating: its canonical
/// chain, defaulting to Base (the live venue) for multi-chain USDC, price
/// references, and unknown symbols. Used by the planner (where a leg settles)
/// and the route-preference filter (which network gates a target). The single
/// replacement for the old `ARC_NATIVE_SYMBOLS`/`BASE_NATIVE_SYMBOLS` lists.
pub fn native_chain(symbol: &str) -> ChainKey {
    canonical_chain(symbol).unwrap_or(ChainKey::Base)
}

/// True when `raw` is a usable address (non-empty, not an all-zero placeholder).
/// Used by capability probes for venue/teller addresses that have no `TokenSpec`.
pub fn is_real_addr(raw: &str) -> bool {
    normalize_addr(raw).is_some()
}

/// Trim, then reject empty strings and all-zero hex placeholders.
fn normalize_addr(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let hex = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"));
    if let Some(hex) = hex {
        if !hex.is_empty() && hex.bytes().all(|b| b == b'0') {
            return None;
        }
    }
    Some(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_symbol_resolves_with_expected_decimals() {
        assert_eq!(token(USDC).unwrap().decimals, 6);
        assert_eq!(token(USYC).unwrap().decimals, 6);
        assert_eq!(token(EURC).unwrap().decimals, 6);
        assert_eq!(token(BTC).unwrap().decimals, 8);
        assert_eq!(token(ETH).unwrap().decimals, 18);
        assert_eq!(token(SOL).unwrap().decimals, 9);
        assert!(token("DOGE").is_none());
    }

    #[test]
    fn classes_and_canonical_chains_are_correct() {
        assert_eq!(token(USDC).unwrap().class, TokenClass::Stable);
        // USDC is multi-chain → no single canonical home.
        assert_eq!(token(USDC).unwrap().canonical_chain(), None);
        assert_eq!(token(USYC).unwrap().class, TokenClass::Yield);
        assert_eq!(token(USYC).unwrap().canonical_chain(), Some(ChainKey::Arc));
        assert_eq!(token(EURC).unwrap().class, TokenClass::FxStable);
        assert_eq!(token(EURC).unwrap().canonical_chain(), Some(ChainKey::Base));
        assert_eq!(token(BTC).unwrap().class, TokenClass::Volatile);
        assert_eq!(token(ETH).unwrap().canonical_chain(), Some(ChainKey::Base));
        // A pure price-reference has no canonical chain (no residency).
        assert_eq!(token(SOL).unwrap().canonical_chain(), None);
    }

    #[test]
    fn normalize_addr_rejects_empty_and_zero() {
        assert_eq!(normalize_addr(""), None);
        assert_eq!(normalize_addr("   "), None);
        assert_eq!(
            normalize_addr("0x0000000000000000000000000000000000000000"),
            None
        );
        assert_eq!(
            normalize_addr("0X0000000000000000000000000000000000000000"),
            None
        );
        assert_eq!(
            normalize_addr("0x036CbD53842c5426634e7929541eC2318f3dCF7e"),
            Some("0x036CbD53842c5426634e7929541eC2318f3dCF7e")
        );
    }

    #[test]
    fn address_for_reads_config_and_normalizes() {
        let mut cfg = crate::config::test_config();
        cfg.chains[ChainKey::Base.index()].usdc =
            "0x036CbD53842c5426634e7929541eC2318f3dCF7e".into();
        cfg.chains[ChainKey::Arc.index()].usdc =
            "0x0000000000000000000000000000000000000000".into();
        cfg.set_token_address(
            ETH,
            ChainKey::Base,
            "0x4200000000000000000000000000000000000006",
        );

        let usdc = token(USDC).unwrap();
        assert_eq!(
            usdc.address_for(&cfg, ChainKey::Base),
            Some(cfg.chain(ChainKey::Base).usdc.as_str())
        );
        // Zero placeholder on Arc resolves to None → fail closed.
        assert_eq!(usdc.address_for(&cfg, ChainKey::Arc), None);

        let eth = token(ETH).unwrap();
        assert_eq!(
            eth.address_for(&cfg, ChainKey::Base),
            Some("0x4200000000000000000000000000000000000006")
        );
        // BTC is price-only (no residency) → None.
        assert_eq!(token(BTC).unwrap().address_for(&cfg, ChainKey::Base), None);
        // EURC resolves on Base once its DEX-pool ERC-20 is configured.
        cfg.set_token_address(
            EURC,
            ChainKey::Base,
            "0x60a3E35Cc302bFA44Cb288Bc5a4F316Fdb1adb42",
        );
        assert_eq!(
            token(EURC).unwrap().address_for(&cfg, ChainKey::Base),
            Some("0x60a3E35Cc302bFA44Cb288Bc5a4F316Fdb1adb42")
        );
        // Empty default ⇒ track-only (fail closed).
        let bare = crate::config::test_config();
        assert_eq!(
            token(EURC).unwrap().address_for(&bare, ChainKey::Base),
            None
        );
    }

    #[test]
    fn usdc_and_weth_resolve_per_chain_on_new_chains() {
        let mut cfg = crate::config::test_config();
        cfg.chains[ChainKey::OpSepolia.index()].usdc =
            "0x036CbD53842c5426634e7929541eC2318f3dCF7e".into();
        cfg.set_token_address(
            ETH,
            ChainKey::OpSepolia,
            "0x4200000000000000000000000000000000000006",
        );
        cfg.set_token_address(
            WBTC,
            ChainKey::EthSepolia,
            "0x0000000000000000000000000000000000000abc",
        );

        let usdc = token(USDC).unwrap();
        assert_eq!(
            usdc.address_for(&cfg, ChainKey::OpSepolia),
            Some(cfg.chain(ChainKey::OpSepolia).usdc.as_str())
        );
        // Unconfigured chains fail closed.
        assert_eq!(usdc.address_for(&cfg, ChainKey::ArbSepolia), None);

        let eth = token(ETH).unwrap();
        assert_eq!(
            eth.address_for(&cfg, ChainKey::OpSepolia),
            Some("0x4200000000000000000000000000000000000006")
        );
        // ETH has no residency on Avax (no V3 venue) → None.
        assert_eq!(eth.address_for(&cfg, ChainKey::AvaxFuji), None);

        let wbtc = token(WBTC).unwrap();
        assert_eq!(
            wbtc.address_for(&cfg, ChainKey::EthSepolia),
            Some("0x0000000000000000000000000000000000000abc")
        );
        // WBTC does not live on OP → None.
        assert_eq!(wbtc.address_for(&cfg, ChainKey::OpSepolia), None);
    }

    #[test]
    fn new_chain_erc20s_default_track_only() {
        // With the committed (empty) defaults, every new-chain ERC-20 resolves
        // to None so the route registry fails closed.
        let cfg = crate::config::test_config();
        for chain in [
            ChainKey::EthSepolia,
            ChainKey::ArbSepolia,
            ChainKey::AvaxFuji,
            ChainKey::OpSepolia,
        ] {
            assert_eq!(token(USDC).unwrap().address_for(&cfg, chain), None);
            assert_eq!(token(ETH).unwrap().address_for(&cfg, chain), None);
        }
    }

    #[test]
    fn unroutable_assets_are_not_designable_routable_ones_are() {
        // The agent must only be offered sleeves it can actually route to.
        for sym in [SOL, BNB, AVAX, MATIC, BTC, WBTC, SUSDS, USDT, DAI] {
            assert!(
                !token(sym).unwrap().designable,
                "{sym} must NOT be designable (no executable Arc/Base route)"
            );
        }
        for sym in [USDC, CBBTC, ETH, EURC, CBETH, LINK, UNI, AERO, USYC] {
            assert!(
                token(sym).unwrap().designable,
                "{sym} should be a designable sleeve"
            );
        }
    }

    #[test]
    fn fe_token_contract_matches_generated_json() {
        // The frontend derives `packages/shared/src/tokens.generated.json` from
        // this registry (the shared `TOKENS` table that `route-capabilities.ts`,
        // `AssetSymbol`, etc. all derive from). This guard fails loudly —
        // printing the up-to-date JSON to paste — if they drift, so the FE token
        // list can never silently diverge from the backend.
        let current = serde_json::Value::Array(
            TOKEN_REGISTRY
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "symbol": s.symbol,
                        "label": s.label,
                        "designable": s.designable,
                        "comingSoon": s.gate.is_some(),
                        "class": serde_json::to_value(s.class).expect("class serializes"),
                    })
                })
                .collect(),
        );
        let committed: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../packages/shared/src/tokens.generated.json"
        ))
        .expect("tokens.generated.json must be valid JSON");
        assert_eq!(
            committed,
            current,
            "FE token contract drifted from the registry — update \
             packages/shared/src/tokens.generated.json to:\n{}",
            serde_json::to_string_pretty(&current).unwrap()
        );
    }

    #[test]
    fn every_registry_token_is_priceable() {
        // No registry token may be unpriceable — that would break portfolio
        // valuation. The price provider derives its symbol set from here, so
        // this is the guard that the two can't drift.
        for s in TOKEN_REGISTRY {
            assert!(
                s.defillama_key.starts_with("coingecko:") || s.defillama_key.contains(':'),
                "registry token {} has a malformed/empty defillama_key: {:?}",
                s.symbol,
                s.defillama_key
            );
            assert!(
                !s.cg_id_legacy.is_empty(),
                "registry token {} has an empty cg_id_legacy",
                s.symbol
            );
        }
    }

    #[test]
    fn priced_only_tokens_have_no_execution_residency() {
        for spec in TOKEN_REGISTRY {
            if spec.priced_only {
                assert!(
                    spec.residencies.is_empty(),
                    "priced_only token {} must not declare an execution residency",
                    spec.symbol
                );
            }
        }
    }

    #[test]
    fn every_designable_token_is_routable_in_principle() {
        // Invariant guard: a designable sleeve MUST resolve an address on a
        // supported chain once configured. Seed every Env-sourced residency from
        // the registry itself (no per-symbol field pokes), then assert.
        let sentinel = "0x1111111111111111111111111111111111111111";
        let mut cfg = crate::config::test_config();
        cfg.usyc_enabled = true; // open the USYC gate so it counts as designable
        cfg.usyc_token_arc = sentinel.into();
        for chain in USDC_RESIDENCIES.iter().map(|r| r.chain) {
            cfg.chains[chain.index()].usdc = sentinel.into();
        }
        for spec in TOKEN_REGISTRY {
            for res in spec.residencies {
                if matches!(res.addr, AddrSource::Env(_)) {
                    cfg.set_token_address(spec.symbol, res.chain, sentinel);
                }
            }
        }

        for spec in TOKEN_REGISTRY {
            if !spec.is_designable_target(&cfg) {
                continue;
            }
            let routable = spec
                .supported_chains()
                .any(|chain| spec.address_for(&cfg, chain).is_some());
            assert!(
                routable,
                "designable token {} has no address route on any supported chain",
                spec.symbol
            );
        }
    }
}
