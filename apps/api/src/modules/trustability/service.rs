//! Trustability data access.
//!
//! `for_user` returns the per-user aggregate row from the leaderboard view.
//! Empty-history users get a `None` row so the handler can fall back to a
//! "not enough data yet" UI state cleanly.

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

pub const CALIBRATION_FLOOR: i64 = 50;

#[derive(Debug, Serialize, Clone, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TrustabilityRow {
    pub user_id: Uuid,
    pub handle: String,
    pub decisions_executed: i64,
    /// Real, completed decisions in the view's 7-day window — turnover read as
    /// decisions/week. Mirrors `decisions_executed` (same window); surfaced
    /// separately so the UI labels it as turnover.
    #[sqlx(default)]
    pub decisions_per_week: i64,
    pub distinct_models: i64,
    pub avg_7d_return: f64,
    pub trustability_delta: f64,
    pub last_decision_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Assets under management — SUM of this user's portfolios' total_value_usd.
    #[sqlx(default)]
    pub aum_usd: f64,
    /// Most recent model_slug used by this user in the last 7 days.
    /// Powers ModelBadge on the public leaderboard.
    #[sqlx(default)]
    pub recent_model_slug: Option<String>,
    /// Most recent critic verdict for this user.
    /// Powers critic pill on the public leaderboard.
    #[sqlx(default)]
    pub recent_critic_verdict: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Clone, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TrustabilityProgress {
    pub calibration_floor: i64,
    pub agent_decisions_7d: i64,
    pub eligible_outcomes_7d: i64,
    pub pending_real_rebalances_7d: i64,
    pub completed_real_rebalances_7d: i64,
    pub distinct_models_7d: i64,
    pub last_decision_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for TrustabilityProgress {
    fn default() -> Self {
        Self {
            calibration_floor: CALIBRATION_FLOOR,
            agent_decisions_7d: 0,
            eligible_outcomes_7d: 0,
            pending_real_rebalances_7d: 0,
            completed_real_rebalances_7d: 0,
            distinct_models_7d: 0,
            last_decision_at: None,
        }
    }
}

pub async fn for_user(db: &PgPool, user_id: Uuid) -> sqlx::Result<Option<TrustabilityRow>> {
    sqlx::query_as::<_, TrustabilityRow>(
        "SELECT user_id, handle, decisions_executed, decisions_per_week, distinct_models,
                avg_7d_return, trustability_delta, last_decision_at, aum_usd
         FROM v_trustability_per_user
         WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await
}

pub async fn progress_for_user(db: &PgPool, user_id: Uuid) -> sqlx::Result<TrustabilityProgress> {
    let progress = sqlx::query_as::<_, TrustabilityProgress>(
        r#"
        SELECT
            $2::bigint AS calibration_floor,
            count(DISTINCT d.id) FILTER (
                WHERE d.id IS NOT NULL AND d.triggered_by <> 'abstain'
            ) AS agent_decisions_7d,
            count(DISTINCT d.id) FILTER (
                WHERE d.id IS NOT NULL
                  AND d.triggered_by <> 'abstain'
                  AND EXISTS (
                      SELECT 1
                      FROM rebalances done
                      WHERE done.decision_id = d.id
                        AND done.status = 'completed'
                        AND done.execution_mode = 'real'
                  )
            ) AS eligible_outcomes_7d,
            count(DISTINCT rb.id) FILTER (
                WHERE rb.execution_mode = 'real'
                  AND rb.status IN ('planned', 'approved', 'executing')
            ) AS pending_real_rebalances_7d,
            count(DISTINCT rb.id) FILTER (
                WHERE rb.execution_mode = 'real' AND rb.status = 'completed'
            ) AS completed_real_rebalances_7d,
            count(DISTINCT d.model_slug) FILTER (
                WHERE d.model_slug IS NOT NULL AND d.triggered_by <> 'abstain'
            ) AS distinct_models_7d,
            max(d.created_at) FILTER (
                WHERE d.id IS NOT NULL AND d.triggered_by <> 'abstain'
            ) AS last_decision_at
        FROM users u
        LEFT JOIN portfolios p ON p.user_id = u.id
        LEFT JOIN agent_decisions d
            ON d.portfolio_id = p.id
           AND d.created_at > now() - INTERVAL '7 days'
        LEFT JOIN rebalances rb ON rb.decision_id = d.id
        WHERE u.id = $1
        GROUP BY u.id
        "#,
    )
    .bind(user_id)
    .bind(CALIBRATION_FLOOR)
    .fetch_optional(db)
    .await?;

    Ok(progress.unwrap_or_default())
}

pub async fn leaderboard(db: &PgPool, limit: i64) -> sqlx::Result<Vec<TrustabilityRow>> {
    // Enhanced query that also fetches the most recent model_slug and critic verdict
    // used in the last 7 days. This powers ModelBadge + critic pill on the public leaderboard.
    sqlx::query_as::<_, TrustabilityRow>(
        r#"
        WITH recent AS (
            SELECT DISTINCT ON (p.user_id) 
                   p.user_id,
                   d.model_slug,
                   d.critic_verdict
            FROM agent_decisions d
            JOIN portfolios p ON p.id = d.portfolio_id
            WHERE d.created_at > NOW() - INTERVAL '7 days'
              AND d.model_slug IS NOT NULL
            ORDER BY p.user_id, d.created_at DESC
        )
        SELECT
            v.user_id,
            v.handle,
            v.decisions_executed,
            v.decisions_per_week,
            v.distinct_models,
            v.avg_7d_return,
            v.trustability_delta,
            v.last_decision_at,
            v.aum_usd,
            r.model_slug AS recent_model_slug,
            r.critic_verdict AS recent_critic_verdict
        FROM v_trustability_per_user v
        LEFT JOIN recent r ON r.user_id = v.user_id
        ORDER BY v.trustability_delta DESC NULLS LAST, v.decisions_executed DESC
        LIMIT $1
        "#,
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
