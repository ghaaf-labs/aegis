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

/// Adopt a strategy by replacing the user's single portfolio target. The user
/// still approves every rebalance — adoption only sets the target, never trades.
pub async fn adopt(pool: &PgPool, user_id: Uuid, strategy: &StrategyPublic) -> sqlx::Result<Uuid> {
    let mut tx = pool.begin().await?;

    let target_allocation = target_without_coming_soon(&strategy.target_allocation);
    let goal = serde_json::json!({
        "name": format!("{} (adopted)", strategy.name),
        "horizon": horizon_label(strategy.min_horizon_months),
        "riskTolerance": strategy.risk_band,
        "targetAllocation": target_allocation,
        "includeUsyc": false,
        "adoptedFromStrategy": strategy.id,
        "createdAt": chrono::Utc::now(),
    });

    let existing_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM portfolios
         WHERE user_id = $1
         ORDER BY updated_at DESC, created_at DESC
         LIMIT 1
         FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;
    let portfolio_id = if let Some(id) = existing_id {
        sqlx::query(
            "UPDATE portfolios
             SET name = $1,
                 goal = $2,
                 updated_at = NOW()
             WHERE id = $3",
        )
        .bind(&strategy.name)
        .bind(&goal)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        id
    } else {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO portfolios (id, user_id, name, total_value_usd, goal)
             VALUES ($1, $2, $3, 0, $4)",
        )
        .bind(id)
        .bind(user_id)
        .bind(&strategy.name)
        .bind(&goal)
        .execute(&mut *tx)
        .await?;
        id
    };

    sqlx::query(
        "DELETE FROM portfolios
         WHERE user_id = $1
           AND id <> $2",
    )
    .bind(user_id)
    .bind(portfolio_id)
    .execute(&mut *tx)
    .await?;

    if let Value::Object(map) = &target_allocation {
        let target_symbols = map.keys().cloned().collect::<Vec<_>>();
        for (symbol, weight) in map {
            let weight_pct = weight.as_f64().unwrap_or(0.0);
            sqlx::query(
                "INSERT INTO allocations \
                   (portfolio_id, asset_symbol, quantity, target_weight, current_weight) \
                 VALUES ($1, $2, 0, $3, 0)
                 ON CONFLICT (portfolio_id, asset_symbol) DO UPDATE
                    SET target_weight = EXCLUDED.target_weight,
                        updated_at = NOW()",
            )
            .bind(portfolio_id)
            .bind(symbol)
            .bind(weight_pct)
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query(
            "DELETE FROM allocations
             WHERE portfolio_id = $1
               AND quantity = 0
               AND value_usd = 0
               AND asset_symbol <> ALL($2)",
        )
        .bind(portfolio_id)
        .bind(&target_symbols)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(portfolio_id)
}

fn target_without_coming_soon(target: &Value) -> Value {
    let Value::Object(map) = target else {
        return serde_json::json!({"USDC": 100.0});
    };
    let mut next = serde_json::Map::new();
    let mut usdc = map.get("USDC").and_then(Value::as_f64).unwrap_or(0.0);
    for (symbol, value) in map {
        if symbol == "USYC" {
            usdc += value.as_f64().unwrap_or(0.0);
            continue;
        }
        if symbol != "USDC" {
            next.insert(symbol.clone(), value.clone());
        }
    }
    next.insert("USDC".into(), serde_json::json!(usdc));
    Value::Object(next)
}

fn horizon_label(min_months: i32) -> &'static str {
    match min_months {
        n if n <= 12 => "1y",
        n if n <= 36 => "3y",
        _ => "5y",
    }
}
