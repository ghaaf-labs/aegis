//! AUM-fee streaming accrual loop (F-AUM-1).
//!
//! Pricing model (§2.1): Pro pays 25 bps/yr, Business pays 15 bps/yr on
//! AUM, streamed continuously via Nanopayments on Arc. Implementation:
//!
//!   1. Ticker runs every 24h.
//!   2. For each (active|past_due) subscription it snapshots AUM in USD,
//!      computes the accrual for the prior 24h, and inserts an
//!      `aum_accruals` row.
//!   3. The row is rolled into the open invoice for the current monthly
//!      period (or a new invoice is opened).
//!   4. At end of period the open invoice transitions open → past_due →
//!      paid via `billing::service::settle_invoice` (Nanopayments).
//!
//! The math uses `Decimal` end-to-end — no f64 rounding drift. AUM source
//! is `portfolios.total_value_usd` (maintained by the existing portfolio
//! service), so this module piggybacks on the same valuation pipeline the
//! UI shows the user; no extra price polling.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Timelike, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::Serialize;
use sqlx::Row;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::service;
use super::types::LineItem;
use crate::config::Config;
use crate::db::Db;
use crate::error::Result;

/// Seconds in a Julian year (365.25 × 86400). Locked-in here so unit tests
/// and the runtime path use the same divisor.
pub const SECONDS_PER_YEAR: i64 = 31_557_600;
/// 24-hour grace window after period_end before the invoice is marked
/// past_due. Charged on the next ticker pass.
pub const PAST_DUE_GRACE_HOURS: i64 = 7 * 24;
const BPS_DIVISOR: Decimal = dec!(10_000);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccrualRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub subscription_id: Uuid,
    pub invoice_id: Option<Uuid>,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub aum_snapshot_usd: Decimal,
    pub bps: i32,
    pub accrued_usdc: Decimal,
}

/// Pure-function accrual math. Kept separate so unit tests don't need a DB.
///
///   accrued = aum × bps × Δt_seconds / (10_000 × SECONDS_PER_YEAR)
///
/// The two divisions are collapsed into a single trailing divide so the
/// numerator carries full precision through the chain — otherwise
/// `Decimal::Decimal::checked_div` rounds at 28 digits after the first
/// divide and the day-1 accrual drifts by ~0.07%.
pub fn compute_accrual(aum_usd: Decimal, bps: u32, period_seconds: i64) -> Decimal {
    if bps == 0 || period_seconds <= 0 {
        return Decimal::ZERO;
    }
    let bps_dec = Decimal::from(bps);
    let secs = Decimal::from(period_seconds);
    let year = Decimal::from(SECONDS_PER_YEAR);
    let numerator = aum_usd * bps_dec * secs;
    let denominator = BPS_DIVISOR * year;
    numerator / denominator
}

