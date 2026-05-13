// Service-layer helpers for portfolio operations.
// Portfolio CRUD is currently handled inline in handlers.rs for hackathon speed.
// Extract complex business logic here as it grows.

use crate::{db::Db, error::Result};
use super::models::Portfolio;
use uuid::Uuid;

pub async fn recalculate_value(db: &Db, portfolio_id: Uuid) -> Result<Portfolio> {
    let portfolio = sqlx::query_as::<_, Portfolio>(
        r#"
        UPDATE portfolios p
        SET
            total_value_usd = COALESCE(
                (SELECT SUM(value_usd) FROM allocations WHERE portfolio_id = p.id), 0
            ),
            updated_at = NOW()
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(portfolio_id)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| crate::error::AppError::NotFound(format!("portfolio {portfolio_id}")))?;

    Ok(portfolio)
}
