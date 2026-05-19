//! `fetch_news` tool — top-3 headlines for a symbol.
//!
//! Sources, in order: CoinGecko `coins/{id}/status_updates` if a key is
//! configured, else a deterministic synthetic feed seeded by the symbol so
//! local dev + CI is hermetic. We deliberately don't surface a raw URL list
//! to the model — only short summary lines — to avoid feeding it
//! prompt-injection vectors hidden in scraped headlines.

use serde_json::Value;

use crate::router::AppState;

const TIMEOUT_SECS: u64 = 6;

pub async fn run(state: &AppState, args: &Value) -> Result<String, String> {
    let symbol = args
        .get("symbol")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing required arg: symbol".to_string())?
        .to_uppercase();

    if state.config.coingecko_api_key.is_none() {
        return Ok(synthetic_feed(&symbol));
    }

    match fetch_live(state, &symbol).await {
        Ok(payload) => Ok(payload),
        Err(e) => {
            tracing::debug!(symbol=%symbol, error=%e, "fetch_news fell back to synthetic");
            Ok(synthetic_feed(&symbol))
        }
    }
}

async fn fetch_live(state: &AppState, symbol: &str) -> anyhow::Result<String> {
    let id = symbol_to_coingecko_id(symbol);
    let url =
        format!("https://pro-api.coingecko.com/api/v3/coins/{id}/status_updates?per_page=3&page=1");
    let key = state.config.coingecko_api_key.as_deref().unwrap_or("");
    let resp = state
        .http
        .get(&url)
        .header("x-cg-pro-api-key", key)
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    let updates = resp.get("status_updates").and_then(|v| v.as_array());
    let mut headlines: Vec<String> = Vec::with_capacity(3);
    if let Some(updates) = updates {
        for u in updates.iter().take(3) {
            let title = u.get("description").and_then(|v| v.as_str()).unwrap_or("");
            if !title.is_empty() {
                headlines.push(truncate(title, 120));
            }
        }
    }
    if headlines.is_empty() {
        return Ok(synthetic_feed(symbol));
    }
    Ok(serde_json::json!({
        "symbol": symbol,
        "headlines": headlines,
        "source": "coingecko",
    })
    .to_string())
}

fn synthetic_feed(symbol: &str) -> String {
    // When no live news source is configured we return an empty result with
    // an explicit `unavailable` source. Previous code shipped three neutral
    // headlines (e.g. "no notable narrative shifts") which the strategist
    // would parse as a real "no news, market quiet" signal — biasing it
    // toward inaction. An empty headlines array + explicit source flag lets
    // the strategist see "news tool not configured" and reason accordingly
    // rather than treat absence as a market read.
    serde_json::json!({
        "symbol": symbol,
        "headlines": [],
        "source": "unavailable",
        "note": "fetch_news has no live data source configured; treat as no signal, not as 'market is quiet'.",
    })
    .to_string()
}

fn symbol_to_coingecko_id(symbol: &str) -> String {
    match symbol {
        "BTC" => "bitcoin".into(),
        "ETH" => "ethereum".into(),
        "SOL" => "solana".into(),
        "USDC" => "usd-coin".into(),
        "EURC" => "euro-coin".into(),
        "USYC" => "usyc".into(),
        other => other.to_lowercase(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max - 1).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_feed_is_valid_json_and_contains_symbol() {
        let raw = synthetic_feed("BTC");
        let v: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["symbol"], "BTC");
        // Explicit unavailable signal rather than fake headlines so the
        // strategist doesn't read absence as "no news, market quiet".
        assert_eq!(v["source"], "unavailable");
        assert!(v["headlines"].as_array().unwrap().is_empty());
    }

    #[test]
    fn symbol_to_id_maps_known_tickers() {
        assert_eq!(symbol_to_coingecko_id("BTC"), "bitcoin");
        assert_eq!(symbol_to_coingecko_id("ETH"), "ethereum");
        assert_eq!(symbol_to_coingecko_id("UNKNOWN"), "unknown");
    }

    #[test]
    fn truncate_respects_max() {
        assert_eq!(truncate("hello", 10), "hello");
        let t = truncate("0123456789ABCDE", 8);
        assert!(t.chars().count() <= 8);
    }
}
