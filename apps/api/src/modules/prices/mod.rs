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
pub use provider::{lookup_symbol, PriceProvider, SpotQuote, Symbol, SYMBOLS};
pub use pyth::PythProvider;

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
