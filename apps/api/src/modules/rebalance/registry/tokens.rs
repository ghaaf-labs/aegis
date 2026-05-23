//! Token registry — the canonical metadata for every symbol Aegis can hold.
//!
//! One `TokenSpec` per symbol carries decimals, economic class, the chain it
//! settles on, and a per-chain ERC-20 address resolver that reads `Config`.
//! An address that is empty or all-zero (the committed testnet placeholder)
//! resolves to `None`, which the route registry treats as "no address → fail
//! closed". This is the single source of truth consulted by the planner,
//! approval gate, executor, and agent.

use crate::config::Config;

use super::super::models::{ChainKey, TokenClass};

pub const USDC: &str = "USDC";
pub const USYC: &str = "USYC";
pub const EURC: &str = "EURC";
pub const ETH: &str = "ETH";
/// Coinbase Wrapped BTC — 1:1 BTC, the real executable BTC sleeve on Base.
pub const CBBTC: &str = "cbBTC";
/// Coinbase Wrapped Staked ETH — staked-ETH yield sleeve on Base.
pub const CBETH: &str = "cbETH";
/// Sky sUSDS — freely-transferable savings-yield token (DEX-swappable),
/// the permissionless risk-off yield sleeve (vs. allowlist-gated USYC).
pub const SUSDS: &str = "sUSDS";
/// Wrapped BTC — the canonical BTC ERC-20 on Eth/Arb (8 decimals).
pub const WBTC: &str = "WBTC";

/// Static metadata for one token. `canonical_chain == None` means the token is
/// multi-chain (USDC), so it stays wherever the user holds it.
#[derive(Debug, Clone, Copy)]
pub struct TokenSpec {
    pub symbol: &'static str,
    pub decimals: u8,
    pub class: TokenClass,
    pub canonical_chain: Option<ChainKey>,
    pub supported_chains: &'static [ChainKey],
}

impl TokenSpec {
    /// The concrete ERC-20 address for this token on `chain`, or `None` when it
    /// is unconfigured / a zero placeholder / the token does not live on that
    /// chain. Callers MUST treat `None` as non-executable.
    pub fn address_for<'a>(&self, cfg: &'a Config, chain: ChainKey) -> Option<&'a str> {
        let raw = match (self.symbol, chain) {
            (USDC, ChainKey::Arc) => cfg.usdc_arc.as_str(),
            (USDC, ChainKey::Base) => cfg.usdc_base.as_str(),
            (USDC, ChainKey::EthSepolia) => cfg.usdc_eth.as_str(),
            (USDC, ChainKey::ArbSepolia) => cfg.usdc_arb.as_str(),
            (USDC, ChainKey::AvaxFuji) => cfg.usdc_avax.as_str(),
            (USDC, ChainKey::OpSepolia) => cfg.usdc_op.as_str(),
            (USYC, ChainKey::Arc) => cfg.usyc_token_arc.as_str(),
            (ETH, ChainKey::Base) => cfg.weth_base.as_str(),
            (ETH, ChainKey::EthSepolia) => cfg.weth_eth.as_str(),
            (ETH, ChainKey::ArbSepolia) => cfg.weth_arb.as_str(),
            (ETH, ChainKey::OpSepolia) => cfg.weth_op.as_str(),
            (WBTC, ChainKey::EthSepolia) => cfg.wbtc_eth.as_str(),
            (WBTC, ChainKey::ArbSepolia) => cfg.wbtc_arb.as_str(),
            (CBBTC, ChainKey::Base) => cfg.cbbtc_base.as_str(),
            (CBETH, ChainKey::Base) => cfg.cbeth_base.as_str(),
            (SUSDS, ChainKey::Base) => cfg.susds_base.as_str(),
            // EURC (Arc StableFX) and the remaining price-reference symbols have
            // no canonical ERC-20 configured, so they resolve to None and fail
            // closed (track-only). New-chain ERC-20s default empty until set.
            _ => return None,
        };
        normalize_addr(raw)
    }
}

