//! Tier extractor + cap enforcement helpers.
//!
//! These don't run as a tower layer — they're invoked imperatively at the
//! top of each handler that needs to gate (POST /portfolios, agent analyze,
//! …). Pattern matches the existing `Claims` extension: each enforcer takes
//! `(pool, user_id, tier)` and returns `AppError::PaymentRequired` when the
//! cap is exceeded, which `IntoResponse` maps to HTTP 402.
//!
//! Tier resolution: a row in `subscriptions` with `status in
//! ('trialing','active')` wins; everything else → Free. The schema's partial
//! unique index on `(user_id) WHERE status IN ('trialing','active','past_due')`
//! guarantees at most one live row, so the lookup is point-query simple.
//!
//! `enforce_aum_cap` lands ahead of its caller (A4 reads the live AUM from
//! the Gateway poller before invoking it) — the per-fn `allow(dead_code)`
//! below is the narrowest scope to suppress the unused-warn until then.

use chrono::Datelike;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::modules::billing::types::{Tier, TierCaps};

/// Resolve a user's effective tier. Returns `Tier::Free` for users with no
/// row, a canceled-only history, or a `past_due` subscription (past_due
/// counts as no entitlement here — the AUM stream + decision cap should
/// throttle until the user pays).
pub async fn resolve_tier(pool: &PgPool, user_id: Uuid) -> Result<Tier> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT tier FROM subscriptions
         WHERE user_id = $1 AND status IN ('trialing','active')
         ORDER BY started_at DESC
         LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(t,)| Tier::from(t.as_str())).unwrap_or(Tier::Free))
}

/// Period start anchor for `usage_meters`: first day of the current UTC
/// month. Matches the `period_start DATE` PK column. Subscription anniversary
/// alignment (Pro/Business may not start on the 1st) is intentionally
/// punted to A4's AUM/invoice cycle — for decision-rate gating, calendar
/// month is the only signal we need and keeps the UPSERT trivially correct.
pub fn current_period_start() -> chrono::NaiveDate {
    let today = chrono::Utc::now().date_naive();
    chrono::NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
        .expect("first-of-month is always a valid date")
}

/// Capability lookup keyed by the static `Tier::caps()` table. The DB row in
/// `plan_tiers` exists for the public `/billing/tiers` response only; runtime
/// gating reads code so callers don't pay a round-trip per request.
pub fn caps_for(tier: Tier) -> TierCaps {
    tier.caps()
}

/// Reject when the user's `decisions_count` for the current period would
/// exceed `tier.decisions_cap_monthly`. `None` cap = unlimited (Business).
pub async fn enforce_decision_cap(pool: &PgPool, user_id: Uuid, tier: Tier) -> Result<()> {
    let Some(cap) = caps_for(tier).decisions_cap_monthly else {
        return Ok(());
    };
    let period_start = current_period_start();
    let used: Option<i32> = sqlx::query_scalar(
        "SELECT decisions_count FROM usage_meters
         WHERE user_id = $1 AND period_start = $2",
    )
    .bind(user_id)
    .bind(period_start)
    .fetch_optional(pool)
    .await?;
    let used = used.unwrap_or(0).max(0) as u32;
    if used >= cap {
        return Err(AppError::PaymentRequired(format!(
            "{tier} tier allows {cap} decisions/month; used {used}. Upgrade to continue."
        )));
    }
    Ok(())
}

/// Reject when the user's effective AUM would exceed `tier.aum_cap_usd`.
/// `None` cap = unlimited (Pro/Business). Caller passes the *current* AUM
/// it already had to compute; we don't re-read it here to avoid double work.
#[allow(dead_code)]
pub async fn enforce_aum_cap(
    _pool: &PgPool,
    _user_id: Uuid,
    tier: Tier,
    current_aum_usd: f64,
) -> Result<()> {
    let Some(cap) = caps_for(tier).aum_cap_usd else {
        return Ok(());
    };
    if current_aum_usd > cap {
        return Err(AppError::PaymentRequired(format!(
            "{tier} tier caps AUM at ${cap:.0}; current ${current_aum_usd:.0}. Upgrade to continue."
        )));
    }
    Ok(())
}

/// Reject when the user already owns `>= tier.portfolios_cap` portfolios.
/// `None` cap = unlimited (Business).
pub async fn enforce_portfolios_cap(pool: &PgPool, user_id: Uuid, tier: Tier) -> Result<()> {
    let Some(cap) = caps_for(tier).portfolios_cap else {
        return Ok(());
    };
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM portfolios WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    if count as u32 >= cap {
        return Err(AppError::PaymentRequired(format!(
            "{tier} tier allows {cap} portfolio(s); already have {count}. Upgrade to continue."
        )));
    }
    Ok(())
}

/// UPSERT a +1 onto `usage_meters.decisions_count` for the current period.
/// Called by the agent service immediately after persisting a decision row;
/// MUST be idempotent on re-run (we're inside a single tokio task so PK
/// collisions are impossible at the row level, but ON CONFLICT keeps things
/// safe if the caller is ever retried).
pub async fn record_decision(pool: &PgPool, user_id: Uuid) -> Result<()> {
    let period_start = current_period_start();
    sqlx::query(
        "INSERT INTO usage_meters (user_id, period_start, decisions_count)
         VALUES ($1, $2, 1)
         ON CONFLICT (user_id, period_start)
         DO UPDATE SET decisions_count = usage_meters.decisions_count + 1,
                       updated_at = NOW()",
    )
    .bind(user_id)
    .bind(period_start)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_period_start_is_first_of_month() {
        let p = current_period_start();
        assert_eq!(p.day(), 1);
    }

    #[test]
    fn caps_for_business_is_unlimited() {
        let c = caps_for(Tier::Business);
        assert!(c.decisions_cap_monthly.is_none());
        assert!(c.portfolios_cap.is_none());
        assert!(c.aum_cap_usd.is_none());
    }

    #[test]
    fn caps_for_free_matches_plan() {
        let c = caps_for(Tier::Free);
        assert_eq!(c.decisions_cap_monthly, Some(5));
        assert_eq!(c.portfolios_cap, Some(1));
        assert_eq!(c.aum_cap_usd, Some(5_000.0));
    }
}
