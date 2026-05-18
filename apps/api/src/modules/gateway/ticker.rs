//! Background poller that keeps every user's `gateway.balance` SSE event
//! fresh without requiring the client to keep hitting `/gateway/balance`.
//!
//! Closes audit item L2 from Sprint 2. The task runs for the process
//! lifetime; each tick fetches every user with a Circle wallet from
//! `users.wallet_id IS NOT NULL` and broadcasts a per-user `GatewayBalance`
//! event. Slow consumers drop frames (the SSE broadcast channel is bounded
//! — see `sse::SSE_CHANNEL_CAPACITY`).

use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use tracing::{debug, warn};
use uuid::Uuid;

use super::service::{broadcast, fetch_balance};
use crate::config::Config;
use crate::db::Db;
use crate::modules::sse::SseSender;

/// Spawn the periodic Gateway-balance poller. Cadence is
/// `Config::gateway_poll_secs` (default 10). When no SSE subscribers are
/// connected the task still runs but doesn't fetch (cheap noop).
pub fn spawn_balance_ticker(db: Db, http: Client, config: Arc<Config>, sse: SseSender) {
    let cadence = Duration::from_secs(config.gateway_poll_secs.max(1));

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(cadence);
        interval.tick().await; // skip the immediate first tick

        loop {
            interval.tick().await;

            if sse.receiver_count() == 0 {
                debug!("gateway ticker: no subscribers, skipping fetch");
                continue;
            }

            let users = match sqlx::query_as::<_, ActiveWallet>(
                "SELECT id FROM users WHERE wallet_id IS NOT NULL",
            )
            .fetch_all(&db)
            .await
            {
                Ok(rows) => rows,
                Err(e) => {
                    warn!("gateway ticker: user query failed: {e}");
                    continue;
                }
            };

            for u in users {
                match fetch_balance(&http, &config, u.id).await {
                    Ok(balance) => broadcast(&sse, u.id, &balance),
                    Err(e) => {
                        debug!("gateway ticker: fetch for user {} failed: {e}", u.id)
                    }
                }
            }
        }
    });
}

#[derive(sqlx::FromRow)]
struct ActiveWallet {
    id: Uuid,
}
