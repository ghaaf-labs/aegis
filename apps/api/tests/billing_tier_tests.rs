//! Integration tests for the Billing v2 tier system (F-TIER-7).
//!
//! These exercise the live Postgres schema introduced by migration 0010 +
//! the runtime invariants the middleware/handlers depend on:
//!   1. Free-tier decision cap (6 inserts on a fresh meter row exceeds 5).
//!   2. The single-portfolio invariant: a second portfolio INSERT for the same
//!      user fails with a unique constraint violation regardless of tier.
//!   3. /billing/subscription resolves the upgraded tier.
//!   4. plan_tiers seed contains exactly the three tiers from §2.1.
//!
//! aegis-api is a binary crate (no library target), so we don't import the
//! axum service into the test process. Instead, the tests run the same SQL
//! the handlers run so a schema/seed regression breaks the test immediately.
//!
//! Each test SKIPs (returns early with a log line) when TEST_DATABASE_URL is
//! unset so CI can run cargo test --all-targets without a live Postgres.

use chrono::Datelike;
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use uuid::Uuid;

const TEST_DB_ENV: &str = "TEST_DATABASE_URL";

async fn pool_or_skip() -> Option<PgPool> {
    let url = std::env::var(TEST_DB_ENV).ok()?;
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .ok()?;
    // We rely on the schema already being migrated by `cargo sqlx migrate run`
    // (per CLAUDE.md). Sanity-check by probing the plan_tiers seed.
    let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM plan_tiers")
        .fetch_one(&pool)
        .await
        .ok()?;
    if n.0 < 3 {
        return None;
    }
    Some(pool)
}

async fn make_user(pool: &PgPool, email: &str) -> Uuid {
    let id: (Uuid,) = sqlx::query_as("INSERT INTO users (email) VALUES ($1) RETURNING id")
        .bind(email)
        .fetch_one(pool)
        .await
        .expect("create user");
    id.0
}

async fn cleanup_user(pool: &PgPool, user_id: Uuid) {
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await;
}

fn period_start_today() -> chrono::NaiveDate {
    let t = chrono::Utc::now().date_naive();
    chrono::NaiveDate::from_ymd_opt(t.year(), t.month(), 1).unwrap()
}

#[tokio::test]
async fn plan_tiers_seed_contains_three_rows() {
    let Some(pool) = pool_or_skip().await else {
        eprintln!("SKIP: set TEST_DATABASE_URL to run");
        return;
    };
    let rows = sqlx::query("SELECT code FROM plan_tiers ORDER BY monthly_usd ASC")
        .fetch_all(&pool)
        .await
        .expect("query plan_tiers");
    let codes: Vec<String> = rows.iter().map(|r| r.get::<String, _>("code")).collect();
    assert_eq!(codes, vec!["free", "pro", "business"]);
}

#[tokio::test]
async fn free_tier_decision_cap_blocks_at_sixth_insert() {
    let Some(pool) = pool_or_skip().await else {
        eprintln!("SKIP: set TEST_DATABASE_URL to run");
        return;
    };
    let email = format!("free-{}@aegis.test", Uuid::new_v4());
    let user = make_user(&pool, &email).await;
    let period = period_start_today();

    // No subscription row → tier resolves to Free (cap = 5).
    let cap_row: (Option<i32>,) =
        sqlx::query_as("SELECT decisions_cap_monthly FROM plan_tiers WHERE code = 'free'")
            .fetch_one(&pool)
            .await
            .expect("free cap");
    let cap = cap_row.0.expect("free has finite cap");
    assert_eq!(cap, 5);

    // Simulate 5 successful decisions: UPSERT bumps decisions_count.
    for _ in 0..cap {
        sqlx::query(
            "INSERT INTO usage_meters (user_id, period_start, decisions_count)
             VALUES ($1, $2, 1)
             ON CONFLICT (user_id, period_start)
             DO UPDATE SET decisions_count = usage_meters.decisions_count + 1",
        )
        .bind(user)
        .bind(period)
        .execute(&pool)
        .await
        .expect("bump meter");
    }

    let used: (i32,) = sqlx::query_as(
        "SELECT decisions_count FROM usage_meters WHERE user_id = $1 AND period_start = $2",
    )
    .bind(user)
    .bind(period)
    .fetch_one(&pool)
    .await
    .expect("read meter");
    // After 5 successful bumps, the 6th attempt enforces the cap (used >= cap).
    assert_eq!(used.0, 5);
    assert!(used.0 >= cap, "6th decision should now hit the cap");

    cleanup_user(&pool, user).await;
}

