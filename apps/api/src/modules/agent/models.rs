use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
#[serde(rename_all = "camelCase")]
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

    /// Sprint 4: prices + holdings captured at decision time. Powers the
    /// outcome compressor's real-delta computation and the diary's
    /// counterfactual replay. Empty `{}` for legacy rows.
    #[sqlx(default)]
    pub snapshot: serde_json::Value,

    // F-CONF-4: calibrated confidence + critic counterfactual. All Optional
    // for back-compat with rows persisted before migration 0013.
    #[sqlx(default)]
    pub raw_confidence: Option<f64>,
    #[sqlx(default)]
    pub calibrated_confidence: Option<f64>,
    #[sqlx(default)]
    pub counterfactual: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeRequest {
    pub portfolio_id: Uuid,
    #[serde(default)]
    pub triggered_by: Option<String>,
}
