use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct AgentDecision {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub reasoning: String,
    pub recommendation: serde_json::Value,
    pub confidence: f64,
    pub triggered_by: String,
    pub created_at: DateTime<Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct Recommendation {
    pub summary: String,
    pub trades: Vec<ProposedTrade>,
    pub expected_impact: ExpectedImpact,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct ProposedTrade {
    pub symbol: String,
    pub action: String,
    pub quantity: f64,
    pub value_usd: f64,
    pub reason: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct ExpectedImpact {
    pub risk_delta: f64,
    pub diversification_score: f64,
}

#[derive(Debug, Deserialize)]
pub struct AnalyzeRequest {
    pub portfolio_id: Uuid,
}
