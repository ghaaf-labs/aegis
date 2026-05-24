//! Price provider abstraction.
//!
//! One trait, three impls (DefiLlama as primary, Pyth as fallback, CoinGecko
//! retained as a rollback lever) and a `FallbackProvider` wrapper that adds
//! per-ticker caching and a circuit breaker. All consumers (market_data,
//! peg_monitor, fx) read prices through `Arc<dyn PriceProvider>` on AppState.

pub mod cache;
pub mod coingecko;
pub mod defillama;
pub mod provider;
pub mod pyth;

pub use cache::FallbackProvider;
pub use coingecko::CoinGeckoProvider;
pub use defillama::DefiLlamaProvider;
pub use provider::{PriceProvider, SpotQuote, Symbol};
pub use pyth::PythProvider;

/// Tokens that have a price feed (a non-empty `defillama_key`), derived from the
/// token registry so the priced set can never drift from a parallel list.
pub fn priceable_symbols() -> Vec<&'static Symbol> {
    crate::domain::token::TOKEN_REGISTRY
        .iter()
        .filter(|s| !s.defillama_key.is_empty())
        .collect()
}

/// Resolve a ticker to its registry entry, if it is priceable. Replaces the old
/// hand-written `SYMBOLS` lookup; the registry is the single source.
pub fn lookup_symbol(ticker: &str) -> Option<&'static Symbol> {
    crate::domain::token::token(ticker).filter(|s| !s.defillama_key.is_empty())
}

use std::sync::Arc;

use crate::config::Config;

/// Build the `Arc<dyn PriceProvider>` for `AppState` from config. Always
/// wraps the primary in a `FallbackProvider` so per-ticker caching is
/// applied even when no fallback is configured — otherwise every consumer
/// (peg monitor, SSE ticker, FX, agent tools) would hit the upstream
/// directly on every poll. When there's no real fallback the primary is
/// used as both providers, so the circuit-breaker path is a no-op.
pub fn build_from_config(http: reqwest::Client, config: &Config) -> Arc<dyn PriceProvider> {
    let primary = construct(&config.price_provider_primary, http.clone(), config);
    let fallback_name = config.price_provider_fallback.as_str();
    let fallback = if fallback_name == "none" || fallback_name == config.price_provider_primary {
        primary.clone()
    } else {
        construct(fallback_name, http, config)
    };
    Arc::new(FallbackProvider::new(primary, fallback))
}

fn construct(name: &str, http: reqwest::Client, config: &Config) -> Arc<dyn PriceProvider> {
    match name {
        "pyth" => Arc::new(PythProvider::new(http)),
        "coingecko" => Arc::new(CoinGeckoProvider::new(
            http,
            config.coingecko_api_key.clone(),
        )),
        // Default: DefiLlama. Unknown names fall through here so a typo in
        // `.env.local` doesn't crash boot; we just log and use the default.
        other => {
            if other != "defillama" {
                tracing::warn!(
                    requested = %other,
                    "unknown PRICE_PROVIDER value; falling back to defillama"
                );
            }
            Arc::new(DefiLlamaProvider::new(http))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_resolves_known_tickers_and_priceable_set_is_the_registry() {
        assert_eq!(lookup_symbol("BTC").map(|s| s.symbol), Some("BTC"));
        assert_eq!(lookup_symbol("USDC").map(|s| s.symbol), Some("USDC"));
        assert!(lookup_symbol("BOGUS").is_none());
        // Every registry token is priceable (guarded in domain::token), so the
        // priceable set is exactly the registry — no parallel list to drift.
        assert_eq!(
            priceable_symbols().len(),
            crate::domain::token::TOKEN_REGISTRY.len()
        );
    }
}
