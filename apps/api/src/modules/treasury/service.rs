use chrono::{DateTime, Utc};
use serde::Serialize;
use tracing::info;
use uuid::Uuid;

use crate::config::Config;
use crate::db::Db;
use crate::modules::analytics;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsycRate {
    pub annualized_yield: f64,
    pub price_usd: f64,
    pub source: &'static str,
    pub fetched_at: DateTime<Utc>,
}

/// Used by the rebalance executor in Sprint 3; the Sprint 2 functions
/// `park_in_usyc` / `redeem_from_usyc` are log-only stubs that exist so
/// the agent service can call into them now and have execution wired up
/// in S3 without changing the call sites.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParkResult {
    pub intent: &'static str,
    pub amount_usdc: f64,
    pub executed: bool,
    pub note: &'static str,
}

/// Fetch the current USYC annualized yield. Mock returns a steady 5.1%
/// rate that matches the latest published Hashnote average for Sprint 2 demos.
pub async fn rate(_http: &reqwest::Client, _config: &Config) -> crate::error::Result<UsycRate> {
    Ok(UsycRate {
        annualized_yield: 0.0510,
        price_usd: 1.00,
        source: "circle-usyc",
        fetched_at: Utc::now(),
    })
}

/// Park USDC into USYC. Sprint 2 is log-only — the executor runtime that
/// performs the on-chain swap lands in Sprint 3.
#[allow(dead_code)]
pub async fn park_in_usyc(
    db: &Db,
    user_id: Uuid,
    amount_usdc: f64,
) -> crate::error::Result<ParkResult> {
    info!("treasury intent: park {amount_usdc:.2} USDC into USYC for user {user_id}");
    analytics::emit(
        db,
        Some(user_id),
        "treasury.park_intent",
        serde_json::json!({ "amountUsdc": amount_usdc, "asset": "USYC" }),
    )
    .await;
    Ok(ParkResult {
        intent: "park_usyc",
        amount_usdc,
        executed: false,
        note: "Sprint 2 stub — execution lands in Sprint 3 with the cross-chain executor.",
    })
}

/// Redeem USYC back into USDC. Same stub policy as `park_in_usyc`.
#[allow(dead_code)]
pub async fn redeem_from_usyc(
    db: &Db,
    user_id: Uuid,
    amount_usdc: f64,
) -> crate::error::Result<ParkResult> {
    info!("treasury intent: redeem {amount_usdc:.2} USDC from USYC for user {user_id}");
    analytics::emit(
        db,
        Some(user_id),
        "treasury.redeem_intent",
        serde_json::json!({ "amountUsdc": amount_usdc, "asset": "USYC" }),
    )
    .await;
    Ok(ParkResult {
        intent: "redeem_usyc",
        amount_usdc,
        executed: false,
        note: "Sprint 2 stub — execution lands in Sprint 3 with the cross-chain executor.",
    })
}
