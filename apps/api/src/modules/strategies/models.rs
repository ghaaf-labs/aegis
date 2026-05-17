use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct StrategyPublic {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub risk_band: String,
    pub min_horizon_months: i32,
    pub target_allocation: serde_json::Value,
    pub is_curated: bool,
    pub created_at: DateTime<Utc>,
}
