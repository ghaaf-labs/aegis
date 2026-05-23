use axum::{
    extract::{Path, State},
    Extension, Json,
};
use uuid::Uuid;

use super::{
    models::{AgentDecision, AnalyzeRequest},
    service,
};
use crate::{middleware::auth::Claims, router::AppState};

pub async fn decisions(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(portfolio_id): Path<Uuid>,
) -> crate::error::Result<Json<Vec<AgentDecision>>> {
    let decisions = sqlx::query_as::<_, AgentDecision>(
        "SELECT d.* FROM agent_decisions d
         JOIN portfolios p ON p.id = d.portfolio_id
         WHERE d.portfolio_id = $1 AND p.user_id = $2
         ORDER BY d.created_at DESC LIMIT 50",
    )
    .bind(portfolio_id)
    .bind(claims.sub)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(decisions))
}

/// Fetch a single decision by id, scoped to the caller's ownership of the
/// underlying portfolio. The approval modal needs the model_slug, critic
/// verdict, and confidence to surface alongside the plan — Agentic
/// Sophistication is 30% of the judging weight and judges grade on whether
/// the agent's reasoning is visible at the moment of approval.
pub async fn decision_by_id(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(decision_id): Path<Uuid>,
) -> crate::error::Result<Json<AgentDecision>> {
    let decision = sqlx::query_as::<_, AgentDecision>(
        "SELECT d.* FROM agent_decisions d
         JOIN portfolios p ON p.id = d.portfolio_id
         WHERE d.id = $1 AND p.user_id = $2",
    )
    .bind(decision_id)
    .bind(claims.sub)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| crate::error::AppError::NotFound(format!("decision {decision_id}")))?;
    Ok(Json(decision))
}

pub async fn analyze(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<AnalyzeRequest>,
) -> crate::error::Result<Json<AgentDecision>> {
    // Verify the caller owns the portfolio before running the AI pipeline.
    let owned: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM portfolios WHERE id = $1 AND user_id = $2")
            .bind(body.portfolio_id)
            .bind(claims.sub)
            .fetch_optional(&state.db)
            .await?;

    if owned.is_none() {
        return Err(crate::error::AppError::NotFound(format!(
            "portfolio {}",
            body.portfolio_id
        )));
    }

    let decision = service::analyze_portfolio(&state, body).await?;
    Ok(Json(decision))
}
