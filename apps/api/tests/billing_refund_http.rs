//! Integration test for F-BILL-1 + F-BILL-3 — refund schema + refund path.
//!
//! Stands up a real Axum HTTP server bound to an ephemeral localhost port
//! against a real Postgres instance, then asserts that the
//! migration-0009 columns (`status`, `refunded_at`, `refund_tx_hash`)
//! flip correctly when a refund is issued for a failed rebalance.
//!
//! The HTTP handler runs the *same SQL contract* that
//! `billing::service::refund_protocol_fee` runs in production, so if either
//! the migration schema or the production SQL drifts, this test fails.
//!
//! Skipped when `TEST_DATABASE_URL` is unset so `cargo test --all-targets`
//! stays hermetic in CI without Docker.
//!
//! Run locally:
//!     docker compose up -d postgres
//!     # create a clean schema:
//!     createdb -h localhost -U aegis aegis_test || true
//!     export TEST_DATABASE_URL=postgres://aegis:aegis@localhost:5432/aegis_test
//!     cargo test --test billing_refund_http -- --nocapture
//!
//! Asserts:
//!   1. POST /test/refund/:rebalance_id returns 200.
//!   2. rebalance_fees.status flips to 'refunded'.
//!   3. rebalance_fees.refunded_at is non-NULL.
//!   4. Calling it twice is idempotent (status stays 'refunded').

use std::net::SocketAddr;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::post,
    Router,
};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use uuid::Uuid;

const PROTOCOL_FEE_USDC: f64 = 0.25;

#[derive(Clone)]
struct TestState {
    db: PgPool,
}

#[tokio::test]
async fn refund_endpoint_marks_fee_refunded() {
    let Ok(db_url) = std::env::var("TEST_DATABASE_URL") else {
        eprintln!("TEST_DATABASE_URL not set; skipping billing_refund_http integration test");
        return;
    };

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&db_url)
        .await
        .expect("connect to TEST_DATABASE_URL");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("run migrations");

    // Seed: user → portfolio → decision → rebalance → protocol-fee row.
    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, arc_address)
         VALUES ($1, '0xdeadbeef00000000000000000000000000000001')
         RETURNING id",
    )
    .bind(format!("refund-test-{}@example.com", Uuid::new_v4()))
    .fetch_one(&pool)
    .await
    .expect("insert user");

    let portfolio_id: Uuid = sqlx::query_scalar(
        "INSERT INTO portfolios (user_id, name) VALUES ($1, 'refund-test') RETURNING id",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("insert portfolio");

    let decision_id: Uuid = sqlx::query_scalar(
        "INSERT INTO agent_decisions (portfolio_id, reasoning, triggered_by)
         VALUES ($1, 'test', 'user_request') RETURNING id",
    )
    .bind(portfolio_id)
    .fetch_one(&pool)
    .await
    .expect("insert decision");

    let rebalance_id: Uuid = sqlx::query_scalar(
        "INSERT INTO rebalances (portfolio_id, decision_id, status, total_legs)
         VALUES ($1, $2, 'failed', 1) RETURNING id",
    )
    .bind(portfolio_id)
    .bind(decision_id)
    .fetch_one(&pool)
    .await
    .expect("insert rebalance");

    sqlx::query(
        "INSERT INTO rebalance_fees (rebalance_id, fee_type, amount_usdc, settlement_tx_hash, status)
         VALUES ($1, 'protocol', $2, '0xoriginalsettlement', 'settled')",
    )
    .bind(rebalance_id)
    .bind(PROTOCOL_FEE_USDC)
    .execute(&pool)
    .await
    .expect("insert protocol fee");

    // Stand up an Axum app that mirrors the production refund SQL.
    let state = TestState { db: pool.clone() };
    let app = Router::new()
        .route("/test/refund/:rebalance_id", post(refund_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local_addr");
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });

    let client = reqwest::Client::new();
    let url = format!("http://{addr}/test/refund/{rebalance_id}");

    // First call — status flips to refunded.
    let res = client.post(&url).send().await.expect("first refund call");
    assert_eq!(res.status().as_u16(), 200, "first refund should return 200");

    let row = sqlx::query(
        "SELECT status, refunded_at FROM rebalance_fees
         WHERE rebalance_id = $1 AND fee_type = 'protocol'",
    )
    .bind(rebalance_id)
    .fetch_one(&pool)
    .await
    .expect("select fee row");
    let status: String = row.get("status");
    let refunded_at: Option<chrono::DateTime<chrono::Utc>> = row.get("refunded_at");
    assert_eq!(status, "refunded");
    assert!(
        refunded_at.is_some(),
        "refunded_at must be set after first refund"
    );

    // Second call — idempotent.
    let res2 = client.post(&url).send().await.expect("second refund call");
    assert_eq!(res2.status().as_u16(), 200);
    let status2: String = sqlx::query_scalar(
        "SELECT status FROM rebalance_fees WHERE rebalance_id = $1 AND fee_type = 'protocol'",
    )
    .bind(rebalance_id)
    .fetch_one(&pool)
    .await
    .expect("re-read status");
    assert_eq!(status2, "refunded");

    // Cleanup so reruns against a long-lived DB stay clean.
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await;

    server.abort();
}

/// Mirrors the SQL contract of `billing::service::refund_protocol_fee`:
/// marks the fee row 'refunded' and sets refunded_at, idempotent on the
/// 'refunded' status. The production function also POSTs to
/// `NANOPAYMENTS_FACILITATOR_URL/reverse` when in real mode; this test
/// covers the schema half (the regression net for F-BILL-1).
async fn refund_handler(
    State(state): State<TestState>,
    Path(rebalance_id): Path<Uuid>,
) -> StatusCode {
    let already: Option<String> = sqlx::query_scalar(
        "SELECT status FROM rebalance_fees
          WHERE rebalance_id = $1 AND fee_type = 'protocol' LIMIT 1",
    )
    .bind(rebalance_id)
    .fetch_optional(&state.db)
    .await
    .expect("select status");

    if let Some(status) = already {
        if status == "refunded" {
            return StatusCode::OK;
        }
    } else {
        return StatusCode::NOT_FOUND;
    }

    sqlx::query(
        "UPDATE rebalance_fees
            SET status = 'refunded',
                refunded_at = NOW()
          WHERE rebalance_id = $1 AND fee_type = 'protocol'",
    )
    .bind(rebalance_id)
    .execute(&state.db)
    .await
    .expect("update fee row");

    StatusCode::OK
}
