use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// All events the API streams over `/sse`. Mirrors the `SseEvent`
/// discriminated union in `packages/shared/src/types.ts`.
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum SseEvent {
    PriceTick(PriceTick),
    RegimeFlip(RegimeFlip),
    AgentDecision(AgentDecisionPayload),
    /// Emitted from Sprint 3 onward (cross-chain executor). Variant defined
    /// now so the frontend hook and event router stay stable across sprints.
    #[allow(dead_code)]
    RebalanceStatus(RebalanceStatus),
    /// Emitted from Sprint 2 onward (Gateway unified balance polling).
    #[allow(dead_code)]
    GatewayBalance(GatewayBalance),
}

impl SseEvent {
    /// Named SSE event type, used by `axum::response::sse::Event::event()`.
    /// Must match the `type` discriminator in the frontend `SseEvent` union.
    pub fn event_name(&self) -> &'static str {
        match self {
            Self::PriceTick(_) => "price.tick",
            Self::RegimeFlip(_) => "regime.flip",
            Self::AgentDecision(_) => "agent.decision",
            Self::RebalanceStatus(_) => "rebalance.status",
            Self::GatewayBalance(_) => "gateway.balance",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceTick {
    pub symbol: String,
    pub price_usd: f64,
    pub change_24h: f64,
    pub source: String,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegimeFlip {
    pub from: Option<String>,
    pub to: String,
    pub confidence: f32,
    pub signals: RegimeSignals,
    pub classified_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegimeSignals {
    pub btc_vol_30d: f64,
    pub corr_90d: f64,
    pub max_drawdown: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDecisionPayload {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub reasoning: String,
    pub recommendation: serde_json::Value,
    pub confidence: f64,
    pub triggered_by: String,
    pub created_at: DateTime<Utc>,
    pub model_slug: Option<String>,
    pub regime: Option<String>,
    pub prompt_tokens: Option<i32>,
    pub completion_tokens: Option<i32>,
    pub latency_ms: Option<i32>,
    pub critic_verdict: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RebalanceStatus {
    pub id: Uuid,
    pub step: String,
    pub chain: Option<String>,
    pub tx_hash: Option<String>,
    pub status: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayBalance {
    pub unified_usdc: f64,
    pub per_chain: std::collections::HashMap<String, f64>,
    pub observed_at: DateTime<Utc>,
}
