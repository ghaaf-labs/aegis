use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::models::StrategyPublic;

pub async fn list_public(pool: &PgPool) -> sqlx::Result<Vec<StrategyPublic>> {
    sqlx::query_as::<_, StrategyPublic>(
        "SELECT id, name, description, risk_band, min_horizon_months, \
                target_allocation, is_curated, created_at \
         FROM strategies \
         WHERE is_curated = TRUE \
         ORDER BY risk_band ASC, name ASC",
    )
    .fetch_all(pool)
    .await
}

pub async fn get(pool: &PgPool, id: Uuid) -> sqlx::Result<Option<StrategyPublic>> {
    sqlx::query_as::<_, StrategyPublic>(
        "SELECT id, name, description, risk_band, min_horizon_months, \
                target_allocation, is_curated, created_at \
         FROM strategies \
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Adopt a strategy: create a new portfolio owned by `user_id` with the
/// strategy's `target_allocation` cloned into the goal. Returns the new
/// portfolio's id. The user still approves every rebalance — adoption
/// only sets the starting target, never trades.
pub async fn adopt(pool: &PgPool, user_id: Uuid, strategy: &StrategyPublic) -> sqlx::Result<Uuid> {
    let mut tx = pool.begin().await?;

    let portfolio_id = Uuid::new_v4();
    let goal = serde_json::json!({
        "name": format!("{} (adopted)", strategy.name),
        "horizon": horizon_label(strategy.min_horizon_months),
        "riskTolerance": strategy.risk_band,
        "targetAllocation": strategy.target_allocation,
        "adoptedFromStrategy": strategy.id,
        "createdAt": chrono::Utc::now(),
    });

    sqlx::query(
        "INSERT INTO portfolios (id, user_id, name, total_value_usd, goal) \
         VALUES ($1, $2, $3, 0, $4)",
    )
    .bind(portfolio_id)
    .bind(user_id)
    .bind(&strategy.name)
    .bind(&goal)
    .execute(&mut *tx)
    .await?;

    if let Value::Object(map) = &strategy.target_allocation {
        for (symbol, weight) in map {
            let weight_pct = weight.as_f64().unwrap_or(0.0);
            sqlx::query(
                "INSERT INTO allocations \
                   (portfolio_id, asset_symbol, quantity, target_weight, current_weight) \
                 VALUES ($1, $2, 0, $3, 0)",
            )
            .bind(portfolio_id)
            .bind(symbol)
            .bind(weight_pct)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    Ok(portfolio_id)
}

fn horizon_label(min_months: i32) -> &'static str {
    match min_months {
        n if n <= 12 => "1y",
        n if n <= 36 => "3y",
        _ => "5y",
    }
}
