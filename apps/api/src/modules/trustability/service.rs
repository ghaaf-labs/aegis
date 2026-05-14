//! Trustability data access.
//!
//! `for_user` returns the per-user aggregate row from the leaderboard view.
//! Empty-history users get a `None` row so the handler can fall back to a
//! "not enough data yet" UI state cleanly.

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Serialize, Clone, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TrustabilityRow {
    pub user_id: Uuid,
    pub handle: String,
    pub decisions_executed: i64,
    pub distinct_models: i64,
    pub avg_7d_return: f64,
    pub trustability_delta: f64,
    pub last_decision_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn for_user(db: &PgPool, user_id: Uuid) -> sqlx::Result<Option<TrustabilityRow>> {
    sqlx::query_as::<_, TrustabilityRow>(
        "SELECT user_id, handle, decisions_executed, distinct_models,
                avg_7d_return, trustability_delta, last_decision_at
         FROM v_trustability_per_user
         WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await
}

pub async fn leaderboard(db: &PgPool, limit: i64) -> sqlx::Result<Vec<TrustabilityRow>> {
    sqlx::query_as::<_, TrustabilityRow>(
        "SELECT user_id, handle, decisions_executed, distinct_models,
                avg_7d_return, trustability_delta, last_decision_at
         FROM v_trustability_per_user
         ORDER BY trustability_delta DESC NULLS LAST, decisions_executed DESC
         LIMIT $1",
    )
    .bind(limit)
    .fetch_all(db)
    .await
}

/// Bucket the delta into a 5-tier label the UI can pin a color to. Lifts
/// the "is this score good?" question out of every consumer.
pub fn label_for_delta(delta: f64) -> &'static str {
    if delta >= 5.0 {
        "excellent"
    } else if delta >= 1.5 {
        "strong"
    } else if delta >= -1.5 {
        "stable"
    } else if delta >= -5.0 {
        "shaky"
    } else {
        "underperforming"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_for_delta_brackets() {
        assert_eq!(label_for_delta(10.0), "excellent");
        assert_eq!(label_for_delta(2.0), "strong");
        assert_eq!(label_for_delta(0.0), "stable");
        assert_eq!(label_for_delta(-2.0), "shaky");
        assert_eq!(label_for_delta(-10.0), "underperforming");
    }
}
