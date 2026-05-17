//! HS-6 — CoinGecko EUR/USD spot fallback for USDC ↔ EURC basis.
//!
//! Hits the free `/api/v3/simple/price` endpoint. Returns `(usdc_usd,
//! eurc_usd)`; the caller divides to get the USDC→EURC mid-market rate.
//! In-memory 30s cache so a busy /analyze loop doesn't rate-limit the
//! free tier (10-50 req/min ceiling per CoinGecko's free plan).
//!
//! On any error the caller should fall back to the prior hardcoded
//! `0.9217` so the agent always has a number.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Deserialize;

const CACHE_TTL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy)]
pub struct Quote {
    pub usdc_usd: f64,
    pub eurc_usd: f64,
}

#[derive(Deserialize)]
struct Resp {
    #[serde(rename = "usd-coin", default)]
    usd_coin: Option<Pair>,
    #[serde(rename = "euro-coin", default)]
    euro_coin: Option<Pair>,
}

#[derive(Deserialize)]
struct Pair {
    #[serde(default)]
    usd: Option<f64>,
}

static CACHE: Mutex<Option<(Instant, Quote)>> = Mutex::new(None);

pub async fn fetch_quote(http: &reqwest::Client, api_key: &str) -> anyhow::Result<Quote> {
    if let Some(q) = cached() {
        return Ok(q);
    }

    let mut url = String::from(
        "https://api.coingecko.com/api/v3/simple/price?ids=usd-coin,euro-coin&vs_currencies=usd",
    );
    if !api_key.is_empty() {
        url.push_str("&x_cg_demo_api_key=");
        url.push_str(api_key);
    }

    let resp: Resp = http
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let usdc = resp
        .usd_coin
        .and_then(|p| p.usd)
        .ok_or_else(|| anyhow::anyhow!("coingecko: usd-coin missing"))?;
    let eurc = resp
        .euro_coin
        .and_then(|p| p.usd)
        .ok_or_else(|| anyhow::anyhow!("coingecko: euro-coin missing"))?;

    let quote = Quote {
        usdc_usd: usdc,
        eurc_usd: eurc,
    };
    if let Ok(mut g) = CACHE.lock() {
        *g = Some((Instant::now(), quote));
    }
    Ok(quote)
}

fn cached() -> Option<Quote> {
    let g = CACHE.lock().ok()?;
    let (at, q) = g.as_ref()?;
    if at.elapsed() < CACHE_TTL {
        Some(*q)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_division_yields_usdc_per_eurc_rate() {
        let q = Quote {
            usdc_usd: 1.0001,
            eurc_usd: 1.0850,
        };
        // mid = usdc_usd / eurc_usd → how many EURC per USDC
        let mid = q.usdc_usd / q.eurc_usd;
        assert!((mid - 0.9217).abs() < 0.01, "expected ~0.92, got {mid}");
    }
}
