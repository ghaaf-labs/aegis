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
    /// Emitted when Circle Wallets create succeeds — lets the UI swap to
    /// authed state without polling. Sprint 2+.
    WalletCreated(crate::modules::wallet::sse::WalletCreatedPayload),
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
            Self::WalletCreated(_) => "wallet.created",
        }
    }

    /// User the event is addressed to, or `None` if the event is public
    /// (e.g. market price ticks, regime flips visible to everyone).
    ///
    /// The SSE handler uses this to filter the per-user stream — a subscriber
    /// authenticated as user X never receives an event addressed to user Y.
    pub fn audience_user_id(&self) -> Option<Uuid> {
        match self {
            Self::PriceTick(_) | Self::RegimeFlip(_) | Self::RebalanceStatus(_) => None,
            Self::AgentDecision(p) => Some(p.user_id),
            Self::GatewayBalance(p) => Some(p.user_id),
            Self::WalletCreated(p) => Some(p.user_id),
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
    /// Audience filter — `/sse` only forwards this event to the matching user.
    pub user_id: Uuid,
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
    /// Audience filter.
    pub user_id: Uuid,
    pub unified_usdc: f64,
    pub per_chain: std::collections::HashMap<String, f64>,
    pub observed_at: DateTime<Utc>,
}

#[cfg(test)]
mod contract_tests {
    //! Lock the on-the-wire JSON shape against the TypeScript types in
    //! `packages/shared/src/types.ts`. Any rename here without a matching
    //! TS change is a contract break.

    use super::*;
    use chrono::TimeZone;
    use serde_json::Value;

    fn ts() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 14, 12, 0, 0).unwrap()
    }

    fn json(value: &impl Serialize) -> Value {
        serde_json::to_value(value).expect("serialize")
    }

    #[test]
    fn price_tick_keys_are_camel_case() {
        let v = json(&PriceTick {
            symbol: "BTC".into(),
            price_usd: 1.0,
            change_24h: 2.0,
            source: "coingecko".into(),
            fetched_at: ts(),
        });
        for key in ["symbol", "priceUsd", "change24h", "source", "fetchedAt"] {
            assert!(v.get(key).is_some(), "missing key {key} in {v}");
        }
        assert!(v.get("price_usd").is_none(), "snake_case leaked");
    }

    #[test]
    fn regime_flip_keys_match_frontend() {
        let v = json(&RegimeFlip {
            from: None,
            to: "risk_off".into(),
            confidence: 0.8,
            signals: RegimeSignals {
                btc_vol_30d: 0.5,
                corr_90d: 0.6,
                max_drawdown: 0.1,
            },
            classified_at: ts(),
        });
        for key in ["from", "to", "confidence", "signals", "classifiedAt"] {
            assert!(v.get(key).is_some(), "regime.flip missing {key}");
        }
        let signals = v.get("signals").and_then(Value::as_object).unwrap();
        for key in ["btcVol30d", "corr90d", "maxDrawdown"] {
            assert!(
                signals.contains_key(key),
                "regime.flip.signals missing {key}"
            );
        }
    }

    #[test]
    fn agent_decision_keys_are_camel_case() {
        let payload = AgentDecisionPayload {
            id: uuid::Uuid::nil(),
            portfolio_id: uuid::Uuid::nil(),
            user_id: uuid::Uuid::nil(),
            reasoning: "r".into(),
            recommendation: serde_json::json!({
                "summary": "x",
                "trades": [],
                "expectedImpact": { "riskDelta": 0.0, "diversificationScore": 0.0 }
            }),
            confidence: 0.7,
            triggered_by: "user_request".into(),
            created_at: ts(),
            model_slug: Some("anthropic/claude-opus-4-7".into()),
            regime: Some("neutral".into()),
            prompt_tokens: Some(123),
            completion_tokens: Some(456),
            latency_ms: Some(789),
            critic_verdict: Some(serde_json::json!({
                "demandsRevision": false,
                "notes": "OK",
                "confidence": 0.9
            })),
        };
        let v = json(&payload);
        for key in [
            "id",
            "portfolioId",
            "userId",
            "reasoning",
            "recommendation",
            "confidence",
            "triggeredBy",
            "createdAt",
            "modelSlug",
            "regime",
            "promptTokens",
            "completionTokens",
            "latencyMs",
            "criticVerdict",
        ] {
            assert!(v.get(key).is_some(), "AgentDecision missing key {key}");
        }
        assert!(
            v.get("portfolio_id").is_none(),
            "AgentDecision leaked snake_case key"
        );
    }

    #[test]
    fn untagged_envelope_serializes_to_inner_payload() {
        // The Rust `SseEvent` is `#[serde(untagged)]` — the JSON body is the
        // inner payload only. The `event:` SSE field carries discrimination.
        let inner = PriceTick {
            symbol: "ETH".into(),
            price_usd: 3500.0,
            change_24h: 1.5,
            source: "coingecko".into(),
            fetched_at: ts(),
        };
        let envelope = SseEvent::PriceTick(inner.clone());
        assert_eq!(json(&envelope), json(&inner));
    }

    #[test]
    fn audience_user_id_filters_user_events() {
        let me = uuid::Uuid::new_v4();
        let other = uuid::Uuid::new_v4();

        let agent_for_me = SseEvent::AgentDecision(AgentDecisionPayload {
            id: uuid::Uuid::nil(),
            portfolio_id: uuid::Uuid::nil(),
            user_id: me,
            reasoning: "r".into(),
            recommendation: serde_json::json!({}),
            confidence: 0.0,
            triggered_by: "x".into(),
            created_at: ts(),
            model_slug: None,
            regime: None,
            prompt_tokens: None,
            completion_tokens: None,
            latency_ms: None,
            critic_verdict: None,
        });
        let agent_for_other = SseEvent::AgentDecision(AgentDecisionPayload {
            id: uuid::Uuid::nil(),
            portfolio_id: uuid::Uuid::nil(),
            user_id: other,
            reasoning: "r".into(),
            recommendation: serde_json::json!({}),
            confidence: 0.0,
            triggered_by: "x".into(),
            created_at: ts(),
            model_slug: None,
            regime: None,
            prompt_tokens: None,
            completion_tokens: None,
            latency_ms: None,
            critic_verdict: None,
        });
        let public_event = SseEvent::PriceTick(PriceTick {
            symbol: "BTC".into(),
            price_usd: 0.0,
            change_24h: 0.0,
            source: "x".into(),
            fetched_at: ts(),
        });

        assert_eq!(agent_for_me.audience_user_id(), Some(me));
        assert_eq!(agent_for_other.audience_user_id(), Some(other));
        assert_eq!(public_event.audience_user_id(), None);
    }

    #[test]
    fn event_names_match_typescript_discriminators() {
        // Event names are exactly what the frontend hook subscribes to.
        let cases: &[(&str, SseEvent)] = &[
            (
                "price.tick",
                SseEvent::PriceTick(PriceTick {
                    symbol: "BTC".into(),
                    price_usd: 0.0,
                    change_24h: 0.0,
                    source: "x".into(),
                    fetched_at: ts(),
                }),
            ),
            (
                "regime.flip",
                SseEvent::RegimeFlip(RegimeFlip {
                    from: None,
                    to: "neutral".into(),
                    confidence: 0.0,
                    signals: RegimeSignals {
                        btc_vol_30d: 0.0,
                        corr_90d: 0.0,
                        max_drawdown: 0.0,
                    },
                    classified_at: ts(),
                }),
            ),
        ];
        for (expected, ev) in cases {
            assert_eq!(ev.event_name(), *expected);
        }
    }
}
