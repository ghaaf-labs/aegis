use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Portfolio {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub total_value_usd: f64,
    pub total_pnl_usd: f64,
    pub total_pnl_pct: f64,
    pub risk_score: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Allocation {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub asset_symbol: String,
    pub quantity: f64,
    pub target_weight: f64,
    pub current_weight: f64,
    pub value_usd: f64,
}

#[derive(Debug, Serialize)]
pub struct PortfolioWithAllocations {
    #[serde(flatten)]
    pub portfolio: Portfolio,
    #[allow(dead_code)]
    pub allocations: Vec<Allocation>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePortfolioRequest {
    pub name: String,
    pub allocations: Vec<AllocationInput>,
}

#[derive(Debug, Deserialize)]
pub struct AllocationInput {
    pub symbol: String,
    pub quantity: f64,
    pub target_weight: f64,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePortfolioRequest {
    pub name: Option<String>,
    #[allow(dead_code)]
    pub allocations: Option<Vec<AllocationInput>>,
}
