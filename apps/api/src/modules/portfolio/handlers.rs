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

/// Create a portfolio owned by the authenticated user. Users can keep multiple
/// portfolios side-by-side: one custom goal, plus strategy-adopted targets for
/// comparison. The agent still requires per-portfolio human approval before
/// any deployment or rebalance executes.
pub async fn create(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreatePortfolioRequest>,
) -> crate::error::Result<(StatusCode, Json<Portfolio>)> {
    let goal_value = body.goal.clone().unwrap_or(serde_json::json!({}));

    let mut tx = state.db.begin().await?;
    let created = sqlx::query_as::<_, Portfolio>(
        "INSERT INTO portfolios (id, user_id, name, goal) VALUES ($1, $2, $3, $4)
         RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(claims.sub)
    .bind(&body.name)
    .bind(&goal_value)
    .fetch_one(&mut *tx)
    .await?;
    let portfolio_id = created.id;

    let target_symbols = body
        .allocations
        .iter()
        .map(|a| a.symbol.clone())
        .collect::<Vec<_>>();

    for alloc in &body.allocations {
        // Portfolio creation captures the target allocation only. Execution
        // updates real holdings later; trusting request quantity here makes
        // setup screens look invested before any approved leg confirms.
        sqlx::query(
            "INSERT INTO allocations
                (id, portfolio_id, asset_symbol, quantity, target_weight, current_weight, value_usd)
             VALUES ($1, $2, $3, $4, $5, 0, 0)
             ON CONFLICT (portfolio_id, asset_symbol) DO UPDATE
                SET target_weight = EXCLUDED.target_weight,
                    updated_at = NOW()",
        )
        .bind(Uuid::new_v4())
        .bind(portfolio_id)
        .bind(&alloc.symbol)
        .bind(0.0_f64)
        .bind(alloc.target_weight)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        "UPDATE allocations
         SET target_weight = 0,
             updated_at = NOW()
         WHERE portfolio_id = $1
           AND asset_symbol <> ALL($2)",
    )
    .bind(portfolio_id)
    .bind(&target_symbols)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "DELETE FROM allocations
         WHERE portfolio_id = $1
           AND target_weight = 0
           AND quantity = 0
           AND value_usd = 0
           AND asset_symbol <> ALL($2)",
    )
    .bind(portfolio_id)
    .bind(&target_symbols)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE allocations a
            SET current_weight = CASE
                WHEN totals.total_value_usd > 0
                THEN (a.value_usd / totals.total_value_usd) * 100
                ELSE 0
            END,
            updated_at = NOW()
         FROM (
            SELECT COALESCE(SUM(value_usd), 0)::DOUBLE PRECISION AS total_value_usd
            FROM allocations
            WHERE portfolio_id = $1
         ) totals
         WHERE a.portfolio_id = $1",
    )
    .bind(portfolio_id)
    .execute(&mut *tx)
    .await?;

    let portfolio = sqlx::query_as::<_, Portfolio>(
        "UPDATE portfolios p
            SET total_value_usd = COALESCE(
                (SELECT SUM(value_usd) FROM allocations WHERE portfolio_id = p.id),
                0
            ),
            updated_at = NOW()
         WHERE id = $1
         RETURNING *",
    )
    .bind(portfolio_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(portfolio)))
}

pub async fn update(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdatePortfolioRequest>,
) -> crate::error::Result<Json<Portfolio>> {
    let portfolio = sqlx::query_as::<_, Portfolio>(
        "UPDATE portfolios
         SET name = COALESCE($1, name),
             goal = COALESCE($2, goal),
             updated_at = NOW()
         WHERE id = $3 AND user_id = $4 RETURNING *",
    )
    .bind(body.name)
    .bind(body.goal)
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
