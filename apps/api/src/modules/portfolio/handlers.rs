use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use uuid::Uuid;

use super::models::*;
use crate::{
    config::Config, middleware::auth::Claims, modules::rebalance::registry::tokens,
    router::AppState,
};

pub async fn list(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> crate::error::Result<Json<Vec<Portfolio>>> {
    let portfolios = sqlx::query_as::<_, Portfolio>(
        "SELECT * FROM portfolios
         WHERE user_id = $1
         ORDER BY updated_at DESC, created_at DESC
         LIMIT 1",
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

/// Create or replace the authenticated user's single portfolio target. Aegis
/// keeps one portfolio per user; calling this endpoint again updates that
/// portfolio's name, goal, and target allocations instead of creating a second
/// portfolio.
pub async fn create(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreatePortfolioRequest>,
) -> crate::error::Result<(StatusCode, Json<Portfolio>)> {
    let goal_value = sanitize_goal_targets(
        body.goal.clone().unwrap_or(serde_json::json!({})),
        &state.config,
    );
    let allocations = sanitize_allocation_targets(&body.allocations, &state.config);

    let mut tx = state.db.begin().await?;
    let existing_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM portfolios
         WHERE user_id = $1
         ORDER BY updated_at DESC, created_at DESC
         LIMIT 1
         FOR UPDATE",
    )
    .bind(claims.sub)
    .fetch_optional(&mut *tx)
    .await?;
    let (portfolio_id, status_code) = if let Some(id) = existing_id {
        sqlx::query(
            "UPDATE portfolios
             SET name = $1,
                 goal = $2,
                 updated_at = NOW()
             WHERE id = $3",
        )
        .bind(&body.name)
        .bind(&goal_value)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        (id, StatusCode::OK)
    } else {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO portfolios (id, user_id, name, goal)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(id)
        .bind(claims.sub)
        .bind(&body.name)
        .bind(&goal_value)
        .execute(&mut *tx)
        .await?;
        (id, StatusCode::CREATED)
    };

    sqlx::query(
        "DELETE FROM portfolios
         WHERE user_id = $1
           AND id <> $2",
    )
    .bind(claims.sub)
    .bind(portfolio_id)
    .execute(&mut *tx)
    .await?;

    let target_symbols = allocations
        .iter()
        .map(|a| a.symbol.clone())
        .collect::<Vec<_>>();

    for alloc in &allocations {
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
    Ok((status_code, Json(portfolio)))
}

pub async fn update(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdatePortfolioRequest>,
) -> crate::error::Result<Json<Portfolio>> {
    let goal = body
        .goal
        .map(|goal| sanitize_goal_targets(goal, &state.config));
    let portfolio = sqlx::query_as::<_, Portfolio>(
        "UPDATE portfolios
         SET name = COALESCE($1, name),
             goal = COALESCE($2, goal),
             updated_at = NOW()
         WHERE id = $3 AND user_id = $4 RETURNING *",
    )
    .bind(body.name)
    .bind(goal)
    .bind(id)
    .bind(claims.sub)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| crate::error::AppError::NotFound(format!("portfolio {id}")))?;

    Ok(Json(portfolio))
}

fn sanitize_goal_targets(mut goal: serde_json::Value, config: &Config) -> serde_json::Value {
    if config.usyc_enabled {
        return goal;
    }

    sweep_goal_target(&mut goal, tokens::USYC);
    remove_route_target(&mut goal, tokens::USYC);
    if let Some(obj) = goal.as_object_mut() {
        obj.insert("includeUsyc".into(), serde_json::json!(false));
    }
    goal
}

fn sweep_goal_target(goal: &mut serde_json::Value, symbol: &str) {
    let Some(targets) = goal
        .get_mut("targetAllocation")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    let swept = targets
        .remove(symbol)
        .and_then(|value| value.as_f64())
        .filter(|weight| weight.is_finite() && *weight > 0.0)
        .unwrap_or(0.0);
    if swept <= 0.0 {
        return;
    }

    let current_usdc = targets
        .get(tokens::USDC)
        .and_then(serde_json::Value::as_f64)
        .filter(|weight| weight.is_finite())
        .unwrap_or(0.0);
    targets.insert(
        tokens::USDC.to_string(),
        serde_json::json!(round_weight(current_usdc + swept)),
    );
}

fn remove_route_target(goal: &mut serde_json::Value, symbol: &str) {
    let Some(route_preferences) = goal
        .get_mut("routePreferences")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };

    remove_string_from_array(route_preferences.get_mut("tokens"), symbol);
    add_string_to_array(route_preferences, "watchlist", symbol);
}

fn remove_string_from_array(value: Option<&mut serde_json::Value>, symbol: &str) {
    if let Some(items) = value.and_then(serde_json::Value::as_array_mut) {
        items.retain(|item| {
            item.as_str()
                .is_none_or(|value| !value.eq_ignore_ascii_case(symbol))
        });
    }
}

fn add_string_to_array(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    symbol: &str,
) {
    let entry = object
        .entry(key.to_string())
        .or_insert_with(|| serde_json::json!([]));
    let Some(items) = entry.as_array_mut() else {
        return;
    };
    if !items.iter().any(|item| {
        item.as_str()
            .is_some_and(|value| value.eq_ignore_ascii_case(symbol))
    }) {
        items.push(serde_json::json!(symbol));
    }
}

fn sanitize_allocation_targets(
    allocations: &[AllocationInput],
    config: &Config,
) -> Vec<AllocationInput> {
    if config.usyc_enabled {
        return allocations
            .iter()
            .map(|alloc| AllocationInput {
                symbol: alloc.symbol.clone(),
                quantity: alloc.quantity,
                target_weight: alloc.target_weight,
            })
            .collect();
    }

    let mut swept = 0.0;
    let mut sanitized = Vec::with_capacity(allocations.len());
    for alloc in allocations {
        if alloc.symbol.eq_ignore_ascii_case(tokens::USYC) {
            if alloc.target_weight.is_finite() && alloc.target_weight > 0.0 {
                swept += alloc.target_weight;
            }
            continue;
        }
        sanitized.push(AllocationInput {
            symbol: alloc.symbol.clone(),
            quantity: alloc.quantity,
            target_weight: alloc.target_weight,
        });
    }

    if swept > 0.0 {
        if let Some(usdc) = sanitized
            .iter_mut()
            .find(|alloc| alloc.symbol.eq_ignore_ascii_case(tokens::USDC))
        {
            usdc.target_weight = round_weight(usdc.target_weight + swept);
        } else {
            sanitized.push(AllocationInput {
                symbol: tokens::USDC.to_string(),
                quantity: 0.0,
                target_weight: round_weight(swept),
            });
        }
    }

    sanitized
}

fn round_weight(weight: f64) -> f64 {
    (weight * 100.0).round() / 100.0
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{sanitize_allocation_targets, sanitize_goal_targets, AllocationInput};

    #[test]
    fn sanitize_goal_targets_sweeps_disabled_usyc_to_usdc() {
        let cfg = crate::config::test_config();
        let goal = json!({
            "includeUsyc": true,
            "targetAllocation": {
                "EURC": 10,
                "USDC": 70,
                "USYC": 20
            },
            "routePreferences": {
                "tokens": ["USDC", "USYC"],
                "watchlist": ["EURC"]
            }
        });

        let sanitized = sanitize_goal_targets(goal, &cfg);

        assert_eq!(sanitized.pointer("/targetAllocation/USYC"), None);
        assert_eq!(
            sanitized
                .pointer("/targetAllocation/USDC")
                .and_then(|v| v.as_f64()),
            Some(90.0)
        );
        assert_eq!(
            sanitized
                .pointer("/targetAllocation/EURC")
                .and_then(|v| v.as_f64()),
            Some(10.0)
        );
        assert_eq!(
            sanitized.get("includeUsyc").and_then(|v| v.as_bool()),
            Some(false)
        );
        let tokens = sanitized
            .pointer("/routePreferences/tokens")
            .and_then(|v| v.as_array())
            .expect("tokens array");
        assert!(!tokens.iter().any(|v| v.as_str() == Some("USYC")));
        let watchlist = sanitized
            .pointer("/routePreferences/watchlist")
            .and_then(|v| v.as_array())
            .expect("watchlist array");
        assert!(watchlist.iter().any(|v| v.as_str() == Some("USYC")));
    }

    #[test]
    fn sanitize_allocation_targets_sweeps_disabled_usyc_to_usdc() {
        let cfg = crate::config::test_config();
        let allocations = vec![
            AllocationInput {
                symbol: "EURC".into(),
                quantity: 0.0,
                target_weight: 10.0,
            },
            AllocationInput {
                symbol: "USDC".into(),
                quantity: 0.0,
                target_weight: 70.0,
            },
            AllocationInput {
                symbol: "USYC".into(),
                quantity: 0.0,
                target_weight: 20.0,
            },
        ];

        let sanitized = sanitize_allocation_targets(&allocations, &cfg);

        assert!(!sanitized.iter().any(|a| a.symbol == "USYC"));
        assert_eq!(
            sanitized
                .iter()
                .find(|a| a.symbol == "USDC")
                .map(|a| a.target_weight),
            Some(90.0)
        );
    }
}
