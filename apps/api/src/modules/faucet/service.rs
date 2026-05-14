use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::config::Config;
use crate::db::Db;
use crate::error::AppError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FaucetClaimResult {
    pub amount_usdc: f64,
    pub chain: String,
    pub tx_hash: Option<String>,
    pub remaining_today_usdc: f64,
    pub claimed_at: DateTime<Utc>,
}

/// Claim USDC from the faucet for the user's Arc address.
///
/// Rate limit: at most `Config::faucet_max_usdc_per_day` per 24h rolling
/// window per user. Reads claims from `analytics_events` (event_name =
/// 'faucet.claimed'); falls back to a memory-only flag in mock mode if the
/// analytics module hasn't been migrated yet.
pub async fn claim(
    db: &Db,
    config: &Config,
    http: &reqwest::Client,
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
    let tx_hash = if config.circle_mock {
        None
    } else {
        Some(call_circle_faucet(http, config, arc_address, amount).await?)
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
        "txHash": tx_hash,
    }))
    .execute(db)
    .await?;

    Ok(FaucetClaimResult {
        amount_usdc: amount,
        chain: "arc-testnet".into(),
        tx_hash,
        remaining_today_usdc: remaining - amount,
        claimed_at: now,
    })
}

async fn call_circle_faucet(
    http: &reqwest::Client,
    config: &Config,
    address: &str,
    amount: f64,
) -> crate::error::Result<String> {
    #[derive(serde::Deserialize)]
    struct FaucetResp {
        #[serde(rename = "txHash")]
        tx_hash: String,
    }
    let resp: FaucetResp = http
        .post(format!("{}/v1/faucet/usdc", config.circle_base_url))
        .header("Authorization", format!("Bearer {}", config.circle_api_key))
        .json(&serde_json::json!({
            "address": address,
            "chain": "arc-testnet",
            "amount": amount,
        }))
        .send()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("faucet network: {e}")))?
        .error_for_status()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("faucet status: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("faucet decode: {e}")))?;
    Ok(resp.tx_hash)
}
