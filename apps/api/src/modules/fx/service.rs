use chrono::{DateTime, Utc};
use serde::Serialize;

use super::coingecko;
use crate::config::Config;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsdcEurcBasis {
    /// USDC → EURC mid-market rate (i.e. how many EURC you get per USDC).
    pub mid_rate: f64,
    /// Half-spread in basis points (StableFX quotes are tight on Arc).
    pub spread_bps: f64,
    pub source: &'static str,
    pub fetched_at: DateTime<Utc>,
}

/// HS-6 — fetch the live USDC ↔ EURC basis. The default path hits
/// CoinGecko spot for both stablecoins and derives the mid rate from
/// `usdc_usd / eurc_usd`. When `STABLEFX_INSTITUTIONAL_ACCESS=true`
/// (default false), an institutional StableFX RFQ would land here first
/// with the CoinGecko path as graceful fallback. Until institutional
/// access opens, the CoinGecko path *is* production.
///
/// On any error degrades to the prior steady ~0.92 mid so the agent's
/// prompt always has a stable signal.
pub async fn usdc_eurc_basis(
    http: &reqwest::Client,
    config: &Config,
) -> crate::error::Result<UsdcEurcBasis> {
    // Future home for the StableFX-institutional path. Currently a no-op
    // unless the env opens; we never have access to flip the API on our
    // side, so this just structures the future replacement.
    if config.stablefx_institutional_access {
        // Intentionally falls through to CoinGecko until the RFQ path is
        // wired. Logged once per process so it's clear the env had no
        // effect yet.
        tracing::debug!("STABLEFX_INSTITUTIONAL_ACCESS=true but RFQ path not wired; using CoinGecko");
    }

    let api_key = config.coingecko_api_key.as_deref().unwrap_or_default();
    match coingecko::fetch_quote(http, api_key).await {
        Ok(q) if q.eurc_usd > 0.0 => Ok(UsdcEurcBasis {
            mid_rate: round4(q.usdc_usd / q.eurc_usd),
            // Spread is unobservable from spot; carry the historical
            // tight quote as a stand-in until we have RFQ depth.
            spread_bps: 1.5,
            source: "coingecko",
            fetched_at: Utc::now(),
        }),
        Ok(_) | Err(_) => Ok(UsdcEurcBasis {
            mid_rate: 0.9217,
            spread_bps: 1.5,
            source: "coingecko-fallback",
            fetched_at: Utc::now(),
        }),
    }
}

fn round4(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}
