use chrono::{DateTime, Utc};
use serde::Serialize;

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

/// Fetch the current USDC ↔ EURC basis. Mock returns a steady ~0.92 mid
/// (typical USD/EUR rate) so the agent's prompt has a stable signal.
pub async fn usdc_eurc_basis(
    _http: &reqwest::Client,
    _config: &Config,
) -> crate::error::Result<UsdcEurcBasis> {
    Ok(UsdcEurcBasis {
        mid_rate: 0.9217,
        spread_bps: 1.5,
        source: "arc-stablefx",
        fetched_at: Utc::now(),
    })
}
