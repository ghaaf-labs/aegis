use chrono::Utc;
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;

use super::events::{PriceTick, SseEvent};
use super::SseSender;
use crate::config::Config;
use crate::db::Db;
use crate::modules::market_data::service::persist_price_history;

/// Spawns a background task that polls market data on a configurable cadence
/// and broadcasts a `price.tick` for every asset in the snapshot.
///
/// The cadence is `Config::sse_price_tick_secs`. Failures are logged and the
/// loop continues; a single upstream hiccup never crashes the broadcaster.
pub fn spawn_price_ticker(http: Client, config: Arc<Config>, sse: SseSender, db: Db) {
    let cadence = Duration::from_secs(config.sse_price_tick_secs.max(1));

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(cadence);
        // Skip the immediate first tick — we don't want to fire on startup
        // before the API has finished binding.
        interval.tick().await;

        loop {
            interval.tick().await;

            // Always fetch + persist for Phase 1 (price_history must be dense even with no UI open).
            // Only the *broadcast* part is skipped when no one is listening.
            match crate::modules::market_data::service::fetch_snapshot(&http, &config).await {
                Ok(snapshot) => {
                    if let Err(e) = persist_price_history(&db, &snapshot.assets).await {
                        warn!("price_history persist failed (non-fatal): {e:#}");
                    }

                    if sse.receiver_count() > 0 {
                        let captured = snapshot.captured_at;
                        for asset in snapshot.assets {
                            let tick = PriceTick {
                                symbol: asset.symbol,
                                price_usd: asset.price_usd,
                                change_24h: asset.change_24h,
                                source: "coingecko".into(),
                                fetched_at: captured.max(Utc::now()),
                            };
                            let _ = sse.send(SseEvent::PriceTick(tick));
                        }
                    }
                }
                Err(e) => warn!("sse ticker: market_data fetch failed: {e:#}"),
            }
        }
    });
}
