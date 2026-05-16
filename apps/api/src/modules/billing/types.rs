//! Billing v2 wire types (agent A2, schema-only).
//!
//! These structs are the Rust mirror of `packages/shared/src/types.ts` and
//! the rows in migration 0010. Handlers + middleware (agent A3) consume them;
//! nothing here makes HTTP or DB calls — it's pure data + tier capability
//! lookup. The dead_code allow is justified until A3 wires the consumers.
#![allow(dead_code)]

use std::fmt;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Three pricing tiers, mirroring the `plan_tiers.code` column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Pro,
    Business,
}

/// Capability envelope per tier. `None` = unlimited.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TierCaps {
    pub aum_cap_usd: Option<f64>,
    pub portfolios_cap: Option<u32>,
    pub decisions_cap_monthly: Option<u32>,
    pub per_rebalance_bps: u32,
    pub aum_annual_bps: u32,
}

impl Tier {
    /// Hard-coded capability lookup. Kept in sync with the `plan_tiers` seed
    /// in migration 0010; one source of truth at runtime is fine because price
    /// changes ship as new migrations + a code change.
    pub fn caps(self) -> TierCaps {
        match self {
            Tier::Free => TierCaps {
                aum_cap_usd: Some(5_000.0),
                portfolios_cap: Some(1),
                decisions_cap_monthly: Some(5),
                per_rebalance_bps: 25,
                aum_annual_bps: 0,
            },
            Tier::Pro => TierCaps {
                aum_cap_usd: None,
                portfolios_cap: Some(5),
                decisions_cap_monthly: Some(240),
                per_rebalance_bps: 15,
                aum_annual_bps: 25,
            },
            Tier::Business => TierCaps {
                aum_cap_usd: None,
                portfolios_cap: None,
                decisions_cap_monthly: None,
                per_rebalance_bps: 10,
                aum_annual_bps: 15,
            },
        }
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Tier::Free => "free",
            Tier::Pro => "pro",
            Tier::Business => "business",
        })
    }
}

/// Lossy parse — anything unrecognized falls back to `Free`. The schema's
/// FK to `plan_tiers(code)` prevents bad rows from being stored, so this
/// only kicks in if we ever read a value that wasn't validated at write.
impl From<&str> for Tier {
    fn from(s: &str) -> Self {
        match s {
            "pro" => Tier::Pro,
            "business" => Tier::Business,
            _ => Tier::Free,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SubscriptionStatus {
    Trialing,
    Active,
    PastDue,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub id: Uuid,
    pub user_id: Uuid,
    pub tier: Tier,
    pub status: SubscriptionStatus,
    pub started_at: DateTime<Utc>,
    pub current_period_start: DateTime<Utc>,
    pub current_period_end: DateTime<Utc>,
    pub cancel_at: Option<DateTime<Utc>>,
    pub billing_anchor_day: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InvoiceStatus {
    Open,
    Paid,
    Void,
    PastDue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineItem {
    pub description: String,
    pub quantity: f64,
    pub unit_amount_usdc: f64,
    pub amount_usdc: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Optional period covered by this line item (used by AUM accrual rollups
    /// that bill a sub-period inside the invoice's larger period).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period_start: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period_end: Option<DateTime<Utc>>,
    /// Optional pointer to the source row (e.g. an `aum_accruals.id`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Invoice {
    pub id: Uuid,
    pub user_id: Uuid,
    pub subscription_id: Option<Uuid>,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    #[sqlx(json)]
    pub line_items: Vec<LineItem>,
    pub subtotal_usdc: f64,
    pub total_usdc: f64,
    #[sqlx(try_from = "String")]
    pub status: InvoiceStatus,
    pub paid_at: Option<DateTime<Utc>>,
    pub paid_tx_hash: Option<String>,
    #[sqlx(default)]
    pub created_at: DateTime<Utc>,
}

impl TryFrom<String> for InvoiceStatus {
    type Error = String;
    fn try_from(s: String) -> std::result::Result<Self, String> {
        match s.as_str() {
            "open" => Ok(InvoiceStatus::Open),
            "paid" => Ok(InvoiceStatus::Paid),
            "void" => Ok(InvoiceStatus::Void),
            "past_due" => Ok(InvoiceStatus::PastDue),
            other => Err(format!("unknown invoice status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageMeter {
    pub user_id: Uuid,
    pub period_start: NaiveDate,
    pub decisions_count: u32,
    pub aum_usd_avg: f64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PerformanceBenchmark {
    Tbill3m,
    Susds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceFee {
    pub id: Uuid,
    pub user_id: Uuid,
    pub decision_id: Option<Uuid>,
    pub period: String,
    pub benchmark: PerformanceBenchmark,
    pub realized_gain_usd: f64,
    pub accrued_bps: u32,
    pub accrued_usdc: f64,
    pub settled_at: Option<DateTime<Utc>>,
    pub settlement_tx_hash: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Wire representation of a row in `plan_tiers` — used by the public
/// `/pricing` page and the upgrade modal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PricingTier {
    pub code: Tier,
    pub monthly_usd: f64,
    pub aum_cap_usd: Option<f64>,
    pub portfolios_cap: Option<u32>,
    pub decisions_cap_monthly: Option<u32>,
    pub per_rebalance_bps: u32,
    pub aum_annual_bps: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_caps_match_plan_section_2_1() {
        let free = Tier::Free.caps();
        assert_eq!(free.aum_cap_usd, Some(5_000.0));
        assert_eq!(free.decisions_cap_monthly, Some(5));
        assert_eq!(free.per_rebalance_bps, 25);
        assert_eq!(free.aum_annual_bps, 0);

        let pro = Tier::Pro.caps();
        assert_eq!(pro.aum_cap_usd, None);
        assert_eq!(pro.portfolios_cap, Some(5));
        assert_eq!(pro.per_rebalance_bps, 15);
        assert_eq!(pro.aum_annual_bps, 25);

        let biz = Tier::Business.caps();
        assert_eq!(biz.portfolios_cap, None);
        assert_eq!(biz.decisions_cap_monthly, None);
        assert_eq!(biz.per_rebalance_bps, 10);
        assert_eq!(biz.aum_annual_bps, 15);
    }

    #[test]
    fn tier_from_str_defaults_to_free() {
        assert_eq!(Tier::from("free"), Tier::Free);
        assert_eq!(Tier::from("pro"), Tier::Pro);
        assert_eq!(Tier::from("business"), Tier::Business);
        assert_eq!(Tier::from("garbage"), Tier::Free);
    }

    #[test]
    fn tier_display_lowercase() {
        assert_eq!(Tier::Pro.to_string(), "pro");
        assert_eq!(Tier::Business.to_string(), "business");
        assert_eq!(Tier::Free.to_string(), "free");
    }

    #[test]
    fn tier_serializes_lowercase() {
        let j = serde_json::to_string(&Tier::Business).unwrap();
        assert_eq!(j, "\"business\"");
    }
}
