//! Billing HTTP endpoints.
//!
//! - `GET  /billing/referrals`         (always on) — referral payout list.
//! - `GET  /billing/tiers`             (always on, public) — pricing catalogue.
//! - `GET  /billing/subscription`      (`BILLING_V2_ENABLED`) — caller's tier.
//! - `POST /billing/subscriptions`     (`BILLING_V2_ENABLED`) — upgrade/start.
//! - `PATCH /billing/subscriptions/:id` — change tier or set `cancel_at`.
//! - `GET  /billing/invoices`          (`BILLING_V2_ENABLED`) — paginated.
//!
//! Real Nanopayments billing for the upgrade is A4's job; this commit only
//! writes the `subscriptions` row + a synthetic 30d period so the rest of the
//! stack (tier middleware, agent model routing) has something to read.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::middleware::auth::Claims;
use crate::modules::billing::types::{
    Invoice, InvoiceStatus, LineItem, PricingTier, Subscription, SubscriptionStatus, Tier,
};
use crate::router::AppState;

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ReferralRow {
    pub id: Uuid,
    pub new_user_id: Uuid,
    pub reward_usdc: f64,
    pub paid_at: Option<DateTime<Utc>>,
    pub tx_hash: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferralsResponse {
    pub rows: Vec<ReferralRow>,
    pub total_paid_usdc: f64,
    pub total_pending_usdc: f64,
}

pub async fn list_referrals(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<ReferralsResponse>> {
    let rows: Vec<ReferralRow> = sqlx::query_as(
        "SELECT id, new_user_id, reward_usdc, paid_at, tx_hash, created_at
         FROM referrals WHERE referrer_user_id = $1
         ORDER BY created_at DESC LIMIT 100",
    )
    .bind(claims.sub)
    .fetch_all(&state.db)
    .await?;

    let total_paid_usdc = rows
        .iter()
        .filter(|r| r.paid_at.is_some())
        .map(|r| r.reward_usdc)
        .sum::<f64>();
    let total_pending_usdc = rows
        .iter()
        .filter(|r| r.paid_at.is_none())
        .map(|r| r.reward_usdc)
        .sum::<f64>();

    Ok(Json(ReferralsResponse {
        rows,
        total_paid_usdc,
        total_pending_usdc,
    }))
}

// ── Billing v2: subscriptions / invoices / tiers ───────────────────────────

/// 404-with-this-body when `BILLING_V2_ENABLED=false`. Centralized so every
/// gated handler returns the same shape.
fn billing_v2_disabled() -> AppError {
    AppError::NotFound("billing v2 disabled".into())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionResponse {
    pub subscription: Subscription,
    pub implicit: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSubscriptionRequest {
    pub tier: Tier,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSubscriptionRequest {
    #[serde(default)]
    pub tier: Option<Tier>,
    #[serde(default)]
    pub cancel_at: Option<DateTime<Utc>>,
}

#[derive(Debug, sqlx::FromRow)]
struct SubscriptionRow {
    id: Uuid,
    user_id: Uuid,
    tier: String,
    status: String,
    started_at: DateTime<Utc>,
    current_period_start: DateTime<Utc>,
    current_period_end: DateTime<Utc>,
    cancel_at: Option<DateTime<Utc>>,
    billing_anchor_day: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<SubscriptionRow> for Subscription {
    fn from(r: SubscriptionRow) -> Self {
        Subscription {
            id: r.id,
            user_id: r.user_id,
            tier: Tier::from(r.tier.as_str()),
            status: match r.status.as_str() {
                "trialing" => SubscriptionStatus::Trialing,
                "active" => SubscriptionStatus::Active,
                "past_due" => SubscriptionStatus::PastDue,
                _ => SubscriptionStatus::Canceled,
            },
            started_at: r.started_at,
            current_period_start: r.current_period_start,
            current_period_end: r.current_period_end,
            cancel_at: r.cancel_at,
            billing_anchor_day: r.billing_anchor_day.max(1) as u32,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

/// `GET /billing/subscription` — caller's live subscription. If none exists
/// (a brand-new user has never upgraded), returns a synthetic "Free" row with
/// `implicit: true` so the UI can treat it uniformly.
pub async fn get_subscription(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<SubscriptionResponse>> {
    if !state.config.billing_v2_enabled {
        return Err(billing_v2_disabled());
    }
    let row: Option<SubscriptionRow> = sqlx::query_as(
        "SELECT id, user_id, tier, status, started_at, current_period_start,
                current_period_end, cancel_at, billing_anchor_day, created_at, updated_at
         FROM subscriptions
         WHERE user_id = $1 AND status IN ('trialing','active','past_due')
         ORDER BY started_at DESC
         LIMIT 1",
    )
    .bind(claims.sub)
    .fetch_optional(&state.db)
    .await?;

    match row {
        Some(r) => Ok(Json(SubscriptionResponse {
            subscription: r.into(),
            implicit: false,
        })),
        None => {
            let now = Utc::now();
            Ok(Json(SubscriptionResponse {
                subscription: Subscription {
                    id: Uuid::nil(),
                    user_id: claims.sub,
                    tier: Tier::Free,
                    status: SubscriptionStatus::Active,
                    started_at: now,
                    current_period_start: now,
                    current_period_end: now + Duration::days(30),
                    cancel_at: None,
                    billing_anchor_day: now.day().clamp(1, 28),
                    created_at: now,
                    updated_at: now,
                },
                implicit: true,
            }))
        }
    }
}

/// `POST /billing/subscriptions` — start or upgrade. A4 will wire the real
/// Nanopayments charge; today we just write the row with `status='active'`
/// and a 30-day period. The schema's partial-unique index on
/// `(user_id) WHERE status IN ('trialing','active','past_due')` makes a
/// second "active" upgrade a 23505 conflict — we surface it as 409.
pub async fn create_subscription(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateSubscriptionRequest>,
) -> Result<(StatusCode, Json<Subscription>)> {
    if !state.config.billing_v2_enabled {
        return Err(billing_v2_disabled());
    }
    if body.tier == Tier::Free {
        return Err(AppError::BadRequest(
            "free tier is implicit; cancel an existing subscription instead".into(),
        ));
    }
    let now = Utc::now();
    let anchor_day: i32 = now.day().clamp(1, 28) as i32;
    let res = sqlx::query_as::<_, SubscriptionRow>(
        "INSERT INTO subscriptions
            (user_id, tier, status, started_at,
             current_period_start, current_period_end, billing_anchor_day)
         VALUES ($1, $2, 'active', $3, $3, $3 + INTERVAL '30 days', $4)
         RETURNING id, user_id, tier, status, started_at, current_period_start,
                   current_period_end, cancel_at, billing_anchor_day, created_at, updated_at",
    )
    .bind(claims.sub)
    .bind(body.tier.to_string())
    .bind(now)
    .bind(anchor_day)
    .fetch_one(&state.db)
    .await;

    let row = match res {
        Ok(r) => r,
        Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some("23505") => {
            return Err(AppError::Conflict(
                "user already has an active subscription; PATCH /billing/subscriptions/:id to change tier".into(),
            ));
        }
        Err(e) => return Err(e.into()),
    };
    Ok((StatusCode::CREATED, Json(row.into())))
}

/// `PATCH /billing/subscriptions/:id` — change tier or set `cancel_at`. Both
/// are independently optional; sending neither is a no-op.
pub async fn patch_subscription(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateSubscriptionRequest>,
) -> Result<Json<Subscription>> {
    if !state.config.billing_v2_enabled {
        return Err(billing_v2_disabled());
    }
    let row: SubscriptionRow = sqlx::query_as(
        "UPDATE subscriptions
            SET tier      = COALESCE($1, tier),
                cancel_at = COALESCE($2, cancel_at),
                updated_at = NOW()
          WHERE id = $3 AND user_id = $4
       RETURNING id, user_id, tier, status, started_at, current_period_start,
                 current_period_end, cancel_at, billing_anchor_day, created_at, updated_at",
    )
    .bind(body.tier.map(|t| t.to_string()))
    .bind(body.cancel_at)
    .bind(id)
    .bind(claims.sub)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("subscription {id}")))?;
    Ok(Json(row.into()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoicesQuery {
    #[serde(default)]
    pub limit: Option<u32>,
    /// Cursor — return invoices created strictly before this timestamp.
    #[serde(default)]
    pub before: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoicesResponse {
    pub invoices: Vec<Invoice>,
}

#[derive(Debug, sqlx::FromRow)]
struct InvoiceRow {
    id: Uuid,
    user_id: Uuid,
    subscription_id: Option<Uuid>,
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
    line_items: serde_json::Value,
    subtotal_usdc: f64,
    total_usdc: f64,
    status: String,
    paid_at: Option<DateTime<Utc>>,
    paid_tx_hash: Option<String>,
    created_at: DateTime<Utc>,
}

fn row_to_invoice(r: InvoiceRow) -> Invoice {
    let line_items: Vec<LineItem> = serde_json::from_value(r.line_items).unwrap_or_default();
    Invoice {
        id: r.id,
        user_id: r.user_id,
        subscription_id: r.subscription_id,
        period_start: r.period_start,
        period_end: r.period_end,
        line_items,
        subtotal_usdc: r.subtotal_usdc,
        total_usdc: r.total_usdc,
        status: match r.status.as_str() {
            "paid" => InvoiceStatus::Paid,
            "void" => InvoiceStatus::Void,
            "past_due" => InvoiceStatus::PastDue,
            _ => InvoiceStatus::Open,
        },
        paid_at: r.paid_at,
        paid_tx_hash: r.paid_tx_hash,
        created_at: r.created_at,
    }
}

pub async fn list_invoices(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(q): Query<InvoicesQuery>,
) -> Result<Json<InvoicesResponse>> {
    if !state.config.billing_v2_enabled {
        return Err(billing_v2_disabled());
    }
    let limit = q.limit.unwrap_or(50).min(200) as i64;
    let before = q.before.unwrap_or_else(Utc::now);
    let rows: Vec<InvoiceRow> = sqlx::query_as(
        "SELECT id, user_id, subscription_id, period_start, period_end, line_items,
                subtotal_usdc, total_usdc, status, paid_at, paid_tx_hash, created_at
         FROM invoices
         WHERE user_id = $1 AND created_at < $2
         ORDER BY created_at DESC
         LIMIT $3",
    )
    .bind(claims.sub)
    .bind(before)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(InvoicesResponse {
        invoices: rows.into_iter().map(row_to_invoice).collect(),
    }))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TiersResponse {
    pub tiers: Vec<PricingTier>,
}

/// `GET /billing/tiers` — public pricing catalogue. Reads from `plan_tiers`
/// (the seed in 0010 is the source of truth) so price changes in a future
/// migration flow into the UI without a code change.
pub async fn list_tiers(State(state): State<AppState>) -> Result<Json<TiersResponse>> {
    if !state.config.billing_v2_enabled {
        return Err(billing_v2_disabled());
    }
    type TierRow = (String, f64, Option<f64>, Option<i32>, Option<i32>, i32, i32);
    let rows: Vec<TierRow> = sqlx::query_as(
        "SELECT code, monthly_usd, aum_cap_usd, portfolios_cap,
                decisions_cap_monthly, per_rebalance_bps, aum_annual_bps
         FROM plan_tiers
         ORDER BY monthly_usd ASC",
    )
    .fetch_all(&state.db)
    .await?;
    let tiers = rows
        .into_iter()
        .map(|(code, monthly, aum, ports, dec, prb, abps)| PricingTier {
            code: Tier::from(code.as_str()),
            monthly_usd: monthly,
            aum_cap_usd: aum,
            portfolios_cap: ports.map(|v| v.max(0) as u32),
            decisions_cap_monthly: dec.map(|v| v.max(0) as u32),
            per_rebalance_bps: prb.max(0) as u32,
            aum_annual_bps: abps.max(0) as u32,
        })
        .collect();
    Ok(Json(TiersResponse { tiers }))
}

use chrono::Datelike;
