use chrono::Utc;
use std::collections::HashMap;

use super::{AssetPrice, MarketSnapshot};
use crate::db::Db;
use crate::modules::prices::{PriceProvider, SpotQuote, SYMBOLS};

/// Fetch the current spot for every symbol in `SYMBOLS` via the configured
/// price provider. Drops symbols the provider has no data for, matching the
/// prior CoinGecko-era behaviour (retired coingecko IDs returned empty maps).
pub async fn fetch_prices(provider: &dyn PriceProvider) -> anyhow::Result<Vec<AssetPrice>> {
    let symbols: Vec<&_> = SYMBOLS.iter().collect();
    let quotes = provider.fetch_spot(&symbols).await?;
    Ok(quotes.into_iter().map(to_asset_price).collect())
}

fn to_asset_price(q: SpotQuote) -> AssetPrice {
    AssetPrice {
        symbol: q.ticker.to_string(),
        price_usd: q.price_usd,
        change_24h: q.change_24h,
        change_7d: q.change_7d,
        market_cap: q.market_cap,
        volume_24h: q.volume_24h,
        updated_at: q.observed_at,
    }
}

pub async fn fetch_snapshot(provider: &dyn PriceProvider) -> anyhow::Result<MarketSnapshot> {
    let assets = fetch_prices(provider).await?;

    let btc_cap = assets
        .iter()
        .find(|a| a.symbol == "BTC")
        .map(|a| a.market_cap)
        .unwrap_or(0.0);
    let total_cap: f64 = assets.iter().map(|a| a.market_cap).sum();
    let btc_dominance = if total_cap > 0.0 {
        (btc_cap / total_cap) * 100.0
    } else {
        0.0
    };

    Ok(MarketSnapshot {
        assets,
        fear_greed_index: fetch_fear_greed_index().await,
        total_market_cap_usd: total_cap,
        btc_dominance,
        captured_at: Utc::now(),
    })
}

/// Fetch the crypto Fear & Greed Index from alternative.me — free, no auth,
/// updated daily. Returns 50 (neutral) if the request fails so we never
/// surface a misleading bullish/bearish read when we have no real signal.
/// Previously this was hard-coded to 65 ("Greed") — a lie to every user.
async fn fetch_fear_greed_index() -> u8 {
    #[derive(serde::Deserialize)]
    struct Envelope {
        data: Vec<Item>,
    }
    #[derive(serde::Deserialize)]
    struct Item {
        value: String,
    }
    match reqwest::Client::new()
        .get("https://api.alternative.me/fng/?limit=1")
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await
        .and_then(|r| r.error_for_status())
    {
        Ok(resp) => match resp.json::<Envelope>().await {
            Ok(e) => e
                .data
                .first()
                .and_then(|i| i.value.parse::<u8>().ok())
                .unwrap_or(50),
            Err(_) => 50,
        },
        Err(_) => 50,
    }
}

/// Persists the current price snapshot into `price_history`. `source` is the
/// provider name (`provider.name()`) so the column tells the truth about where
/// each row came from. Called on every successful ticker tick so we have dense
/// data for correlation, realized vol, outcome analysis and backtests.
pub async fn persist_price_history(
    db: &Db,
    assets: &[AssetPrice],
    source: &str,
) -> anyhow::Result<()> {
    if assets.is_empty() {
        return Ok(());
    }
    for asset in assets {
        sqlx::query(
            r#"
            INSERT INTO price_history (symbol, price_usd, fetched_at, source)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(&asset.symbol)
        .bind(asset.price_usd)
        .bind(asset.updated_at)
        .bind(source)
        .execute(db)
        .await?;
    }
    Ok(())
}

/// Returns the most recent price for each symbol as of a specific point in time
/// (used by outcome compressor for true 24h "what would hold be worth" using price_history).
pub async fn get_historical_prices(
    db: &Db,
    symbols: &[String],
    as_of: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<HashMap<String, f64>> {
    if symbols.is_empty() {
        return Ok(HashMap::new());
    }

    let rows: Vec<(String, f64)> = sqlx::query_as(
        r#"
        SELECT DISTINCT ON (symbol) symbol, price_usd::DOUBLE PRECISION
        FROM price_history
        WHERE symbol = ANY($1)
          AND fetched_at <= $2
        ORDER BY symbol, fetched_at DESC
        "#,
    )
    .bind(symbols)
    .bind(as_of)
    .fetch_all(db)
    .await?;

    Ok(rows.into_iter().collect())
}
