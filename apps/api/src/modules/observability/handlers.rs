use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;

use super::counters::render_prometheus;
use crate::router::AppState;

pub async fn metrics() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, "text/plain; version=0.0.4")],
        render_prometheus(),
    )
}

/// Live, DB-backed traction metrics for the public landing page. Replaces the
/// previously-hardcoded "portfolios / decisions / $ managed / chains" stats so
/// the product never shows fabricated numbers. Best-effort: a failed sub-query
/// degrades that field to 0 rather than failing the whole response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Traction {
    /// Distinct registered users.
    pub total_users: i64,
    /// Users with at least one real, completed rebalance (real traction).
    pub users_with_real_rebalance: i64,
    /// Portfolios under management.
    pub portfolios: i64,
    /// Total assets under management (USD), summed across all portfolios.
    pub total_aum_usd: f64,
    /// Agent decisions made.
    pub agent_decisions: i64,
    /// Completed, real (non-mock) rebalances.
    pub completed_real_rebalances: i64,
    /// Execution settlement chains (Arc testnet + Base Sepolia today).
    pub chains: i64,
}

async fn count(db: &crate::db::Db, sql: &'static str) -> i64 {
    sqlx::query_scalar::<_, i64>(sql)
        .fetch_one(db)
        .await
        .unwrap_or(0)
}

pub async fn traction(State(state): State<AppState>) -> Json<Traction> {
    let db = &state.db;
    let total_aum_usd: f64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(total_value_usd), 0)::float8 FROM portfolios")
            .fetch_one(db)
            .await
            .unwrap_or(0.0);

    Json(Traction {
        total_users: count(db, "SELECT COUNT(*) FROM users").await,
        users_with_real_rebalance: count(
            db,
            "SELECT COUNT(DISTINCT p.user_id)
             FROM rebalances r JOIN portfolios p ON p.id = r.portfolio_id
             WHERE r.status = 'completed' AND r.execution_mode = 'real'",
        )
        .await,
        portfolios: count(db, "SELECT COUNT(*) FROM portfolios").await,
        total_aum_usd,
        agent_decisions: count(db, "SELECT COUNT(*) FROM agent_decisions").await,
        completed_real_rebalances: count(
            db,
            "SELECT COUNT(*) FROM rebalances WHERE status = 'completed' AND execution_mode = 'real'",
        )
        .await,
        // Arc testnet + Base Sepolia are the live execution chains.
        chains: 2,
    })
}
