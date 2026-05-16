use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::router::AppState;

/// Public diary entry — exposed only for portfolios with `diary_public=true`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiaryEntry {
    pub decision_id: Uuid,
    pub portfolio_id: Uuid,
    pub wallet_address: String,
    pub regime: Option<String>,
    pub model_slug: Option<String>,
    pub confidence: f64,
    pub recommendation_summary: String,
    pub created_at: DateTime<Utc>,
    pub outcome: Option<DiaryOutcome>,
    pub critic_verdict: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiaryOutcome {
    pub realized_pct_change: f64,
    pub counterfactual_pct_change: f64,
    pub compressed_summary: String,
    pub recorded_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct Row {
    decision_id: Uuid,
    portfolio_id: Uuid,
    wallet_address: String,
    regime: Option<String>,
    model_slug: Option<String>,
    confidence: f64,
    recommendation: serde_json::Value,
    created_at: DateTime<Utc>,
    outcome_24h: Option<serde_json::Value>,
    memory_recorded_at: Option<DateTime<Utc>>,
    critic_verdict: Option<serde_json::Value>,
}

const ROW_SELECT: &str = "SELECT d.id              AS decision_id,
                                d.portfolio_id    AS portfolio_id,
                                COALESCE(u.arc_address, u.base_address, '') AS wallet_address,
                                d.regime          AS regime,
                                d.model_slug      AS model_slug,
                                d.confidence      AS confidence,
                                d.recommendation  AS recommendation,
                                d.created_at      AS created_at,
                                m.outcome_24h     AS outcome_24h,
                                m.recorded_at     AS memory_recorded_at,
                                d.critic_verdict  AS critic_verdict
                         FROM agent_decisions d
                         JOIN portfolios p ON p.id = d.portfolio_id AND p.diary_public = TRUE
                         JOIN users u      ON u.id = p.user_id
                         LEFT JOIN agent_memory m ON m.decision_id = d.id";

pub async fn by_wallet(
    State(state): State<AppState>,
    Path(wallet): Path<String>,
) -> Result<Json<Vec<DiaryEntry>>> {
    let wallet = wallet.to_lowercase();
    // Match either of the user's two MSCAs (arc + base). The old
    // COALESCE-first form would fail to find users-by-base when arc_address
    // was also populated.
    let sql = format!(
        "{ROW_SELECT}
         WHERE LOWER(u.arc_address) = $1 OR LOWER(u.base_address) = $1
         ORDER BY d.created_at DESC LIMIT 50"
    );
    let rows: Vec<Row> = sqlx::query_as(&sql)
        .bind(&wallet)
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(row_to_entry).collect()))
}

pub async fn by_decision(
    State(state): State<AppState>,
    Path(decision_id): Path<Uuid>,
) -> Result<Json<DiaryEntry>> {
    let sql = format!("{ROW_SELECT} WHERE d.id = $1");
    let row: Option<Row> = sqlx::query_as(&sql)
        .bind(decision_id)
        .fetch_optional(&state.db)
        .await?;
    let row = row.ok_or_else(|| AppError::NotFound(format!("decision {decision_id}")))?;
    Ok(Json(row_to_entry(row)))
}

fn row_to_entry(row: Row) -> DiaryEntry {
    let summary = row
        .recommendation
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("(no summary)")
        .to_string();
    let outcome = row.outcome_24h.as_ref().and_then(|j| {
        let realized = j.get("realizedPctChange")?.as_f64()?;
        let counterfactual = j.get("counterfactualPctChange")?.as_f64()?;
        let summary = j
            .get("compressedSummary")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        Some(DiaryOutcome {
            realized_pct_change: realized,
            counterfactual_pct_change: counterfactual,
            compressed_summary: summary,
            recorded_at: row.memory_recorded_at.unwrap_or_else(Utc::now),
        })
    });
    DiaryEntry {
        decision_id: row.decision_id,
        portfolio_id: row.portfolio_id,
        wallet_address: row.wallet_address,
        regime: row.regime,
        model_slug: row.model_slug,
        confidence: row.confidence,
        recommendation_summary: summary,
        created_at: row.created_at,
        outcome,
        critic_verdict: row.critic_verdict,
    }
}
