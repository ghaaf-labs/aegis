use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::modules::wallet_routes;
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
                                COALESCE(arc_route.address, base_route.address, '') AS wallet_address,
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
                         LEFT JOIN user_wallet_networks arc_route
                           ON arc_route.user_id = p.user_id
                          AND arc_route.blockchain = 'ARC-TESTNET'
                         LEFT JOIN user_wallet_networks base_route
                           ON base_route.user_id = p.user_id
                          AND base_route.blockchain = 'BASE-SEPOLIA'
                         LEFT JOIN agent_memory m ON m.decision_id = d.id";

pub async fn by_wallet(
    State(state): State<AppState>,
    Path(wallet): Path<String>,
) -> Result<Json<Vec<DiaryEntry>>> {
    let wallet = wallet.to_lowercase();
    let Some(user_id) = wallet_routes::user_id_for_address(&state.db, &wallet).await? else {
        return Ok(Json(vec![]));
    };
    let sql = format!(
        "{ROW_SELECT}
         WHERE p.user_id = $1
         ORDER BY d.created_at DESC LIMIT 50"
    );
    let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(row_to_entry).collect()))
}

pub async fn by_decision(
    State(state): State<AppState>,
    Path(decision_id): Path<Uuid>,
) -> Result<Json<DiaryEntry>> {
    let sql = format!("{ROW_SELECT} WHERE d.id = $1");
    let row: Option<Row> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(decision_id)
        .fetch_optional(&state.db)
        .await?;
    let row = row.ok_or_else(|| AppError::NotFound(format!("decision {decision_id}")))?;
    Ok(Json(row_to_entry(row)))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionFull {
    pub decision_id: Uuid,
    pub portfolio_id: Uuid,
    pub regime: Option<String>,
    pub model_slug: Option<String>,
    pub confidence: f64,
    pub raw_confidence: Option<f64>,
    pub calibrated_confidence: Option<f64>,
    pub prompt_tokens: Option<i32>,
    pub completion_tokens: Option<i32>,
    pub latency_ms: Option<i32>,
    /// Strategist's user-visible reasoning. The system prompt stays internal.
    pub prompt_excerpt: String,
    pub recommendation: serde_json::Value,
    pub critic_verdict: Option<serde_json::Value>,
    pub counterfactual: Option<String>,
    pub snapshot: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub outcome: Option<DiaryOutcome>,
    pub legs: Vec<DecisionLeg>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DecisionLeg {
    pub leg_index: i32,
    pub kind: String,
    pub src_chain: Option<String>,
    pub dest_chain: Option<String>,
    pub src_symbol: Option<String>,
    pub dest_symbol: Option<String>,
    pub amount_usdc: f64,
    pub status: String,
    pub tx_hash: Option<String>,
}

#[derive(sqlx::FromRow)]
struct FullRow {
    decision_id: Uuid,
    portfolio_id: Uuid,
    regime: Option<String>,
    model_slug: Option<String>,
    confidence: f64,
    raw_confidence: Option<f64>,
    calibrated_confidence: Option<f64>,
    prompt_tokens: Option<i32>,
    completion_tokens: Option<i32>,
    latency_ms: Option<i32>,
    reasoning: String,
    recommendation: serde_json::Value,
    critic_verdict: Option<serde_json::Value>,
    counterfactual: Option<String>,
    snapshot: serde_json::Value,
    created_at: DateTime<Utc>,
    outcome_24h: Option<serde_json::Value>,
    memory_recorded_at: Option<DateTime<Utc>>,
}

/// Full audit trail for a single decision. Public — same `diary_public`
/// gate as `by_decision`. Powers the /decision/<id> audit-trail panel.
pub async fn full_by_decision(
    State(state): State<AppState>,
    Path(decision_id): Path<Uuid>,
) -> Result<Json<DecisionFull>> {
    let row: Option<FullRow> = sqlx::query_as(
        "SELECT d.id              AS decision_id,
                d.portfolio_id    AS portfolio_id,
                d.regime          AS regime,
                d.model_slug      AS model_slug,
                d.confidence      AS confidence,
                d.raw_confidence  AS raw_confidence,
                d.calibrated_confidence AS calibrated_confidence,
                d.prompt_tokens   AS prompt_tokens,
                d.completion_tokens AS completion_tokens,
                d.latency_ms      AS latency_ms,
                d.reasoning       AS reasoning,
                d.recommendation  AS recommendation,
                d.critic_verdict  AS critic_verdict,
                d.counterfactual  AS counterfactual,
                d.snapshot        AS snapshot,
                d.created_at      AS created_at,
                m.outcome_24h     AS outcome_24h,
                m.recorded_at     AS memory_recorded_at
         FROM agent_decisions d
         JOIN portfolios p ON p.id = d.portfolio_id AND p.diary_public = TRUE
         LEFT JOIN agent_memory m ON m.decision_id = d.id
         WHERE d.id = $1",
    )
    .bind(decision_id)
    .fetch_optional(&state.db)
    .await?;
    let row = row.ok_or_else(|| AppError::NotFound(format!("decision {decision_id}")))?;

    let legs: Vec<DecisionLeg> = sqlx::query_as(
        "SELECT l.leg_index, l.kind, l.src_chain, l.dest_chain, l.src_symbol,
                l.dest_symbol, l.amount_usdc, l.status, l.tx_hash
         FROM rebalance_legs l
         JOIN rebalances r ON r.id = l.rebalance_id
         WHERE r.decision_id = $1
         ORDER BY l.leg_index",
    )
    .bind(decision_id)
    .fetch_all(&state.db)
    .await?;

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

    Ok(Json(DecisionFull {
        decision_id: row.decision_id,
        portfolio_id: row.portfolio_id,
        regime: row.regime,
        model_slug: row.model_slug,
        confidence: row.confidence,
        raw_confidence: row.raw_confidence,
        calibrated_confidence: row.calibrated_confidence,
        prompt_tokens: row.prompt_tokens,
        completion_tokens: row.completion_tokens,
        latency_ms: row.latency_ms,
        prompt_excerpt: row.reasoning,
        recommendation: row.recommendation,
        critic_verdict: row.critic_verdict,
        counterfactual: row.counterfactual,
        snapshot: row.snapshot,
        created_at: row.created_at,
        outcome,
        legs,
    }))
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
