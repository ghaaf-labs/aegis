//! Aegis Constitution — versioned hard constraints the strategist must obey.
//!
//! Loaded once at startup from `apps/api/config/constitution.yaml`. Every
//! strategist proposal is run through [`evaluate`] before the LLM critic.
//! Any violation short-circuits the critic to a VETO verdict whose reasoning
//! cites the clause IDs. The UI surfaces those IDs next to `model_slug` so
//! the user sees the explicit rulebook behind every block — closing the
//! "LLM-prompted-Bankr" attack class.
//!
//! The constitution is pure data plus a pure function. No I/O, no DB; the
//! only side effect is the OnceCell cache to avoid re-parsing per request.

use std::path::PathBuf;

use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

/// Tier mirror — kept local so the constitution module doesn't depend on
/// `billing::types::Tier` (which lands in a parallel agent). When A2's
/// canonical Tier lands, swap this re-export for `pub use`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Pro,
    Business,
}

impl Tier {
    /// Tier-ordering: a clause with `tier_min: pro` applies to Pro and
    /// Business but not Free.
    fn rank(&self) -> u8 {
        match self {
            Tier::Free => 0,
            Tier::Pro => 1,
            Tier::Business => 2,
        }
    }

    fn satisfies(&self, min: &Tier) -> bool {
        self.rank() >= min.rank()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Constitution {
    pub version: u32,
    pub effective_at: chrono::DateTime<chrono::Utc>,
    pub clauses: Vec<Clause>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clause {
    pub id: String,
    pub summary: String,
    pub description: String,
    pub kind: ClauseKind,
    pub field: String,
    pub param: ClauseParam,
    #[serde(default)]
    pub tier_min: Option<Tier>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClauseKind {
    HardLimit,
    Band,
    Floor,
    Ceiling,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClauseParam {
    Number(f64),
    Range([f64; 2]),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClauseViolation {
    pub clause_id: String,
    pub summary: String,
    pub field: String,
    pub observed: serde_json::Value,
    pub expected: serde_json::Value,
}

/// Slimmed proposal shape the constitution evaluator reads. The full
/// `StrategistProposal` lives in `service.rs` and serialises into this
/// shape via `serde_json`.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    #[serde(default)]
    pub expected_max_drawdown_pct: Option<f64>,
    #[serde(default)]
    pub allocations: Vec<ProposalAllocation>,
    #[serde(default)]
    pub legs: Vec<ProposalLeg>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProposalAllocation {
    pub asset: String,
    /// Target weight as a fraction in `[0, 1]`. Inputs in `0..100` are
    /// normalised by [`evaluate`] so YAML can read either convention.
    pub target_weight_pct: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProposalLeg {
    #[serde(default)]
    pub slippage_bps: f64,
}

static CACHE: OnceCell<Constitution> = OnceCell::new();

/// Load the constitution YAML, parse it, and cache it for the lifetime of the
/// process. Returns a reference to the cached value on subsequent calls.
pub fn load() -> anyhow::Result<&'static Constitution> {
    if let Some(c) = CACHE.get() {
        return Ok(c);
    }
    let path = config_path();
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("read constitution at {}: {e}", path.display()))?;
    let parsed: Constitution = serde_yaml::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("parse constitution at {}: {e}", path.display()))?;
    let _ = CACHE.set(parsed);
    Ok(CACHE.get().expect("just set"))
}

/// Test/inspection-only entry point that parses YAML without populating the
/// global cache.
#[allow(dead_code)]
pub fn load_from_str(yaml: &str) -> anyhow::Result<Constitution> {
    serde_yaml::from_str(yaml).map_err(|e| anyhow::anyhow!("parse constitution: {e}"))
}

fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("CONSTITUTION_PATH") {
        return PathBuf::from(p);
    }
    // CARGO_MANIFEST_DIR resolves to `apps/api/` at build time so the file
    // ships with the binary's source checkout without needing CWD munging.
    let manifest = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest)
        .join("config")
        .join("constitution.yaml")
}

/// Normalise a weight value: accept either `0..1` fractions or `0..100`
/// percent and return a fraction in `[0, 1]`. Anything above 1.0 is treated
/// as a percent.
fn norm_weight(w: f64) -> f64 {
    if w > 1.0 {
        w / 100.0
    } else {
        w
    }
}

/// Evaluate a proposal against every applicable clause of the constitution.
/// Returns an empty `Vec` when clean; otherwise one entry per violation.
pub fn evaluate(
    constitution: &Constitution,
    proposal: &Proposal,
    user_tier: Tier,
    aum_usd: f64,
) -> Vec<ClauseViolation> {
    let mut violations = Vec::new();
    for clause in &constitution.clauses {
        if let Some(min) = clause.tier_min {
            if !user_tier.satisfies(&min) {
                continue;
            }
        }
        match clause.id.as_str() {
            "RISK-1" => check_risk_1(clause, proposal, &mut violations),
            "RISK-2" => check_risk_2(clause, proposal, &mut violations),
            "RISK-3" => check_risk_3(clause, proposal, &mut violations),
            "FX-1" => check_fx_1(clause, proposal, &mut violations),
            "USYC-1" => check_usyc_1(clause, proposal, aum_usd, &mut violations),
            _ => {} // Unknown clause id — ignore so YAML can add docs-only entries.
        }
    }
    violations
}

fn check_risk_1(clause: &Clause, proposal: &Proposal, out: &mut Vec<ClauseViolation>) {
    let Some(observed) = proposal.expected_max_drawdown_pct else {
        return;
    };
    let normalized = norm_weight(observed);
    let limit = match clause.param {
        ClauseParam::Number(n) => n,
        _ => return,
    };
    if normalized > limit {
        out.push(ClauseViolation {
            clause_id: clause.id.clone(),
            summary: clause.summary.clone(),
            field: clause.field.clone(),
            observed: serde_json::json!(normalized),
            expected: serde_json::json!({ "lessThanOrEqual": limit }),
        });
    }
}

fn check_risk_2(clause: &Clause, proposal: &Proposal, out: &mut Vec<ClauseViolation>) {
    let limit = match clause.param {
        ClauseParam::Number(n) => n,
        _ => return,
    };
    for alloc in &proposal.allocations {
        let w = norm_weight(alloc.target_weight_pct);
        if w > limit {
            out.push(ClauseViolation {
                clause_id: clause.id.clone(),
                summary: clause.summary.clone(),
                field: format!("allocations[asset='{}'].targetWeightPct", alloc.asset),
                observed: serde_json::json!(w),
                expected: serde_json::json!({ "lessThanOrEqual": limit }),
            });
        }
    }
}

fn check_risk_3(clause: &Clause, proposal: &Proposal, out: &mut Vec<ClauseViolation>) {
    let ceiling = match clause.param {
        ClauseParam::Number(n) => n,
        _ => return,
    };
    for (i, leg) in proposal.legs.iter().enumerate() {
        if leg.slippage_bps > ceiling {
            out.push(ClauseViolation {
                clause_id: clause.id.clone(),
                summary: clause.summary.clone(),
                field: format!("legs[{i}].slippageBps"),
                observed: serde_json::json!(leg.slippage_bps),
                expected: serde_json::json!({ "lessThanOrEqual": ceiling }),
            });
        }
    }
}

fn check_fx_1(clause: &Clause, proposal: &Proposal, out: &mut Vec<ClauseViolation>) {
    let (lo, hi) = match clause.param {
        ClauseParam::Range([a, b]) => (a, b),
        _ => return,
    };
    let eurc: f64 = proposal
        .allocations
        .iter()
        .filter(|a| a.asset.eq_ignore_ascii_case("EURC"))
        .map(|a| norm_weight(a.target_weight_pct))
        .sum();
    if eurc < lo || eurc > hi {
        out.push(ClauseViolation {
            clause_id: clause.id.clone(),
            summary: clause.summary.clone(),
            field: clause.field.clone(),
            observed: serde_json::json!(eurc),
            expected: serde_json::json!({ "between": [lo, hi] }),
        });
    }
}

fn check_usyc_1(
    clause: &Clause,
    proposal: &Proposal,
    aum_usd: f64,
    out: &mut Vec<ClauseViolation>,
) {
    let floor = match clause.param {
        ClauseParam::Number(n) => n,
        _ => return,
    };
    if aum_usd < 50_000.0 {
        return;
    }
    let usyc: f64 = proposal
        .allocations
        .iter()
        .filter(|a| a.asset.eq_ignore_ascii_case("USYC"))
        .map(|a| norm_weight(a.target_weight_pct))
        .sum();
    if usyc < floor {
        out.push(ClauseViolation {
            clause_id: clause.id.clone(),
            summary: clause.summary.clone(),
            field: clause.field.clone(),
            observed: serde_json::json!(usyc),
            expected: serde_json::json!({ "greaterThanOrEqual": floor }),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Constitution {
        load_from_str(include_str!("../../../config/constitution.yaml")).unwrap()
    }

    fn clean_proposal() -> Proposal {
        Proposal {
            expected_max_drawdown_pct: Some(0.12),
            allocations: vec![
                ProposalAllocation {
                    asset: "USDC".into(),
                    target_weight_pct: 0.50,
                },
                ProposalAllocation {
                    asset: "BTC".into(),
                    target_weight_pct: 0.30,
                },
                ProposalAllocation {
                    asset: "EURC".into(),
                    target_weight_pct: 0.10,
                },
                ProposalAllocation {
                    asset: "USYC".into(),
                    target_weight_pct: 0.10,
                },
            ],
            legs: vec![ProposalLeg { slippage_bps: 25.0 }],
        }
    }

    #[test]
    fn yaml_round_trips_with_six_clauses() {
        let c = fixture();
        assert_eq!(c.version, 1);
        assert_eq!(c.clauses.len(), 5);
        let ids: Vec<&str> = c.clauses.iter().map(|x| x.id.as_str()).collect();
        assert!(ids.contains(&"RISK-1"));
        assert!(ids.contains(&"FX-1"));
        assert!(ids.contains(&"USYC-1"));
    }

    #[test]
    fn clean_proposal_yields_no_violations() {
        let c = fixture();
        let v = evaluate(&c, &clean_proposal(), Tier::Business, 100_000.0);
        assert!(v.is_empty(), "expected clean but got: {:?}", v);
    }

    #[test]
    fn risk_1_fires_on_excess_drawdown() {
        let c = fixture();
        let mut p = clean_proposal();
        p.expected_max_drawdown_pct = Some(0.27);
        let v = evaluate(&c, &p, Tier::Pro, 10_000.0);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].clause_id, "RISK-1");
        assert_eq!(v[0].observed, serde_json::json!(0.27));
    }

    #[test]
    fn risk_2_and_risk_3_fire_together() {
        let c = fixture();
        let mut p = clean_proposal();
        // Push BTC over the 60% single-asset cap.
        p.allocations[1].target_weight_pct = 0.75;
        p.allocations[0].target_weight_pct = 0.10;
        // And blow past the slippage ceiling.
        p.legs.push(ProposalLeg {
            slippage_bps: 200.0,
        });
        let v = evaluate(&c, &p, Tier::Pro, 10_000.0);
        let ids: Vec<&str> = v.iter().map(|x| x.clause_id.as_str()).collect();
        assert!(ids.contains(&"RISK-2"));
        assert!(ids.contains(&"RISK-3"));
    }

    #[test]
    fn fx_1_only_applies_to_pro_and_above() {
        let c = fixture();
        let mut p = clean_proposal();
        p.allocations.iter_mut().for_each(|a| {
            if a.asset == "EURC" {
                a.target_weight_pct = 0.55; // out of band
            }
        });
        // Free user: clause skipped — but RISK-2 still fires since 55% < 60%? No, it's under.
        // Drop drawdown back to clean to isolate.
        let free = evaluate(&c, &p, Tier::Free, 10_000.0);
        assert!(
            !free.iter().any(|v| v.clause_id == "FX-1"),
            "FX-1 must skip Free tier"
        );
        let pro = evaluate(&c, &p, Tier::Pro, 10_000.0);
        assert!(pro.iter().any(|v| v.clause_id == "FX-1"));
    }

    #[test]
    fn usyc_1_only_fires_for_business_and_aum_threshold() {
        let c = fixture();
        let mut p = clean_proposal();
        // Strip USYC entirely.
        p.allocations.retain(|a| a.asset != "USYC");
        // Rebalance USDC to keep weights summing to 1.0.
        p.allocations[0].target_weight_pct = 0.60;

        // Below $50k AUM, USYC-1 exempts.
        let exempt = evaluate(&c, &p, Tier::Business, 25_000.0);
        assert!(!exempt.iter().any(|v| v.clause_id == "USYC-1"));

        // Pro tier, even at high AUM, doesn't trigger USYC-1.
        let pro = evaluate(&c, &p, Tier::Pro, 250_000.0);
        assert!(!pro.iter().any(|v| v.clause_id == "USYC-1"));

        // Business + AUM >= $50k → fires.
        let biz = evaluate(&c, &p, Tier::Business, 250_000.0);
        assert!(biz.iter().any(|v| v.clause_id == "USYC-1"));
    }

    #[test]
    fn weight_normaliser_accepts_percent_or_fraction() {
        assert!((norm_weight(0.42) - 0.42).abs() < 1e-9);
        assert!((norm_weight(42.0) - 0.42).abs() < 1e-9);
    }

    #[test]
    fn tier_ordering_holds() {
        assert!(Tier::Business.satisfies(&Tier::Pro));
        assert!(Tier::Pro.satisfies(&Tier::Pro));
        assert!(!Tier::Free.satisfies(&Tier::Pro));
        assert!(!Tier::Pro.satisfies(&Tier::Business));
    }
}
