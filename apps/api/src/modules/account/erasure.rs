use std::time::Duration;

use sqlx::Row;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::router::AppState;

const ACCOUNT_ERASURE_GRACE_DAYS: i32 = 7;
const ERASURE_TICK_SECS: u64 = 3600;
const PROCESS_DUE_ERASURES_SQL: &str = r#"
    WITH due AS (
        SELECT id
        FROM users
        WHERE deletion_requested_at IS NOT NULL
          AND anonymized_at IS NULL
          AND deletion_requested_at <= NOW() - ($1::int || ' days')::interval
    ),
    revoked AS (
        UPDATE auth_sessions
        SET revoked_at = COALESCE(revoked_at, NOW())
        WHERE user_id IN (SELECT id FROM due)
          AND revoked_at IS NULL
        RETURNING user_id
    ),
    scrubbed AS (
        UPDATE users u
        SET email = CONCAT('deleted+', u.id::text, '@deleted.aegis.local'),
            marketing_opt_in = FALSE,
            anonymized_at = NOW(),
            updated_at = NOW()
        FROM due
        WHERE u.id = due.id
        RETURNING u.id
    )
    SELECT id FROM scrubbed
"#;

pub fn spawn_erasure_reconciler(state: AppState, cancel: CancellationToken) {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = tokio::time::sleep(Duration::from_secs(ERASURE_TICK_SECS)) => {}
            }

            match process_due_erasures(&state).await {
                Ok(count) if count > 0 => {
                    tracing::info!(count, "account erasure reconciler anonymized accounts");
                }
                Ok(_) => {}
                Err(e) => tracing::warn!(error=%e, "account erasure reconciler failed"),
            }
        }
    });
}

pub async fn process_due_erasures(state: &AppState) -> crate::error::Result<u64> {
    let rows = sqlx::query(PROCESS_DUE_ERASURES_SQL)
        .bind(ACCOUNT_ERASURE_GRACE_DAYS)
        .fetch_all(&state.db)
        .await?;

    let mut count = 0;
    for row in rows {
        let user_id: Uuid = row.try_get("id")?;
        count += 1;
        crate::modules::analytics::service::emit(
            &state.db,
            Some(user_id),
            "account.delete_completed",
            serde_json::json!({}),
        )
        .await;
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn due_erasure_query_revokes_sessions_and_anonymizes_email() {
        assert!(PROCESS_DUE_ERASURES_SQL.contains("auth_sessions"));
        assert!(PROCESS_DUE_ERASURES_SQL.contains("revoked_at"));
        assert!(PROCESS_DUE_ERASURES_SQL.contains("deleted+"));
        assert!(PROCESS_DUE_ERASURES_SQL.contains("anonymized_at = NOW()"));
        assert!(PROCESS_DUE_ERASURES_SQL.contains("deletion_requested_at <= NOW()"));
    }
}
