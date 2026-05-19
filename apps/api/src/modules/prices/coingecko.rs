//! CoinGecko provider — retained for rollback. The platform default is
//! `defillama`, but flipping `PRICE_PROVIDER_PRIMARY=coingecko` in env brings
//! this path back unchanged. Free tier is heavily rate-limited; do not use
//! for sustained polling.

use chrono::Utc;
use serde::Deserialize;
use std::collections::HashMap;

use super::provider::{PriceProvider, SpotQuote, Symbol};

pub struct CoinGeckoProvider {
    http: reqwest::Client,
    api_key: Option<String>,
}

impl CoinGeckoProvider {
    pub fn new(http: reqwest::Client, api_key: Option<String>) -> Self {
        Self { http, api_key }
    }
}

#[async_trait::async_trait]
impl PriceProvider for CoinGeckoProvider {
    async fn fetch_spot(&self, symbols: &[&Symbol]) -> anyhow::Result<Vec<SpotQuote>> {
        if symbols.is_empty() {
            return Ok(vec![]);
        }
        let ids: Vec<&str> = symbols.iter().map(|s| s.cg_id_legacy).collect();
        let ids_str = ids.join(",");
        let url = format!(
            "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies=usd&include_24hr_change=true&include_7d_change=true&include_market_cap=true&include_24hr_vol=true",
            ids_str
        );
        let mut req = self.http.get(&url);
        if let Some(key) = &self.api_key {
            req = req.header("x-cg-demo-api-key", key);
        }
        let resp = req.send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            anyhow::bail!(
                "coingecko {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            );
        }
        let raw: HashMap<String, CgPrice> = serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!(
                "coingecko decode: {e}; body: {}",
                body.chars().take(200).collect::<String>()
            )
        })?;
        let now = Utc::now();
        let out = symbols
            .iter()
            .filter_map(|s| {
                let p = raw.get(s.cg_id_legacy)?;
                let usd = p.usd?;
                Some(SpotQuote {
                    ticker: s.ticker,
                    price_usd: usd,
                    change_24h: p.usd_24h_change.unwrap_or(0.0),
                    change_7d: p.usd_7d_change.unwrap_or(0.0),
                    market_cap: p.usd_market_cap.unwrap_or(0.0),
                    volume_24h: p.usd_24h_vol.unwrap_or(0.0),
                    observed_at: now,
                    confidence: None,
                })
            })
            .collect();
        Ok(out)
    }

    fn name(&self) -> &'static str {
        "coingecko"
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct CgPrice {
    usd: Option<f64>,
    usd_24h_change: Option<f64>,
    usd_7d_change: Option<f64>,
    usd_market_cap: Option<f64>,
    usd_24h_vol: Option<f64>,
}