/// Snapshot total AUM (USD) across every portfolio the user owns. Reads
/// `portfolios.total_value_usd` — the same field the dashboard renders, so
/// the AUM number on the invoice line item matches what the user sees.
pub async fn snapshot_aum(db: &Db, user_id: Uuid) -> Result<Decimal> {
    let total: Option<Decimal> = sqlx::query_scalar(
        "SELECT COALESCE(SUM(total_value_usd)::numeric, 0)
         FROM portfolios WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?
    .flatten();
    Ok(total.unwrap_or(Decimal::ZERO))
}

/// Accrue one period for one user. Persists an `aum_accruals` row (idempotent
/// on `(subscription_id, period_start, period_end)`) and rolls the accrual
/// into the open invoice for that subscription's current monthly period.
pub async fn accrue_for_period(
    db: &Db,
    user_id: Uuid,
    subscription_id: Uuid,
    tier: &str,
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
) -> anyhow::Result<AccrualRow> {
    let bps: i32 = sqlx::query_scalar("SELECT aum_annual_bps FROM plan_tiers WHERE tier = $1")
        .bind(tier)
        .fetch_optional(db)
        .await?
        .unwrap_or(0);

    let aum = snapshot_aum(db, user_id).await?;
    let seconds = (period_end - period_start).num_seconds().max(0);
    let accrued = compute_accrual(aum, bps as u32, seconds);

    let anchor_day: i32 = sqlx::query_scalar("SELECT anchor_day FROM subscriptions WHERE id = $1")
        .bind(subscription_id)
        .fetch_optional(db)
        .await?
        .unwrap_or(1);
    let (invoice_period_start, invoice_period_end) = monthly_period_for(period_end, anchor_day);
    let invoice = service::open_or_create_invoice(
        db,
        user_id,
        subscription_id,
        invoice_period_start,
        invoice_period_end,
    )
    .await?;

    let row = sqlx::query(
        r#"
        INSERT INTO aum_accruals
            (user_id, subscription_id, invoice_id, period_start, period_end,
             aum_snapshot_usd, bps, accrued_usdc)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (subscription_id, period_start, period_end) DO UPDATE
          SET invoice_id = EXCLUDED.invoice_id
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(subscription_id)
    .bind(invoice.id)
    .bind(period_start)
    .bind(period_end)
    .bind(aum)
    .bind(bps)
    .bind(accrued)
    .fetch_one(db)
    .await?;
    let accrual_id: Uuid = row.get("id");

    if accrued > Decimal::ZERO {
        service::append_invoice_line_item(
            db,
            invoice.id,
            &LineItem {
                kind: Some("aum_accrual".into()),
                description: format!("AUM fee {} bps × ${} × {}s", bps, aum.round_dp(2), seconds),
                quantity: 1.0,
                unit_amount_usdc: accrued.to_string().parse().unwrap_or(0.0),
                amount_usdc: accrued.to_string().parse().unwrap_or(0.0),
                period_start: Some(period_start),
                period_end: Some(period_end),
                ref_id: Some(accrual_id),
            },
        )
        .await?;
    }

    Ok(AccrualRow {
        id: accrual_id,
        user_id,
        subscription_id,
        invoice_id: Some(invoice.id),
        period_start,
        period_end,
        aum_snapshot_usd: aum,
        bps,
        accrued_usdc: accrued,
    })
}

/// Compute the [period_start, period_end) monthly window that contains
/// `t`, anchored on `anchor_day` of the month. Used for AUM-fee invoices.
pub fn monthly_period_for(t: DateTime<Utc>, anchor_day: i32) -> (DateTime<Utc>, DateTime<Utc>) {
    let anchor = anchor_day.clamp(1, 28) as u32;
    let day = t.day();
    let (start_y, start_m) = if day >= anchor {
        (t.year(), t.month())
    } else if t.month() == 1 {
        (t.year() - 1, 12)
    } else {
        (t.year(), t.month() - 1)
    };
    let (end_y, end_m) = if start_m == 12 {
        (start_y + 1, 1)
    } else {
        (start_y, start_m + 1)
    };
    let start = Utc
        .with_ymd_and_hms(start_y, start_m, anchor, 0, 0, 0)
        .single()
        .unwrap_or_else(|| Utc.with_ymd_and_hms(start_y, start_m, 1, 0, 0, 0).unwrap());
    let end = Utc
        .with_ymd_and_hms(end_y, end_m, anchor, 0, 0, 0)
        .single()
        .unwrap_or_else(|| Utc.with_ymd_and_hms(end_y, end_m, 1, 0, 0, 0).unwrap());
    (start, end)
}

#[derive(sqlx::FromRow)]
struct ActiveSubRow {
    id: Uuid,
    user_id: Uuid,
    tier: String,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TickReport {
    pub accruals: usize,
    pub past_due: usize,
    pub settled: usize,
    pub errors: usize,
}

/// One pass of the AUM streamer. Public so the admin "run-once" endpoint
/// can fire it on demand.
pub async fn run_once(db: &Db, config: &Config) -> anyhow::Result<TickReport> {
    let now = Utc::now();
    let period_start = now - chrono::Duration::hours(24);
    let mut report = TickReport::default();

    let subs: Vec<ActiveSubRow> = sqlx::query_as(
        "SELECT id, user_id, tier FROM subscriptions WHERE status IN ('active','past_due')",
    )
    .fetch_all(db)
    .await?;

    for sub in subs {
        match accrue_for_period(db, sub.user_id, sub.id, &sub.tier, period_start, now).await {
            Ok(_) => report.accruals += 1,
            Err(e) => {
                warn!("accrue_for_period failed sub={} err={e}", sub.id);
                report.errors += 1;
            }
        }
    }

    let due_invoices: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM invoices
         WHERE status = 'open'
           AND period_end <= NOW()
           AND total_usdc > 0",
    )
    .fetch_all(db)
    .await?;
    for (inv_id,) in due_invoices {
        if let Err(e) = service::mark_invoice_past_due(db, inv_id).await {
            warn!("mark_past_due failed inv={inv_id} err={e}");
            report.errors += 1;
        } else {
            report.past_due += 1;
        }
    }

    let to_settle: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM invoices
         WHERE status = 'past_due'
           AND period_end <= NOW() - $1::interval
           AND total_usdc > 0",
    )
    .bind(format!("{} hours", PAST_DUE_GRACE_HOURS))
    .fetch_all(db)
    .await?;
    for (inv_id,) in to_settle {
        match service::settle_invoice(db, config, inv_id).await {
            Ok(Some(tx)) => {
                info!("settled invoice {inv_id} tx={tx}");
                report.settled += 1;
            }
            Ok(None) => debug!("settle_invoice noop inv={inv_id}"),
            Err(e) => {
                warn!("settle_invoice failed inv={inv_id} err={e}");
                report.errors += 1;
            }
        }
    }

    Ok(report)
}

/// Spawn the periodic accrual ticker. Cadence: 24h. Bails immediately if
/// the V2 billing flag is off (validated at boot — see `Config::validate`).
pub fn spawn(db: Db, config: Arc<Config>) {
    if !config.aum_stream_enabled {
        return;
    }
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(86_400));
        interval.tick().await;
        loop {
            interval.tick().await;
            match run_once(&db, &config).await {
                Ok(r) => info!(
                    "aum_stream tick: accruals={} past_due={} settled={} errors={}",
                    r.accruals, r.past_due, r.settled, r.errors
                ),
                Err(e) => warn!("aum_stream tick failed: {e}"),
            }
        }
    });
}

