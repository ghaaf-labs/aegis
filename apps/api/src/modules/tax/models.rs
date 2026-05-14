use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CostBasisLot {
    pub id: Uuid,
    pub allocation_id: Uuid,
    pub acquired_at: DateTime<Utc>,
    pub quantity: f64,
    pub basis_usd: f64,
    pub disposed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarvestableLot {
    pub lot_id: Uuid,
    pub acquired_at: DateTime<Utc>,
    pub quantity: f64,
    pub basis_usd: f64,
    pub current_value_usd: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarvestableLoss {
    pub portfolio_id: Uuid,
    pub allocation_id: Uuid,
    pub symbol: String,
    pub unrealized_loss_usd: f64,
    pub lots: Vec<HarvestableLot>,
}
