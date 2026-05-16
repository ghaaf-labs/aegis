//! Shared billing types — `Tier`, `TierCaps`, and the wire structs used
//! by both the AUM-stream accrual loop (A4) and the tier-gate middleware
//! (A3). Owned by A2 in the plan; A4 scaffolds the minimum surface here
//! so its work compiles in isolation. When A2 lands first, this file's
//! shape will be a superset — A3 may add fields without breaking A4.

// Most of these symbols are consumed by A3 (tier middleware) when that
// agent's branch lands; the dead-code allow keeps clippy quiet on the
// A4-only intermediate state.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Pro,
    Business,
}

impl Tier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Tier::Free => "free",
            Tier::Pro => "pro",
            Tier::Business => "business",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "free" => Some(Tier::Free),
            "pro" => Some(Tier::Pro),
            "business" => Some(Tier::Business),
            _ => None,
        }
    }

    /// Caps decided in §2.1 of the roadmap plan. A3's middleware reads
    /// these directly; A4 only needs `aum_annual_bps`.
    pub fn caps(&self) -> TierCaps {
        match self {
            Tier::Free => TierCaps {
                monthly_usd: 0,
                aum_cap_usd: Some(5_000),
                portfolios_cap: Some(1),
                decisions_per_mo: 5,
                aum_annual_bps: 0,
                rebalance_bps: 25,
            },
            Tier::Pro => TierCaps {
                monthly_usd: 19,
                aum_cap_usd: None,
                portfolios_cap: Some(5),
                decisions_per_mo: 240,
                aum_annual_bps: 25,
                rebalance_bps: 15,
            },
            Tier::Business => TierCaps {
                monthly_usd: 199,
                aum_cap_usd: None,
                portfolios_cap: None,
                decisions_per_mo: 100_000,
                aum_annual_bps: 15,
                rebalance_bps: 10,
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TierCaps {
    pub monthly_usd: u32,
    pub aum_cap_usd: Option<u64>,
    pub portfolios_cap: Option<u32>,
    pub decisions_per_mo: u32,
    pub aum_annual_bps: u32,
    pub rebalance_bps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub id: Uuid,
    pub user_id: Uuid,
    pub tier: String,
    pub status: String,
    pub anchor_day: i32,
    pub started_at: DateTime<Utc>,
    pub canceled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Invoice {
    pub id: Uuid,
    pub user_id: Uuid,
    pub subscription_id: Uuid,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub status: String,
    pub line_items: serde_json::Value,
    pub subtotal_usdc: Decimal,
    pub total_usdc: Decimal,
    pub paid_at: Option<DateTime<Utc>>,
    pub paid_tx_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineItem {
    pub kind: String,
    pub description: String,
    pub amount_usdc: Decimal,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub ref_id: Option<Uuid>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_round_trips() {
        for t in [Tier::Free, Tier::Pro, Tier::Business] {
            assert_eq!(Tier::from_str(t.as_str()), Some(t));
        }
    }

    #[test]
    fn pro_caps_are_25bps_aum() {
        assert_eq!(Tier::Pro.caps().aum_annual_bps, 25);
        assert_eq!(Tier::Business.caps().aum_annual_bps, 15);
        assert_eq!(Tier::Free.caps().aum_annual_bps, 0);
    }
}