const ARC: &[ChainKey] = &[ChainKey::Arc];
const BASE: &[ChainKey] = &[ChainKey::Base];
/// USDC is multi-chain across every wallet-supported chain (custody is not the
/// same as executability — the route engine still gates on `is_execution()`).
const ALL_CHAINS: &[ChainKey] = &[
    ChainKey::Arc,
    ChainKey::Base,
    ChainKey::EthSepolia,
    ChainKey::ArbSepolia,
    ChainKey::AvaxFuji,
    ChainKey::OpSepolia,
];
/// WETH residency — every EVM execution chain that has a V3-compatible venue.
const WETH_CHAINS: &[ChainKey] = &[
    ChainKey::Base,
    ChainKey::EthSepolia,
    ChainKey::ArbSepolia,
    ChainKey::OpSepolia,
];
/// WBTC residency — Eth + Arb (canonical WBTC ERC-20).
const WBTC_CHAINS: &[ChainKey] = &[ChainKey::EthSepolia, ChainKey::ArbSepolia];

/// Every token Aegis prices, tracks, or settles. Decimals match each token's
/// on-chain ERC-20 (USDC/USYC/EURC = 6; ETH/most ERC-20s = 18; BTC = 8; SOL = 9).
pub const TOKEN_REGISTRY: &[TokenSpec] = &[
    TokenSpec {
        symbol: USDC,
        decimals: 6,
        class: TokenClass::Stable,
        canonical_chain: None,
        supported_chains: ALL_CHAINS,
    },
    TokenSpec {
        symbol: USYC,
        decimals: 6,
        class: TokenClass::Yield,
        canonical_chain: Some(ChainKey::Arc),
        supported_chains: ARC,
    },
    TokenSpec {
        symbol: EURC,
        decimals: 6,
        class: TokenClass::FxStable,
        canonical_chain: Some(ChainKey::Arc),
        supported_chains: ARC,
    },
    TokenSpec {
        symbol: "BTC",
        decimals: 8,
        class: TokenClass::Volatile,
        canonical_chain: Some(ChainKey::Base),
        supported_chains: BASE,
    },
    TokenSpec {
        symbol: ETH,
        decimals: 18,
        class: TokenClass::Volatile,
        // Canonically resides on Base (the live volatile venue) for cross-chain
        // planning, but its WETH ERC-20 is resolvable on every V3-compatible
        // chain so a same-chain swap on Eth/Arb/OP works once configured.
        canonical_chain: Some(ChainKey::Base),
        supported_chains: WETH_CHAINS,
    },
    // WBTC — canonical wrapped-BTC ERC-20 on Eth/Arb. Track-only until its
    // per-chain address is configured (the registry fails closed on empty).
    TokenSpec {
        symbol: WBTC,
        decimals: 8,
        class: TokenClass::Volatile,
        canonical_chain: Some(ChainKey::EthSepolia),
        supported_chains: WBTC_CHAINS,
    },
    // Real, DEX-executable Base sleeves (Uniswap V3 / Aerodrome). These settle
    // on-chain once their Base ERC-20 address is configured — cbBTC = BTC
    // exposure, cbETH = staked-ETH yield, sUSDS = savings yield.
    TokenSpec {
        symbol: CBBTC,
        decimals: 8,
        class: TokenClass::Volatile,
        canonical_chain: Some(ChainKey::Base),
        supported_chains: BASE,
    },
    TokenSpec {
        symbol: CBETH,
        decimals: 18,
        class: TokenClass::Volatile,
        canonical_chain: Some(ChainKey::Base),
        supported_chains: BASE,
    },
    TokenSpec {
        symbol: SUSDS,
        decimals: 18,
        class: TokenClass::Volatile,
        canonical_chain: Some(ChainKey::Base),
        supported_chains: BASE,
    },
    TokenSpec {
        symbol: "SOL",
        decimals: 9,
        class: TokenClass::Volatile,
        canonical_chain: Some(ChainKey::Base),
        supported_chains: BASE,
    },
    TokenSpec {
        symbol: "BNB",
        decimals: 18,
        class: TokenClass::Volatile,
        canonical_chain: Some(ChainKey::Base),
        supported_chains: BASE,
    },
    TokenSpec {
        symbol: "AVAX",
        decimals: 18,
        class: TokenClass::Volatile,
        canonical_chain: Some(ChainKey::Base),
        supported_chains: BASE,
    },
    TokenSpec {
        symbol: "MATIC",
        decimals: 18,
        class: TokenClass::Volatile,
        canonical_chain: Some(ChainKey::Base),
        supported_chains: BASE,
    },
    TokenSpec {
        symbol: "LINK",
        decimals: 18,
        class: TokenClass::Volatile,
        canonical_chain: Some(ChainKey::Base),
        supported_chains: BASE,
    },
    TokenSpec {
        symbol: "UNI",
        decimals: 18,
        class: TokenClass::Volatile,
        canonical_chain: Some(ChainKey::Base),
        supported_chains: BASE,
    },
];

