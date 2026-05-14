use serde::{Deserialize, Serialize};
use tracing::warn;
use uuid::Uuid;

use crate::db::Db;

/// Persist an analytics event. Best-effort: failures are logged but never
/// propagated, so analytics writes can't break the user's request path.
pub async fn emit(db: &Db, user_id: Option<Uuid>, event_name: &str, properties: serde_json::Value) {
    let result = sqlx::query(
        "INSERT INTO analytics_events (user_id, event_name, properties) VALUES ($1, $2, $3)",
    )
    .bind(user_id)
    .bind(event_name)
    .bind(&properties)
    .execute(db)
    .await;

    if let Err(e) = result {
        warn!("analytics emit failed for {event_name}: {e}");
    }
}

#[derive(Debug, Deserialize)]
pub struct ClientEventBody {
    pub event_name: String,
    #[serde(default)]
    pub properties: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventAccepted {
    pub accepted: bool,
}