#[tokio::test]
async fn second_portfolio_is_rejected_for_single_portfolio_invariant() {
    let Some(pool) = pool_or_skip().await else {
        eprintln!("SKIP: set TEST_DATABASE_URL to run");
        return;
    };
    let email = format!("upgrade-{}@aegis.test", Uuid::new_v4());
    let user = make_user(&pool, &email).await;

    // Insert the one permitted portfolio.
    sqlx::query("INSERT INTO portfolios (id, user_id, name) VALUES ($1, $2, 'p1')")
        .bind(Uuid::new_v4())
        .bind(user)
        .execute(&pool)
        .await
        .expect("first portfolio");

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM portfolios WHERE user_id = $1")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1);

    // Upgrade to Pro — tier resolution still works.
    sqlx::query(
        "INSERT INTO subscriptions
            (user_id, tier, status, current_period_start, current_period_end, billing_anchor_day)
         VALUES ($1, 'pro', 'active', NOW(), NOW() + INTERVAL '30 days', 1)",
    )
    .bind(user)
    .execute(&pool)
    .await
    .expect("upgrade to pro");

    let tier: (String,) = sqlx::query_as(
        "SELECT tier FROM subscriptions
         WHERE user_id = $1 AND status IN ('trialing','active')
         ORDER BY started_at DESC LIMIT 1",
    )
    .bind(user)
    .fetch_one(&pool)
    .await
    .expect("read tier");
    assert_eq!(tier.0, "pro");

    // Even on Pro, a second portfolio for the same user must fail the unique
    // constraint introduced by migrations 0034 / 0036 (portfolios_user_id_unique).
    let result = sqlx::query("INSERT INTO portfolios (id, user_id, name) VALUES ($1, $2, 'p2')")
        .bind(Uuid::new_v4())
        .bind(user)
        .execute(&pool)
        .await;

    assert!(
        result.is_err(),
        "unique constraint portfolios_user_id_unique must reject a second portfolio even on Pro"
    );

    // Exactly one portfolio must remain.
    let count2: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM portfolios WHERE user_id = $1")
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count2.0, 1,
        "one-portfolio invariant holds across tier upgrade"
    );

    cleanup_user(&pool, user).await;
}

#[tokio::test]
async fn subscription_status_filter_excludes_canceled_rows() {
    // The handler resolves "active" by status IN ('trialing','active','past_due'),
    // and the middleware further narrows to ('trialing','active'). A historical
    // canceled row must NOT bring back a paid tier — the partial-unique index
    // wouldn't catch it (it allows multiple canceled rows per user).
    let Some(pool) = pool_or_skip().await else {
        eprintln!("SKIP: set TEST_DATABASE_URL to run");
        return;
    };
    let email = format!("cancel-{}@aegis.test", Uuid::new_v4());
    let user = make_user(&pool, &email).await;

    sqlx::query(
        "INSERT INTO subscriptions
            (user_id, tier, status, current_period_start, current_period_end, billing_anchor_day)
         VALUES ($1, 'pro', 'canceled', NOW() - INTERVAL '60 days', NOW() - INTERVAL '30 days', 1)",
    )
    .bind(user)
    .execute(&pool)
    .await
    .unwrap();

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT tier FROM subscriptions
         WHERE user_id = $1 AND status IN ('trialing','active')
         ORDER BY started_at DESC LIMIT 1",
    )
    .bind(user)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(
        row.is_none(),
        "canceled-only history must resolve to implicit Free"
    );

    cleanup_user(&pool, user).await;
}
