use chrono::{DateTime, Utc};
use serde::Serialize;

use super::prices;
use crate::config::Config;
use crate::modules::prices::PriceProvider;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsdcEurcBasis {
    /// USDC → EURC mid-market rate (i.e. how many EURC you get per USDC).
    pub mid_rate: f64,
    /// Half-spread in basis points (StableFX quotes are tight on Arc).
    pub spread_bps: f64,
    pub source: String,
    pub fetched_at: DateTime<Utc>,
}

/// HS-6 — fetch the live USDC ↔ EURC basis via the platform price provider.
/// When `STABLEFX_INSTITUTIONAL_ACCESS=true` (default false), an institutional
/// StableFX RFQ would land here first with the provider path as graceful
/// fallback. Until institutional access opens, the provider path *is* production.
///
/// On any error degrades to a steady ~0.92 mid so the agent's prompt always
/// has a stable signal.
pub async fn usdc_eurc_basis(
    provider: &dyn PriceProvider,
    config: &Config,
) -> crate::error::Result<UsdcEurcBasis> {
    if config.stablefx_institutional_access {
        tracing::debug!(
            "STABLEFX_INSTITUTIONAL_ACCESS=true but RFQ path not wired; using price provider"
        );
    }

    let source_name = provider.name().to_string();
    match prices::fetch_quote(provider).await {
        Ok(q) if q.eurc_usd > 0.0 => Ok(UsdcEurcBasis {
            mid_rate: round4(q.usdc_usd / q.eurc_usd),
            spread_bps: 1.5,
            source: source_name,
            fetched_at: Utc::now(),
        }),
        Ok(_) | Err(_) => Ok(UsdcEurcBasis {
            mid_rate: 0.9217,
            spread_bps: 1.5,
            source: format!("{source_name}-fallback"),
            fetched_at: Utc::now(),
        }),
    }
}

fn round4(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}
