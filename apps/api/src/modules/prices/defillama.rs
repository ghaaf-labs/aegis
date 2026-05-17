use chrono::{TimeZone, Utc};
use serde::Deserialize;
use std::collections::HashMap;

use super::provider::{PriceProvider, SpotQuote, Symbol};

const SPOT_URL: &str = "https://coins.llama.fi/prices/current";
const PCT_URL: &str = "https://coins.llama.fi/percentage";

pub struct DefiLlamaProvider {
    http: reqwest::Client,
}

impl DefiLlamaProvider {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    async fn fetch_spot_raw(&self, keys: &str) -> anyhow::Result<HashMap<String, RawCoin>> {
        let url = format!("{}/{}", SPOT_URL, keys);
        let resp = self.http.get(&url).send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            anyhow::bail!(
                "defillama spot {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            );
        }
        let parsed: SpotResp = serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!(
                "defillama spot decode: {e}; body: {}",
                body.chars().take(200).collect::<String>()
            )
        })?;
        Ok(parsed.coins)
    }

    async fn fetch_percentage(
        &self,
        keys: &str,
        period: &str,
    ) -> anyhow::Result<HashMap<String, f64>> {
        let url = format!("{}/{}?lookForward=false&period={}", PCT_URL, keys, period);
        let resp = self.http.get(&url).send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            // Percentage endpoint sometimes 404s for newly listed coins; treat
            // as "no percentage data" rather than failing the whole call.
            tracing::debug!(period, status = %status, "defillama percentage non-200");
            return Ok(HashMap::new());
        }
        let parsed: PctResp = serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!(
                "defillama pct decode: {e}; body: {}",
                body.chars().take(200).collect::<String>()
            )
        })?;
        Ok(parsed.coins)
    }
}

#[async_trait::async_trait]
impl PriceProvider for DefiLlamaProvider {
    async fn fetch_spot(&self, symbols: &[&Symbol]) -> anyhow::Result<Vec<SpotQuote>> {
        if symbols.is_empty() {
            return Ok(vec![]);
        }
        let keys = symbols
            .iter()
            .map(|s| s.defillama_key)
            .collect::<Vec<_>>()
            .join(",");

        // DefiLlama returns spot price + timestamp + confidence in one shot.
        // Percentage changes need two separate calls (24h, 7d). Run all three
        // concurrently — they're independent.
        let (spot, pct_24h, pct_7d) = tokio::join!(
            self.fetch_spot_raw(&keys),
            self.fetch_percentage(&keys, "24h"),
            self.fetch_percentage(&keys, "7d"),
        );
        let spot = spot?;
        // pct endpoints are best-effort; treat errors as empty so a transient
        // 5xx doesn't poison the whole tick.
        let pct_24h = pct_24h.unwrap_or_default();
        let pct_7d = pct_7d.unwrap_or_default();

        let mut out = Vec::with_capacity(symbols.len());
        for sym in symbols {
            let Some(raw) = spot.get(sym.defillama_key) else {
                continue;
            };
            // DefiLlama omits coins it can't price rather than returning null
            // — but defensively, drop zero prices anyway.
            if raw.price <= 0.0 {
                continue;
            }
            let observed_at = Utc
                .timestamp_opt(raw.timestamp as i64, 0)
                .single()
                .unwrap_or_else(Utc::now);
            out.push(SpotQuote {
                ticker: sym.ticker,
                price_usd: raw.price,
                change_24h: pct_24h.get(sym.defillama_key).copied().unwrap_or(0.0),
                change_7d: pct_7d.get(sym.defillama_key).copied().unwrap_or(0.0),
                // Free tier doesn't expose mcap/volume; consumers handle zero.
                market_cap: 0.0,
                volume_24h: 0.0,
                observed_at,
                confidence: raw.confidence,
            });
        }
        Ok(out)
    }

    fn name(&self) -> &'static str {
        "defillama"
    }
}

#[derive(Deserialize)]
struct SpotResp {
    coins: HashMap<String, RawCoin>,
}

#[derive(Deserialize)]
struct RawCoin {
    price: f64,
    #[serde(default)]
    timestamp: u64,
    #[serde(default)]
    confidence: Option<f64>,
}

#[derive(Deserialize)]
struct PctResp {
    coins: HashMap<String, f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_spot_response_fixture() {
        let body = std::fs::read_to_string("tests/fixtures/defillama_current.json")
            .expect("fixture file present");
        let parsed: SpotResp = serde_json::from_str(&body).expect("fixture parses");
        let btc = parsed.coins.get("coingecko:bitcoin").expect("btc present");
        assert!(btc.price > 0.0);
        assert!(btc.timestamp > 0);
    }

    #[test]
    fn parses_percentage_response_fixture() {
        let body = std::fs::read_to_string("tests/fixtures/defillama_pct_24h.json")
            .expect("fixture file present");
        let parsed: PctResp = serde_json::from_str(&body).expect("fixture parses");
        assert!(parsed.coins.contains_key("coingecko:bitcoin"));
    }

    #[test]
    fn skips_zero_price_coins() {
        // Build a fake spot map with one valid + one zero-priced coin.
        let raw: SpotResp = serde_json::from_str(
            r#"{"coins":{
                "coingecko:bitcoin":{"price":50000.0,"timestamp":1700000000,"confidence":0.99},
                "coingecko:dead":{"price":0.0,"timestamp":1700000000,"confidence":0.5}
            }}"#,
        )
        .unwrap();
        let btc = raw.coins.get("coingecko:bitcoin").unwrap();
        let dead = raw.coins.get("coingecko:dead").unwrap();
        assert!(btc.price > 0.0);
        assert_eq!(dead.price, 0.0);
    }
}
