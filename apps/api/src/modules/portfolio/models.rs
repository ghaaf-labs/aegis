use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Portfolio {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub total_value_usd: f64,
    pub total_pnl_usd: f64,
    pub total_pnl_pct: f64,
    pub risk_score: i32,
    /// Goal-wizard output (Sprint 2). JSONB column; opaque to Rust until the
    /// strategist renders it into the prompt.
    #[serde(default)]
    pub goal: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct CreatePortfolioRequest {
    pub name: String,
    pub allocations: Vec<AllocationInput>,
    /// Goal-wizard output, written verbatim to `portfolios.goal` JSONB.
    /// Optional so Sprint 1 callers still work; Sprint 2's wizard always sets it.
    #[serde(default)]
    pub goal: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllocationInput {
    pub symbol: String,
    pub quantity: f64,
    pub target_weight: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePortfolioRequest {
    pub name: Option<String>,
    /// Full goal replacement. Used by account surfaces that attach agent
    /// preferences such as allowed wallet routes.
    pub goal: Option<serde_json::Value>,
    #[allow(dead_code)]
    pub allocations: Option<Vec<AllocationInput>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pre-submission audit (2026-05-15): the frontend uniformly sends and
    // expects camelCase JSON; the API silently rejected camelCase request
    // bodies because the structs deserialized snake_case. Lock the contract.
    #[test]
    fn allocation_input_deserializes_camel_case() {
        let json = r#"{"symbol":"BTC","quantity":1.0,"targetWeight":0.5}"#;
        let parsed: AllocationInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.symbol, "BTC");
        assert_eq!(parsed.target_weight, 0.5);
    }

    #[test]
    fn create_portfolio_request_deserializes_camel_case_allocations() {
        let json = r#"{
            "name": "Treasury",
            "allocations": [
                {"symbol":"BTC","quantity":1.0,"targetWeight":0.5},
                {"symbol":"USYC","quantity":0.0,"targetWeight":0.5}
            ],
            "goal": {"horizon":"5y"}
        }"#;
        let parsed: CreatePortfolioRequest = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.allocations.len(), 2);
        assert_eq!(parsed.allocations[1].target_weight, 0.5);
        assert!(parsed.goal.is_some());
    }

    #[test]
    fn update_portfolio_request_accepts_goal_patch() {
        let json = r#"{
            "goal": {
                "name": "Treasury",
                "routePreferences": {
                    "networks": ["ARC-TESTNET", "BASE-SEPOLIA"],
                    "tokens": ["USDC"],
                    "watchlist": ["USYC", "EURC"]
                }
            }
        }"#;
        let parsed: UpdatePortfolioRequest = serde_json::from_str(json).unwrap();
        let goal = parsed.goal.expect("goal patch");
        assert_eq!(
            goal.pointer("/routePreferences/tokens/0")
                .and_then(|v| v.as_str()),
            Some("USDC")
        );
    }

    #[test]
    fn portfolio_serializes_camel_case_keys() {
        let p = Portfolio {
            id: Uuid::nil(),
            user_id: Uuid::nil(),
            name: "x".into(),
            total_value_usd: 1.0,
            total_pnl_usd: 2.0,
            total_pnl_pct: 0.5,
            risk_score: 50,
            goal: serde_json::json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_value(&p).unwrap();
        // The frontend reads these exact keys (packages/shared/src/types.ts).
        assert!(json.get("totalValueUsd").is_some(), "got: {json}");
        assert!(json.get("totalPnlUsd").is_some());
        assert!(json.get("totalPnlPct").is_some());
        assert!(json.get("riskScore").is_some());
        assert!(json.get("createdAt").is_some());
        assert!(json.get("user_id").is_none(), "snake_case key leaked");
    }
}