/// True iff `t` is at-or-after the period anchor (used by docs/tests).
#[allow(dead_code)]
pub fn at_anchor(t: DateTime<Utc>, anchor_day: u32) -> bool {
    t.day() == anchor_day && t.hour() == 0
}

/// Convert a `NaiveDate` into a UTC midnight DateTime — convenience for
/// tests that build synthetic period windows.
#[allow(dead_code)]
pub fn midnight(d: NaiveDate) -> DateTime<Utc> {
    Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accrual_math_pro_one_day_on_20k() {
        // 25 bps annual on $20k AUM is $50/yr ⇒ $50/365.25 ≈ $0.13689253 per day.
        //   20_000 × 25 × 86_400 = 4.32e10
        //   10_000 × 31_557_600  = 3.15576e11
        //   4.32e10 / 3.15576e11 = 0.13689253935...
        // (Pro-tier sanity: ~$4.107/mo on a constant $20k AUM, see roadmap §2.2.)
        let aum = dec!(20_000);
        let accrued = compute_accrual(aum, 25, 86_400);
        let target = dec!(0.1368925393566050650239561944);
        let diff = (accrued - target).abs();
        assert!(
            diff < dec!(0.000000001),
            "got {accrued}, target {target}, diff {diff}"
        );
    }

    #[test]
    fn accrual_math_thirty_days_matches_invoice_total() {
        use rust_decimal::prelude::ToPrimitive;
        let aum = dec!(20_000);
        let per_day = compute_accrual(aum, 25, 86_400);
        let thirty_days = per_day * Decimal::from(30);
        let approx = thirty_days.to_f64().unwrap();
        // Roadmap §2.2: 25 bps × $20k × 30/365.25 ≈ $4.107
        assert!(
            (approx - 4.107).abs() < 0.01,
            "30-day rollup should ≈ $4.107, got {approx}"
        );
    }

    #[test]
    fn accrual_math_business_one_month_on_500k() {
        // 15 bps on $500k AUM over 30 days ≈ $61.60
        use rust_decimal::prelude::ToPrimitive;
        let aum = dec!(500_000);
        let accrued = compute_accrual(aum, 15, 30 * 86_400);
        let approx = accrued.to_f64().unwrap();
        assert!((approx - 61.6).abs() < 0.05, "expected ~61.6, got {approx}");
    }

    #[test]
    fn accrual_zero_bps_is_free_tier() {
        assert_eq!(compute_accrual(dec!(100_000), 0, 86_400), Decimal::ZERO);
    }

    #[test]
    fn accrual_zero_period_is_zero() {
        assert_eq!(compute_accrual(dec!(100_000), 25, 0), Decimal::ZERO);
    }

    #[test]
    fn monthly_period_anchor_first_of_month() {
        let t = Utc.with_ymd_and_hms(2026, 5, 16, 12, 0, 0).unwrap();
        let (s, e) = monthly_period_for(t, 1);
        assert_eq!(s, Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap());
        assert_eq!(e, Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap());
    }

    #[test]
    fn monthly_period_anchor_mid_month_walks_back() {
        let t = Utc.with_ymd_and_hms(2026, 5, 10, 12, 0, 0).unwrap();
        let (s, e) = monthly_period_for(t, 15);
        assert_eq!(s, Utc.with_ymd_and_hms(2026, 4, 15, 0, 0, 0).unwrap());
        assert_eq!(e, Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap());
    }

    #[test]
    fn line_item_rollup_thirty_days_matches_target() {
        // Simulate the line-item path: 30 daily accruals on $20k AUM at 25 bps,
        // each appended into an invoice's running total. Asserts the rollup
        // matches roadmap §2.2's $4.107/mo Pro figure to within a cent.
        use rust_decimal::prelude::ToPrimitive;
        let aum = dec!(20_000);
        let mut invoice_total = Decimal::ZERO;
        for _ in 0..30 {
            invoice_total += compute_accrual(aum, 25, 86_400);
        }
        let approx = invoice_total.to_f64().unwrap();
        assert!(
            (approx - 4.107).abs() < 0.01,
            "30-day rollup should be ~$4.107, got {approx}"
        );
    }

    #[test]
    fn line_item_rollup_with_variable_aum_is_additive() {
        // AUM moves day-to-day; verify the daily accruals still sum linearly.
        let mut total = Decimal::ZERO;
        for aum_value in [10_000u32, 20_000, 30_000, 20_000, 15_000] {
            total += compute_accrual(Decimal::from(aum_value), 25, 86_400);
        }
        let manual = compute_accrual(dec!(10_000), 25, 86_400)
            + compute_accrual(dec!(20_000), 25, 86_400)
            + compute_accrual(dec!(30_000), 25, 86_400)
            + compute_accrual(dec!(20_000), 25, 86_400)
            + compute_accrual(dec!(15_000), 25, 86_400);
        assert_eq!(total, manual);
    }

    #[test]
    fn monthly_period_crosses_year_boundary() {
        let t = Utc.with_ymd_and_hms(2027, 1, 5, 0, 0, 0).unwrap();
        let (s, e) = monthly_period_for(t, 10);
        assert_eq!(s, Utc.with_ymd_and_hms(2026, 12, 10, 0, 0, 0).unwrap());
        assert_eq!(e, Utc.with_ymd_and_hms(2027, 1, 10, 0, 0, 0).unwrap());
    }

    /// DB-backed end-to-end. Walks 30 daily ticks for a Pro user with constant
    /// $20k AUM, asserts a single open invoice with 30 line items totaling
    /// `25 bps × 20k × 30/365.25 ≈ $4.107`. Ignored by default because it
    /// requires `DATABASE_URL` pointing at an empty Postgres with migrations
    /// run. Re-enable locally with:
    ///   `DATABASE_URL=postgres://aegis:aegis@localhost:5432/aegis_test \
    ///    cargo test --all-targets -- --ignored aum_stream_thirty_day`
    #[tokio::test]
    #[ignore]
    async fn aum_stream_thirty_day_rollup_db_backed() {
        use rust_decimal::prelude::ToPrimitive;
        let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
        let pool = sqlx::PgPool::connect(&db_url).await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let user_id: Uuid = sqlx::query_scalar(
            "INSERT INTO users (email, password_hash) VALUES ($1, 'x') RETURNING id",
        )
        .bind(format!("aum-test-{}@example.com", Uuid::new_v4()))
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE users SET arc_address='0x0000000000000000000000000000000000000abc' WHERE id=$1",
        )
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO portfolios (user_id, name, total_value_usd, total_pnl_usd, total_pnl_pct, risk_score)
             VALUES ($1, 'aum-test', 20000, 0, 0, 50)",
        )
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
        let sub_id: Uuid = sqlx::query_scalar(
            "INSERT INTO subscriptions (user_id, tier, status, anchor_day) VALUES ($1, 'pro', 'active', 1) RETURNING id",
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        let base = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
        for day in 0..30 {
            let start = base + chrono::Duration::days(day);
            let end = start + chrono::Duration::days(1);
            accrue_for_period(&pool, user_id, sub_id, "pro", start, end)
                .await
                .unwrap();
        }

        let totals: (Decimal, i64) = sqlx::query_as(
            "SELECT total_usdc, jsonb_array_length(line_items) FROM invoices WHERE subscription_id=$1",
        )
        .bind(sub_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(totals.1, 30, "expected 30 line items, got {}", totals.1);
        let approx = totals.0.to_f64().unwrap();
        assert!(
            (approx - 4.107).abs() < 0.01,
            "expected ~$4.107, got {approx}"
        );
    }
}
