pub mod handlers;
pub mod service;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssetPrice {
    pub symbol: String,
    pub price_usd: f64,
    pub change_24h: f64,
    pub change_7d: f64,
    pub market_cap: f64,
    pub volume_24h: f64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MarketSnapshot {
    pub assets: Vec<AssetPrice>,
    pub fear_greed_index: u8,
    pub total_market_cap_usd: f64,
    pub btc_dominance: f64,
    pub captured_at: DateTime<Utc>,
}
