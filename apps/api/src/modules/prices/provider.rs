use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// The platform symbol type is the canonical token registry entry. Price
/// providers read `.symbol`, `.defillama_key`, `.pyth_feed_id`, `.cg_id_legacy`
/// off it — the same table the route engine/planner/agent use, so the priced
/// set can never drift from a separate symbol list. (`SYMBOLS`/`lookup_symbol`
/// now live in `super` as registry-derived helpers.)
pub use crate::domain::token::TokenSpec as Symbol;

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
