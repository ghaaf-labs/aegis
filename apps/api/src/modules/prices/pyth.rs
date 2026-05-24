use chrono::{TimeZone, Utc};
use serde::Deserialize;

use super::provider::{PriceProvider, SpotQuote, Symbol};

const HERMES_URL: &str = "https://hermes.pyth.network/v2/updates/price/latest";

/// Confidence band above which we reject a Pyth quote. Pyth's `conf` field is
/// in price units; we treat `conf / price > 0.01` (1%) as too uncertain to use.
const MAX_CONF_RATIO: f64 = 0.01;

pub struct PythProvider {
    http: reqwest::Client,
}

impl PythProvider {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }
}

#[async_trait::async_trait]
impl PriceProvider for PythProvider {
    async fn fetch_spot(&self, symbols: &[&Symbol]) -> anyhow::Result<Vec<SpotQuote>> {
        let with_feeds: Vec<&Symbol> = symbols
            .iter()
            .copied()
            .filter(|s| !s.pyth_feed_id.is_empty())
            .collect();
        if with_feeds.is_empty() {
            return Ok(vec![]);
        }

        // Hermes wants repeated `ids[]=` params. reqwest's query API collects
        // identical keys into the right shape.
        let query: Vec<(&str, &str)> = with_feeds
            .iter()
            .map(|s| ("ids[]", s.pyth_feed_id))
            .collect();
        let resp = self.http.get(HERMES_URL).query(&query).send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            anyhow::bail!(
                "pyth hermes {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            );
        }
        let parsed: HermesResp = serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!(
                "pyth decode: {e}; body: {}",
                body.chars().take(200).collect::<String>()
            )
        })?;

        // Build a quick lookup so we can map response feed IDs back to tickers.
        // Hermes returns ids without the 0x prefix; normalize both sides.
        let by_feed: std::collections::HashMap<String, &Symbol> = with_feeds
            .iter()
            .map(|s| (strip_0x(s.pyth_feed_id).to_ascii_lowercase(), *s))
            .collect();

        let mut out = Vec::with_capacity(parsed.parsed.len());
        for item in parsed.parsed {
            let key = strip_0x(&item.id).to_ascii_lowercase();
            let Some(sym) = by_feed.get(&key) else {
                continue;
            };
            let Some(price) = decode_price(&item.price) else {
                continue;
            };
            if price.abs() < f64::EPSILON {
                continue;
            }
            let conf = decode_price_field(&item.price.conf, item.price.expo);
            if let Some(c) = conf {
                if c / price.abs() > MAX_CONF_RATIO {
                    tracing::debug!(
                        ticker = sym.symbol,
                        conf_ratio = c / price.abs(),
                        "pyth quote rejected (low confidence)"
                    );
                    continue;
                }
            }
            let observed_at = Utc
                .timestamp_opt(item.price.publish_time, 0)
                .single()
                .unwrap_or_else(Utc::now);
            out.push(SpotQuote {
                ticker: sym.symbol,
                price_usd: price,
                change_24h: 0.0,
                change_7d: 0.0,
                market_cap: 0.0,
                volume_24h: 0.0,
                observed_at,
                confidence: conf,
            });
        }
        Ok(out)
    }

    fn name(&self) -> &'static str {
        "pyth"
    }
}

fn strip_0x(s: &str) -> &str {
    s.strip_prefix("0x").unwrap_or(s)
}

fn decode_price(p: &PriceField) -> Option<f64> {
    decode_price_field(&p.price, p.expo)
}

fn decode_price_field(raw: &str, expo: i32) -> Option<f64> {
    let n: f64 = raw.parse().ok()?;
    Some(n * 10_f64.powi(expo))
}

#[derive(Deserialize)]
struct HermesResp {
    parsed: Vec<ParsedItem>,
}

#[derive(Deserialize)]
struct ParsedItem {
    id: String,
    price: PriceField,
}

#[derive(Deserialize)]
struct PriceField {
    price: String,
    conf: String,
    expo: i32,
    publish_time: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_price_with_negative_exponent() {
        // BTC at $42,000 with expo -8 → raw integer 4_200_000_000_000.
        assert_eq!(decode_price_field("4200000000000", -8), Some(42_000.0));
    }

    #[test]
    fn parses_fixture_and_rejects_low_confidence() {
        let body = std::fs::read_to_string("tests/fixtures/pyth_latest.json")
            .expect("fixture file present");
        let parsed: HermesResp = serde_json::from_str(&body).expect("fixture parses");
        assert_eq!(parsed.parsed.len(), 2);

        let btc = &parsed.parsed[0];
        let btc_price = decode_price(&btc.price).unwrap();
        let btc_conf = decode_price_field(&btc.price.conf, btc.price.expo).unwrap();
        assert!(btc_price > 0.0);
        assert!(btc_conf / btc_price.abs() < MAX_CONF_RATIO);

        let bad = &parsed.parsed[1];
        let bad_price = decode_price(&bad.price).unwrap();
        let bad_conf = decode_price_field(&bad.price.conf, bad.price.expo).unwrap();
        assert!(bad_conf / bad_price.abs() > MAX_CONF_RATIO);
    }

    #[test]
    fn strip_prefix_handles_both_forms() {
        assert_eq!(strip_0x("0xabc"), "abc");
        assert_eq!(strip_0x("abc"), "abc");
    }
}
