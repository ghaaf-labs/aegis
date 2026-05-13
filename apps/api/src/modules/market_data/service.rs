use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;

use super::{AssetPrice, MarketSnapshot};
use crate::config::Config;

const COINGECKO_IDS: &[(&str, &str)] = &[
    ("BTC", "bitcoin"),
    ("ETH", "ethereum"),
    ("SOL", "solana"),
    ("BNB", "binancecoin"),
    ("AVAX", "avalanche-2"),
    ("LINK", "chainlink"),
    ("UNI", "uniswap"),
    ("MATIC", "matic-network"),
];

#[derive(Deserialize)]
struct CoinGeckoPrice {
    usd: f64,
    usd_24h_change: Option<f64>,
    usd_7d_change: Option<f64>,
    usd_market_cap: Option<f64>,
    usd_24h_vol: Option<f64>,
}

pub async fn fetch_prices(client: &Client, cfg: &Config) -> anyhow::Result<Vec<AssetPrice>> {
    let ids: Vec<&str> = COINGECKO_IDS.iter().map(|(_, id)| *id).collect();
    let ids_str = ids.join(",");

    let mut url = format!(
        "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies=usd&include_24hr_change=true&include_7d_change=true&include_market_cap=true&include_24hr_vol=true",
        ids_str
    );

    let mut req = client.get(&url);
    if let Some(key) = &cfg.coingecko_api_key {
        req = req.header("x-cg-demo-api-key", key);
    }

    let raw: HashMap<String, CoinGeckoPrice> = req.send().await?.json().await?;

    let prices = COINGECKO_IDS
        .iter()
        .filter_map(|(symbol, id)| {
            raw.get(*id).map(|p| AssetPrice {
                symbol: symbol.to_string(),
                price_usd: p.usd,
                change_24h: p.usd_24h_change.unwrap_or(0.0),
                change_7d: p.usd_7d_change.unwrap_or(0.0),
                market_cap: p.usd_market_cap.unwrap_or(0.0),
                volume_24h: p.usd_24h_vol.unwrap_or(0.0),
                updated_at: Utc::now(),
            })
        })
        .collect();

    Ok(prices)
}

pub async fn fetch_snapshot(client: &Client, cfg: &Config) -> anyhow::Result<MarketSnapshot> {
    let assets = fetch_prices(client, cfg).await?;

    let btc_cap = assets.iter().find(|a| a.symbol == "BTC").map(|a| a.market_cap).unwrap_or(0.0);
    let total_cap: f64 = assets.iter().map(|a| a.market_cap).sum();
    let btc_dominance = if total_cap > 0.0 { (btc_cap / total_cap) * 100.0 } else { 0.0 };

    Ok(MarketSnapshot {
        assets,
        fear_greed_index: 65,
        total_market_cap_usd: total_cap,
        btc_dominance,
        captured_at: Utc::now(),
    })
}
