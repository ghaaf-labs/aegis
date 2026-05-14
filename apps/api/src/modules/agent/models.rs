use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct AgentDecision {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub reasoning: String,
    pub recommendation: serde_json::Value,
    pub confidence: f64,
    pub triggered_by: String,
    pub created_at: DateTime<Utc>,

    // Telemetry — populated by Sprint 1's OpenRouter pipeline. Nullable for
    // back-compat with rows persisted before migration 0002.
    #[sqlx(default)]
    pub model_slug: Option<String>,
    #[sqlx(default)]
    pub regime: Option<String>,
    #[sqlx(default)]
    pub prompt_tokens: Option<i32>,
    #[sqlx(default)]
    pub completion_tokens: Option<i32>,
    #[sqlx(default)]
    pub latency_ms: Option<i32>,
    #[sqlx(default)]
    pub critic_verdict: Option<serde_json::Value>,
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
    #[serde(default)]
    pub triggered_by: Option<String>,
}
