use axum::{extract::{Path, State}, Extension, Json};
use uuid::Uuid;

use crate::{middleware::auth::Claims, router::AppState};
use crate::modules::agent::{models::AnalyzeRequest, service::analyze_portfolio};

pub mod handlers {
    use super::*;

    pub async fn trigger(
        State(state): State<AppState>,
        Extension(claims): Extension<Claims>,
        Path(portfolio_id): Path<Uuid>,
    ) -> crate::error::Result<Json<crate::modules::agent::models::AgentDecision>> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM portfolios WHERE id = $1 AND user_id = $2)",
        )
        .bind(portfolio_id)
        .bind(claims.sub)
        .fetch_one(&state.db)
        .await?;

        if !exists {
            return Err(crate::error::AppError::NotFound(format!("portfolio {portfolio_id}")));
        }

        let decision = analyze_portfolio(&state, AnalyzeRequest { portfolio_id }).await?;
        Ok(Json(decision))
    }
}
