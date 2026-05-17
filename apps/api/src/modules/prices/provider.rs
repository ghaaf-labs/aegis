use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// One price observation for one asset from one provider. `confidence` is
/// optional because not every provider supplies one (DefiLlama does; CoinGecko
/// doesn't; Pyth does).
#[derive(Debug, Clone)]
pub struct SpotQuote {
    pub ticker: &'static str,
    pub price_usd: f64,
    pub change_24h: f64,
    pub change_7d: f64,
    pub market_cap: f64,
    pub volume_24h: f64,
    pub observed_at: DateTime<Utc>,
    pub confidence: Option<f64>,
}

#[async_trait]
pub trait PriceProvider: Send + Sync {
    /// Batched spot fetch. Implementations should make as few upstream
    /// requests as possible — DefiLlama supports one request for all tickers.
    /// Missing tickers (provider has no data) are dropped from the response
    /// rather than returned with sentinel values.
    async fn fetch_spot(&self, symbols: &[&Symbol]) -> anyhow::Result<Vec<SpotQuote>>;

    /// Stable provider name — surfaced on SSE `price.tick` events and the
    /// `price_history.source` column so the frontend trust badge tells the
    /// truth about where the number came from.
    fn name(&self) -> &'static str;
}

/// Per-asset metadata: tickers we surface in our API, the provider-specific
/// keys for each upstream, and the legacy CoinGecko ID for compatibility.
pub struct Symbol {
    pub ticker: &'static str,
    /// DefiLlama coin identifier — uses `coingecko:<id>` form for tokens that
    /// are listed on CoinGecko (the simplest cross-chain key).
    pub defillama_key: &'static str,
    /// Pyth Hermes feed ID (32-byte hex, 0x-prefixed). Empty string means
    /// "no known Pyth feed"; the PythProvider drops these from its request.
    pub pyth_feed_id: &'static str,
    /// CoinGecko `ids` query value — retained so the legacy CoinGecko provider
    /// keeps working as a rollback lever.
    pub cg_id_legacy: &'static str,
}

/// The one authoritative symbol table for the platform. Add new tickers here
/// once; consumers look them up by `ticker` via `lookup_symbol`.
///
/// Pyth feed IDs source: https://pyth.network/developers/price-feed-ids
/// DefiLlama coin keys source: https://defillama.com/docs/api (coins endpoint)
pub const SYMBOLS: &[Symbol] = &[
    Symbol {
        ticker: "BTC",
        defillama_key: "coingecko:bitcoin",
        pyth_feed_id: "0xe62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43",
        cg_id_legacy: "bitcoin",
    },
    Symbol {
        ticker: "ETH",
        defillama_key: "coingecko:ethereum",
        pyth_feed_id: "0xff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace",
        cg_id_legacy: "ethereum",
    },
    Symbol {
        ticker: "SOL",
        defillama_key: "coingecko:solana",
        pyth_feed_id: "0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d",
        cg_id_legacy: "solana",
    },
    Symbol {
        ticker: "BNB",
        defillama_key: "coingecko:binancecoin",
        pyth_feed_id: "0x2f95862b045670cd22bee3114c39763a4a08beeb663b145d283c31d7d1101c4f",
        cg_id_legacy: "binancecoin",
    },
    Symbol {
        ticker: "AVAX",
        defillama_key: "coingecko:avalanche-2",
        pyth_feed_id: "0x93da3352f9f1d105fdfe4971cfa80e9dd777bfc5d0f683ebb6e1294b92137bb7",
        cg_id_legacy: "avalanche-2",
    },
    Symbol {
        ticker: "LINK",
        defillama_key: "coingecko:chainlink",
        pyth_feed_id: "0x8ac0c70fff57e9aefdf5edf44b51d62c2d433653cbb2cf5cc06bb115af04d221",
        cg_id_legacy: "chainlink",
    },
    Symbol {
        ticker: "UNI",
        defillama_key: "coingecko:uniswap",
        pyth_feed_id: "0x78d185a741d07edb3412b09008b7c5cfb9bbbd7d568bf00ba737b456ba171501",
        cg_id_legacy: "uniswap",
    },
    Symbol {
        ticker: "MATIC",
        defillama_key: "coingecko:matic-network",
        pyth_feed_id: "0x5de33a9112c2b700b8d30b8a3402c103578ccfa2765696471cc672bd5cf6ac52",
        cg_id_legacy: "matic-network",
    },
    Symbol {
        ticker: "USDC",
        defillama_key: "coingecko:usd-coin",
        pyth_feed_id: "0xeaa020c61cc479712813461ce153894a96a6c00b21ed0cfc2798d1f9a9e9c94a",
        cg_id_legacy: "usd-coin",
    },
    Symbol {
        ticker: "USDT",
        defillama_key: "coingecko:tether",
        pyth_feed_id: "0x2b89b9dc8fdf9f34709a5b106b472f0f39bb6ca9ce04b0fd7f2e971688e2e53b",
        cg_id_legacy: "tether",
    },
    Symbol {
        ticker: "DAI",
        defillama_key: "coingecko:dai",
        pyth_feed_id: "0xb0948a5e5313200c632b51bb5ca32f6de0d36e9950a942d19751e833f70dabfd",
        cg_id_legacy: "dai",
    },
    Symbol {
        ticker: "USDS",
        defillama_key: "coingecko:usds",
        pyth_feed_id: "",
        cg_id_legacy: "usds",
    },
    Symbol {
        ticker: "FRAX",
        defillama_key: "coingecko:frax",
        pyth_feed_id: "0xc3d5d8d6d17081b3d0bbca6e2fa3a6704bb9a9561d9f9e1dc52db47629f862ad",
        cg_id_legacy: "frax",
    },
    Symbol {
        ticker: "EURC",
        defillama_key: "coingecko:euro-coin",
        pyth_feed_id: "",
        cg_id_legacy: "euro-coin",
    },
    Symbol {
        ticker: "USYC",
        defillama_key: "coingecko:hashnote-us-yield-coin",
        pyth_feed_id: "",
        cg_id_legacy: "hashnote-us-yield-coin",
    },
];

pub fn lookup_symbol(ticker: &str) -> Option<&'static Symbol> {
    SYMBOLS.iter().find(|s| s.ticker == ticker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_resolves_known_tickers() {
        assert_eq!(lookup_symbol("BTC").map(|s| s.ticker), Some("BTC"));
        assert_eq!(lookup_symbol("USDC").map(|s| s.ticker), Some("USDC"));
        assert!(lookup_symbol("BOGUS").is_none());
    }

    #[test]
    fn every_symbol_has_a_defillama_key() {
        for s in SYMBOLS {
            assert!(
                s.defillama_key.starts_with("coingecko:") || s.defillama_key.contains(':'),
                "symbol {} has malformed defillama_key {}",
                s.ticker,
                s.defillama_key
            );
        }
    }

    #[test]
    fn every_symbol_has_a_cg_id_legacy() {
        for s in SYMBOLS {
            assert!(
                !s.cg_id_legacy.is_empty(),
                "symbol {} has empty cg_id_legacy",
                s.ticker
            );
        }
    }
}