/// Look up a token by symbol (case-sensitive — symbols are uppercase canonical).
pub fn token(symbol: &str) -> Option<&'static TokenSpec> {
    TOKEN_REGISTRY.iter().find(|t| t.symbol == symbol)
}

/// The chain a symbol settles on for cross-chain planning. USDC (multi-chain)
/// has no single canonical chain and returns `None`.
pub fn canonical_chain(symbol: &str) -> Option<ChainKey> {
    token(symbol).and_then(|t| t.canonical_chain)
}

/// True when `raw` is a usable address (non-empty, not an all-zero placeholder).
/// Used by the capability probe for venue/teller addresses that have no
/// `TokenSpec` of their own.
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
        assert_eq!(token("BTC").unwrap().decimals, 8);
        assert_eq!(token(ETH).unwrap().decimals, 18);
        assert_eq!(token("SOL").unwrap().decimals, 9);
        assert!(token("DOGE").is_none());
    }

    #[test]
    fn classes_and_canonical_chains_are_correct() {
        assert_eq!(token(USDC).unwrap().class, TokenClass::Stable);
        assert_eq!(token(USDC).unwrap().canonical_chain, None);
        assert_eq!(token(USYC).unwrap().class, TokenClass::Yield);
        assert_eq!(token(USYC).unwrap().canonical_chain, Some(ChainKey::Arc));
        assert_eq!(token(EURC).unwrap().class, TokenClass::FxStable);
        assert_eq!(token(EURC).unwrap().canonical_chain, Some(ChainKey::Arc));
        assert_eq!(token("BTC").unwrap().class, TokenClass::Volatile);
        assert_eq!(token(ETH).unwrap().canonical_chain, Some(ChainKey::Base));
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
        cfg.usdc_base = "0x036CbD53842c5426634e7929541eC2318f3dCF7e".into();
        cfg.usdc_arc = "0x0000000000000000000000000000000000000000".into();
        cfg.weth_base = "0x4200000000000000000000000000000000000006".into();

        let usdc = token(USDC).unwrap();
        assert_eq!(
            usdc.address_for(&cfg, ChainKey::Base),
            Some(cfg.usdc_base.as_str())
        );
        // Zero placeholder on Arc resolves to None → fail closed.
        assert_eq!(usdc.address_for(&cfg, ChainKey::Arc), None);

        let eth = token(ETH).unwrap();
        assert_eq!(
            eth.address_for(&cfg, ChainKey::Base),
            Some(cfg.weth_base.as_str())
        );
        // BTC has no configured Base ERC-20 → None.
        assert_eq!(
            token("BTC").unwrap().address_for(&cfg, ChainKey::Base),
            None
        );
    }

    #[test]
    fn usdc_and_weth_resolve_per_chain_on_new_chains() {
        let mut cfg = crate::config::test_config();
        cfg.usdc_op = "0x036CbD53842c5426634e7929541eC2318f3dCF7e".into();
        cfg.weth_op = "0x4200000000000000000000000000000000000006".into();
        cfg.wbtc_eth = "0x0000000000000000000000000000000000000abc".into();

        let usdc = token(USDC).unwrap();
        assert_eq!(
            usdc.address_for(&cfg, ChainKey::OpSepolia),
            Some(cfg.usdc_op.as_str())
        );
        // Unconfigured chains fail closed.
        assert_eq!(usdc.address_for(&cfg, ChainKey::ArbSepolia), None);

        let eth = token(ETH).unwrap();
        assert_eq!(
            eth.address_for(&cfg, ChainKey::OpSepolia),
            Some(cfg.weth_op.as_str())
        );
        // ETH has no canonical ERC-20 on Avax (no V3 venue) → None.
        assert_eq!(eth.address_for(&cfg, ChainKey::AvaxFuji), None);

        let wbtc = token(WBTC).unwrap();
        assert_eq!(
            wbtc.address_for(&cfg, ChainKey::EthSepolia),
            Some(cfg.wbtc_eth.as_str())
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
}
