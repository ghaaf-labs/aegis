use axum::{
    extract::{Path, State},
    Extension, Json,
};
use uuid::Uuid;

use crate::{middleware::auth::Claims, router::AppState};
use super::{models::{AgentDecision, AnalyzeRequest}, service};

pub async fn decisions(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(portfolio_id): Path<Uuid>,
) -> crate::error::Result<Json<Vec<AgentDecision>>> {
    let decisions = sqlx::query_as::<_, AgentDecision>(
        "SELECT * FROM agent_decisions WHERE portfolio_id = $1 ORDER BY created_at DESC LIMIT 50",
    )
    .bind(portfolio_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(decisions))
}

pub async fn analyze(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(body): Json<AnalyzeRequest>,
) -> crate::error::Result<Json<AgentDecision>> {
    let decision = service::analyze_portfolio(&state, body).await?;
    Ok(Json(decision))
}
