use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use uuid::Uuid;

use super::models::*;
use crate::{middleware::auth::Claims, router::AppState};

pub async fn list(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> crate::error::Result<Json<Vec<Portfolio>>> {
    let portfolios = sqlx::query_as::<_, Portfolio>(
        "SELECT * FROM portfolios WHERE user_id = $1 ORDER BY created_at DESC",
    )
    .bind(claims.sub)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(portfolios))
}

pub async fn get(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> crate::error::Result<Json<PortfolioWithAllocations>> {
    let portfolio =
        sqlx::query_as::<_, Portfolio>("SELECT * FROM portfolios WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(claims.sub)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| crate::error::AppError::NotFound(format!("portfolio {id}")))?;

    let allocations =
        sqlx::query_as::<_, Allocation>("SELECT * FROM allocations WHERE portfolio_id = $1")
            .bind(id)
            .fetch_all(&state.db)
            .await?;

    Ok(Json(PortfolioWithAllocations {
        portfolio,
        allocations,
    }))
}

pub async fn create(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreatePortfolioRequest>,
) -> crate::error::Result<(StatusCode, Json<Portfolio>)> {
    let goal_value = body.goal.clone().unwrap_or(serde_json::json!({}));
    let portfolio = sqlx::query_as::<_, Portfolio>(
        "INSERT INTO portfolios (id, user_id, name, goal) VALUES ($1, $2, $3, $4) RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(claims.sub)
    .bind(&body.name)
    .bind(&goal_value)
    .fetch_one(&state.db)
    .await?;

    for alloc in &body.allocations {
        sqlx::query(
            "INSERT INTO allocations (id, portfolio_id, asset_symbol, quantity, target_weight, current_weight, value_usd)
             VALUES ($1, $2, $3, $4, $5, $5, 0)",
        )
        .bind(Uuid::new_v4())
        .bind(portfolio.id)
        .bind(&alloc.symbol)
        .bind(alloc.quantity)
        .bind(alloc.target_weight)
        .execute(&state.db)
        .await?;
    }

    Ok((StatusCode::CREATED, Json(portfolio)))
}

pub async fn update(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdatePortfolioRequest>,
) -> crate::error::Result<Json<Portfolio>> {
    let portfolio = sqlx::query_as::<_, Portfolio>(
        "UPDATE portfolios SET name = COALESCE($1, name), updated_at = NOW()
         WHERE id = $2 AND user_id = $3 RETURNING *",
    )
    .bind(body.name)
    .bind(id)
    .bind(claims.sub)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| crate::error::AppError::NotFound(format!("portfolio {id}")))?;

    Ok(Json(portfolio))
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiaryPublicRequest {
    pub diary_public: bool,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiaryPublicResponse {
    pub id: Uuid,
    pub diary_public: bool,
}

pub async fn get_diary_public(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> crate::error::Result<Json<DiaryPublicResponse>> {
    let row: Option<(Uuid, bool)> =
        sqlx::query_as("SELECT id, diary_public FROM portfolios WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(claims.sub)
            .fetch_optional(&state.db)
            .await?;
    let (id, diary_public) =
        row.ok_or_else(|| crate::error::AppError::NotFound(format!("portfolio {id}")))?;
    Ok(Json(DiaryPublicResponse { id, diary_public }))
}

pub async fn set_diary_public(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<DiaryPublicRequest>,
) -> crate::error::Result<Json<DiaryPublicResponse>> {
    let row: Option<(Uuid, bool)> = sqlx::query_as(
        "UPDATE portfolios SET diary_public = $1, updated_at = NOW()
         WHERE id = $2 AND user_id = $3
         RETURNING id, diary_public",
    )
    .bind(body.diary_public)
    .bind(id)
    .bind(claims.sub)
    .fetch_optional(&state.db)
    .await?;
    let (id, diary_public) =
        row.ok_or_else(|| crate::error::AppError::NotFound(format!("portfolio {id}")))?;
    Ok(Json(DiaryPublicResponse { id, diary_public }))
}

pub async fn delete(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> crate::error::Result<StatusCode> {
    let result = sqlx::query("DELETE FROM portfolios WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(claims.sub)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(crate::error::AppError::NotFound(format!("portfolio {id}")));
    }

    Ok(StatusCode::NO_CONTENT)
}
