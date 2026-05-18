use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::config::Config;
use crate::db::Db;
use crate::error::AppError;

/// Public Circle testnet faucet. There is no W3S API to drip USDC from a
/// server — Circle's faucet is web-only — so real-mode claims hand the user
/// a deep-link to `https://faucet.circle.com` instead of pretending to mint.
const CIRCLE_FAUCET_URL: &str = "https://faucet.circle.com";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FaucetClaimResult {
    pub amount_usdc: f64,
    pub chain: String,
    /// On-chain mint hash when the faucet really minted. `None` in real mode
    /// (Circle's faucet is web-only) and in mock mode.
    pub tx_hash: Option<String>,
    pub remaining_today_usdc: f64,
    pub claimed_at: DateTime<Utc>,
    /// Public URL the user should open to actually pull USDC into the wallet.
    /// `None` in mock mode (synthetic balance applies immediately); `Some` in
    /// real mode (user must complete the claim on Circle's faucet site).
    pub claim_url: Option<String>,
    /// Wallet address the user should paste into the faucet, surfaced so the
    /// UI doesn't have to re-fetch.
    pub arc_address: Option<String>,
}

/// Record an intent to claim USDC from the faucet.
///
/// In **mock mode**, this is the whole story — the gateway service returns a
/// synthetic 100 USDC balance so the demo flows work without external state.
///
/// In **real mode**, Circle doesn't expose a server-side faucet endpoint
/// (only the public web faucet at `https://faucet.circle.com`). We record the
/// intent for the rate-limit counter, then return `claim_url` so the UI can
/// hand off to Circle's site with the user's address. The on-chain mint
/// happens out-of-band; the gateway poller picks it up on the next tick.
///
/// Rate limit: at most `Config::faucet_max_usdc_per_day` per 24h rolling
/// window per user, counted off `analytics_events.faucet.claimed` rows.
pub async fn claim(
    db: &Db,
    config: &Config,
    user_id: Uuid,
    arc_address: &str,
) -> crate::error::Result<FaucetClaimResult> {
    let now = Utc::now();
    let window_start = now - Duration::hours(24);

    let claimed_in_window: Option<f64> = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM((properties->>'amountUsdc')::float), 0)
        FROM analytics_events
        WHERE user_id = $1
          AND event_name = 'faucet.claimed'
          AND occurred_at > $2
        "#,
    )
    .bind(user_id)
    .bind(window_start)
    .fetch_optional(db)
    .await
    .unwrap_or(Some(0.0));

    let used = claimed_in_window.unwrap_or(0.0);
    let max = config.faucet_max_usdc_per_day;
    let remaining = (max - used).max(0.0);
    if remaining <= 0.0 {
        return Err(AppError::BadRequest(format!(
            "faucet rate limit reached: {max:.0} USDC per 24h"
        )));
    }

    let amount = remaining.min(100.0);
    let claim_url = if config.circle_mock {
        None
    } else {
        Some(CIRCLE_FAUCET_URL.to_string())
    };

    sqlx::query(
        r#"INSERT INTO analytics_events (user_id, event_name, properties)
           VALUES ($1, 'faucet.claimed', $2)"#,
    )
    .bind(user_id)
    .bind(serde_json::json!({
        "amountUsdc": amount,
        "chain": "arc-testnet",
        "address": arc_address,
        "txHash": Option::<String>::None,
    }))
    .execute(db)
    .await?;

    Ok(FaucetClaimResult {
        amount_usdc: amount,
        chain: "arc-testnet".into(),
        tx_hash: None,
        remaining_today_usdc: remaining - amount,
        claimed_at: now,
        claim_url,
        arc_address: Some(arc_address.to_string()),
    })
}
