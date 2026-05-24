//! Deterministic allocation clamping — the safety net between a raw LLM
//! allocation map and a valid, executable target.
//!
//! [`propose_allocation`](super::service::propose_allocation) and
//! [`apply_allocation`](super::service::apply_allocation) both run the model's
//! (or stored) weights through [`clamp_allocation`] under regime/vol-aware
//! [`Guardrails`] so a bad or refused model output can never produce an invalid
//! or over-concentrated target.

/// Structural + risk guardrails the clamp enforces. Phase 1 extends this with
/// regime/vol/correlation tilts; Phase 0 keeps the invariant set: a per-asset
/// cap on volatile sleeves (≤ the constitution's RISK-2 60%) plus a stable +
/// yield reserve floor derived from the user's risk tolerance.
#[derive(Debug, Clone, Copy)]
pub(super) struct Guardrails {
    /// Max weight (%) for any single non-stable asset (≤ RISK-2 60%).
    pub(super) single_asset_cap: f64,
    /// Minimum combined weight (%) for the stable/yield reserve sleeve.
    pub(super) stable_floor: f64,
    /// Max combined weight (%) across ALL non-stable assets. This is the
    /// correlation-diversification guardrail: cbBTC/WETH/cbETH move together,
    /// so the whole crypto sleeve is bounded, not just each leg.
    pub(super) volatile_cluster_cap: f64,
}

/// Regime/vol-aware guardrails. Base caps/floors come from risk tolerance;
/// `risk_off` raises the stable+yield floor and trims volatile caps, `risk_on`
/// loosens them, and high BTC realized vol scales the volatile sleeve down.
pub(super) fn derive_guardrails(
    risk_tolerance: &str,
    regime: &str,
    btc_vol_30d: f64,
) -> Guardrails {
    let (mut single_asset_cap, mut stable_floor, mut volatile_cluster_cap): (f64, f64, f64) =
        match risk_tolerance.to_lowercase().as_str() {
            "aggressive" => (60.0, 5.0, 90.0),
            "moderate" => (45.0, 20.0, 70.0),
            // conservative (also the safe default for unknown values)
            _ => (25.0, 50.0, 45.0),
        };
    match regime.to_lowercase().as_str() {
        "risk_off" => {
            stable_floor = (stable_floor + 20.0).min(80.0);
            volatile_cluster_cap = (volatile_cluster_cap - 20.0).max(10.0);
            single_asset_cap = single_asset_cap.min(40.0);
        }
        "risk_on" => {
            stable_floor = (stable_floor - 5.0).max(5.0);
            volatile_cluster_cap = (volatile_cluster_cap + 10.0).min(95.0);
        }
        _ => {}
    }
    // High realized vol → trim the volatile sleeve further.
    if btc_vol_30d > 0.8 {
        volatile_cluster_cap = (volatile_cluster_cap - 15.0).max(10.0);
        single_asset_cap = (single_asset_cap - 10.0).max(10.0);
    }
    // RISK-2 hard ceiling regardless of inputs.
    single_asset_cap = single_asset_cap.min(60.0);
    Guardrails {
        single_asset_cap,
        stable_floor,
        volatile_cluster_cap,
    }
}

/// Symbols treated as the stable / cash / yield reserve sleeve for risk
/// purposes: exempt from the single-asset volatility cap and counted toward the
/// reserve floor. Shared with the constitution evaluator (RISK-2) as the single
/// source of truth, so a USDC-heavy reserve is never flagged as
/// over-concentrated.
pub(crate) const STABLE_SYMBOLS: &[&str] = &[
    crate::domain::token::USDC,
    crate::domain::token::SUSDS,
    crate::domain::token::USYC,
    crate::domain::token::EURC,
];

pub(crate) fn is_stable_symbol(sym: &str) -> bool {
    STABLE_SYMBOLS.iter().any(|s| s.eq_ignore_ascii_case(sym))
}

pub(super) fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// Deterministic safety net: turn a raw LLM allocation map into a valid target.
/// Drops non-executable tokens (USDC always allowed) and non-positive weights,
/// caps any single non-stable asset at the guardrail cap, enforces the
/// stable/yield reserve floor, normalizes to sum 100, and sweeps the rounding
/// residual into USDC. Always returns a non-empty map summing to ~100 (USDC-only
/// in the worst case) — so a bad/refused LLM output can never produce an invalid
/// or over-concentrated target.
pub(super) fn clamp_allocation(
    raw: &serde_json::Map<String, serde_json::Value>,
    executable: &[&str],
    guardrails: Guardrails,
) -> std::collections::BTreeMap<String, f64> {
    use std::collections::BTreeMap;
    let usdc_only =
        || -> BTreeMap<String, f64> { std::iter::once(("USDC".to_string(), 100.0)).collect() };
    let exec_ok = |sym: &str| {
        sym.eq_ignore_ascii_case("USDC") || executable.iter().any(|e| e.eq_ignore_ascii_case(sym))
    };

    // 1. Keep executable, positive weights (canonicalize USDC casing).
    let mut weights: BTreeMap<String, f64> = BTreeMap::new();
    for (sym, v) in raw {
        let w = v.as_f64().unwrap_or(0.0);
        if w <= 0.0 || !exec_ok(sym) {
            continue;
        }
        let key = if sym.eq_ignore_ascii_case("USDC") {
            "USDC".to_string()
        } else {
            sym.clone()
        };
        *weights.entry(key).or_insert(0.0) += w;
    }
    let total: f64 = weights.values().sum();
    if total <= 0.0 {
        return usdc_only();
    }

    // 2. Normalize to sum 100.
    for w in weights.values_mut() {
        *w = (*w / total) * 100.0;
    }

    // 3. Cap each non-stable asset; route the excess into USDC (a stable, so
    //    this can never re-violate a cap).
    let mut excess = 0.0;
    for (k, w) in weights.iter_mut() {
        if !is_stable_symbol(k) && *w > guardrails.single_asset_cap {
            excess += *w - guardrails.single_asset_cap;
            *w = guardrails.single_asset_cap;
        }
    }
    if excess > 0.0 {
        *weights.entry("USDC".to_string()).or_insert(0.0) += excess;
    }

    // 3b. Cap the combined non-stable (correlated) cluster; route the excess
    //     into USDC. Scaling down can't re-violate the single-asset cap.
    let cluster_sum: f64 = weights
        .iter()
        .filter(|(k, _)| !is_stable_symbol(k))
        .map(|(_, w)| *w)
        .sum();
    if cluster_sum > guardrails.volatile_cluster_cap && cluster_sum > 0.0 {
        let scale = guardrails.volatile_cluster_cap / cluster_sum;
        let mut moved = 0.0;
        for (k, w) in weights.iter_mut() {
            if !is_stable_symbol(k) {
                let nw = *w * scale;
                moved += *w - nw;
                *w = nw;
            }
        }
        *weights.entry("USDC".to_string()).or_insert(0.0) += moved;
    }

    // 4. Enforce the stable/yield reserve floor by scaling non-stables DOWN
    //    proportionally (never up — so caps still hold) and topping up USDC.
    let stable_sum: f64 = weights
        .iter()
        .filter(|(k, _)| is_stable_symbol(k))
        .map(|(_, w)| *w)
        .sum();
    if stable_sum < guardrails.stable_floor {
        let nonstable_sum: f64 = weights
            .iter()
            .filter(|(k, _)| !is_stable_symbol(k))
            .map(|(_, w)| *w)
            .sum();
        let deficit = (guardrails.stable_floor - stable_sum).min(nonstable_sum);
        if nonstable_sum > 0.0 && deficit > 0.0 {
            let scale = (nonstable_sum - deficit) / nonstable_sum;
            for (k, w) in weights.iter_mut() {
                if !is_stable_symbol(k) {
                    *w *= scale;
                }
            }
            *weights.entry("USDC".to_string()).or_insert(0.0) += deficit;
        }
    }

    // 5. Round to 0.01 and sweep the residual into USDC so the map sums to 100.
    for w in weights.values_mut() {
        *w = round2(*w);
    }
    let sum: f64 = weights.values().sum();
    let residual = round2(100.0 - sum);
    if residual.abs() >= 0.01 {
        let usdc = weights.entry("USDC".to_string()).or_insert(0.0);
        *usdc = round2((*usdc + residual).max(0.0));
    }
    weights.retain(|_, w| *w > 0.0);
    if weights.is_empty() {
        return usdc_only();
    }
    weights
}

/// The raw, pre-clamp allocator output handed to [`finalize_allocation`].
/// Borrowed so the caller keeps ownership of the parsed proposal.
pub(super) struct RawAllocation<'a> {
    /// Model-proposed weights, `{ symbol: pct }`.
    pub(super) weights: &'a serde_json::Map<String, serde_json::Value>,
    /// Model reasoning, verbatim.
    pub(super) reasoning: &'a str,
    /// Model confidence, 0..1.
    pub(super) confidence: f64,
    /// Model's projected max drawdown (percent), if it gave one.
    pub(super) expected_max_drawdown_pct: Option<f64>,
}

/// The reconciled, persist-ready allocation. Its payload fields are guaranteed
/// mutually CONSISTENT: `reasoning` never describes a mix we didn't keep (a
/// risk-adjustment note is appended whenever clamping changed the weights), and
/// `expected_max_drawdown_pct` reflects the FINAL allocation, not the model's
/// pre-clamp guess. This invariant is what stops the proposal modal from showing
/// "USDC 100%" next to reasoning about cbBTC/cbETH.
#[derive(Debug, Clone)]
pub(super) struct FinalizedAllocation {
    pub(super) allocation: std::collections::BTreeMap<String, f64>,
    pub(super) reasoning: String,
    pub(super) expected_max_drawdown_pct: Option<f64>,
    /// Human-readable risk adjustments applied to the raw weights; empty when the
    /// model's mix survived clamping unchanged.
    pub(super) adjustments: Vec<String>,
}

/// Confidence below which the allocator declines to trust the raw weights and
/// falls back to the stored target (then a USDC reserve). Mirrors the agent's
/// abstain threshold; kept local so the clamp stays a self-contained pure unit.
const LOW_CONFIDENCE_FALLBACK: f64 = 0.5;

/// Conservative peak-to-trough proxy (percent) for the volatile sleeve when
/// re-estimating a changed allocation's max drawdown; stables contribute ~0.
const VOLATILE_DRAWDOWN_PCT: f64 = 55.0;

/// Reconcile a raw model proposal into a consistent, persist-ready target.
/// Deterministic and side-effect-free — the unit-testable core of
/// [`propose_allocation`](super::service::propose_allocation):
///
/// 1. Clamp against the **designable** universe (NOT executable): a sleeve whose
///    execution rail is offline stays in the target and is surfaced at approval,
///    never silently collapsed to USDC.
/// 2. A low-confidence or empty proposal falls back to `fallback_target` (the
///    stored target), then a USDC reserve — never a no-op.
/// 3. If clamping changed the intended mix, append a risk-adjustment note so the
///    reasoning can never name an asset absent from the final allocation, and
///    re-estimate the drawdown for the kept weights.
pub(super) fn finalize_allocation(
    raw: RawAllocation<'_>,
    fallback_target: Option<&serde_json::Map<String, serde_json::Value>>,
    designable: &[&str],
    guardrails: Guardrails,
) -> FinalizedAllocation {
    let low_confidence = raw.confidence < LOW_CONFIDENCE_FALLBACK;

    // Pick the source weights: the model's, unless they're untrustworthy
    // (low-confidence) or empty, in which case fall back to the stored target,
    // then a USDC reserve.
    let source = if low_confidence || raw.weights.is_empty() {
        match fallback_target {
            Some(target) if !target.is_empty() => target,
            _ => return usdc_reserve(raw.reasoning, low_confidence, raw.confidence),
        }
    } else {
        raw.weights
    };

    let allocation = clamp_allocation(source, designable, guardrails);
    let adjustments = diff_allocations(source, &allocation);
    let changed = !adjustments.is_empty();

    let mut reasoning = if low_confidence {
        let confidence = raw.confidence;
        format!(
            "Low allocator confidence ({confidence:.2}); proposing a conservative reserve allocation. {}",
            raw.reasoning.trim()
        )
        .trim()
        .to_string()
    } else {
        raw.reasoning.trim().to_string()
    };
    if changed {
        if !reasoning.is_empty() {
            reasoning.push_str("\n\n");
        }
        reasoning.push_str(&format!(
            "Adjusted to risk limits: {}.",
            adjustments.join("; ")
        ));
    }

    let expected_max_drawdown_pct = if changed {
        Some(estimate_drawdown_pct(&allocation))
    } else {
        raw.expected_max_drawdown_pct
    };

    FinalizedAllocation {
        allocation,
        reasoning,
        expected_max_drawdown_pct,
        adjustments,
    }
}

/// A 100% USDC reserve with reasoning consistent with that target — the floor
/// fallback when there are no usable weights to clamp.
fn usdc_reserve(raw_reasoning: &str, low_confidence: bool, confidence: f64) -> FinalizedAllocation {
    let trimmed = raw_reasoning.trim();
    let reasoning = if low_confidence {
        format!(
            "Low allocator confidence ({confidence:.2}); holding a 100% USDC reserve. {trimmed}"
        )
        .trim()
        .to_string()
    } else if trimmed.is_empty() {
        "Holding a 100% USDC reserve — no valid designable weights were returned.".to_string()
    } else {
        format!(
            "Holding a 100% USDC reserve — no valid designable weights were returned. {trimmed}"
        )
    };
    FinalizedAllocation {
        allocation: std::iter::once(("USDC".to_string(), 100.0)).collect(),
        reasoning,
        expected_max_drawdown_pct: Some(0.0),
        adjustments: vec!["all weights swept to a USDC reserve".to_string()],
    }
}

/// Human-readable risk adjustments between the intended (pre-clamp, normalized)
/// weights and the final clamped allocation: dropped sleeves and significant
/// per-asset weight changes. Sub-1pp (rounding-level) deltas are ignored, so a
/// clean pass produces no note (and thus leaves reasoning/drawdown untouched).
fn diff_allocations(
    source: &serde_json::Map<String, serde_json::Value>,
    final_alloc: &std::collections::BTreeMap<String, f64>,
) -> Vec<String> {
    let total: f64 = source
        .values()
        .filter_map(serde_json::Value::as_f64)
        .filter(|w| *w > 0.0)
        .sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let mut notes = Vec::new();
    for (symbol, value) in source {
        let raw_weight = value.as_f64().unwrap_or(0.0);
        if raw_weight <= 0.0 {
            continue;
        }
        let intended = raw_weight / total * 100.0;
        let final_weight = final_alloc
            .iter()
            .find(|&(key, _)| key.as_str().eq_ignore_ascii_case(symbol))
            .map_or(0.0, |(_, w)| *w);
        if final_weight <= 0.0 {
            notes.push(format!("dropped {symbol}"));
        } else if (final_weight - intended).abs() >= 1.0 {
            notes.push(format!("{symbol} {intended:.0}->{final_weight:.0}%"));
        }
    }
    notes
}

/// Deterministic projected-max-drawdown estimate (percent) for a final mix, used
/// when clamping changed the allocation so the persisted figure matches the kept
/// weights rather than the model's pre-clamp guess.
fn estimate_drawdown_pct(alloc: &std::collections::BTreeMap<String, f64>) -> f64 {
    let volatile: f64 = alloc
        .iter()
        .filter(|(symbol, _)| !is_stable_symbol(symbol))
        .map(|(_, w)| *w)
        .sum();
    round2(volatile / 100.0 * VOLATILE_DRAWDOWN_PCT)
}

/// Read the user's high-level objective from the goal JSONB (set at onboarding).
pub(super) fn goal_objective(goal: &serde_json::Value) -> String {
    goal.get("objective")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("grow")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn clamp_keeps_designable_mix_executable_would_collapse() {
        let raw = json!({ "USDC": 60.0, "cbBTC": 20.0, "SOL": 20.0 })
            .as_object()
            .unwrap()
            .clone();
        let g = derive_guardrails("aggressive", "neutral", 0.0);
        // The bug: clamping against the executable set (just USDC when the swap
        // rail is offline) drops cbBTC/SOL and collapses to USDC 100%.
        let exec_only = clamp_allocation(&raw, &["USDC"], g);
        assert_eq!(exec_only.get("USDC").copied(), Some(100.0));
        assert!(!exec_only.contains_key("cbBTC"));
        // The fix: clamping against the designable universe preserves the mix.
        let designable = clamp_allocation(&raw, &["USDC", "cbBTC", "SOL"], g);
        assert!(designable.get("cbBTC").copied().unwrap_or(0.0) > 0.0);
        assert!(designable.get("SOL").copied().unwrap_or(0.0) > 0.0);
    }

    #[test]
    fn finalize_preserves_reasoning_and_drawdown_when_unchanged() {
        let raw = json!({ "USDC": 60.0, "cbBTC": 20.0, "SOL": 20.0 })
            .as_object()
            .unwrap()
            .clone();
        let out = finalize_allocation(
            RawAllocation {
                weights: &raw,
                reasoning: "Balanced: 60% USDC reserve, 20% cbBTC, 20% SOL.",
                confidence: 0.9,
                expected_max_drawdown_pct: Some(12.5),
            },
            None,
            &["USDC", "cbBTC", "SOL"],
            derive_guardrails("aggressive", "neutral", 0.0),
        );
        // Clean pass within risk limits → no note, reasoning + drawdown intact.
        assert!(out.adjustments.is_empty());
        assert_eq!(
            out.reasoning,
            "Balanced: 60% USDC reserve, 20% cbBTC, 20% SOL."
        );
        assert_eq!(out.expected_max_drawdown_pct, Some(12.5));
        assert!(out.allocation.contains_key("cbBTC"));
    }

    #[test]
    fn finalize_annotates_and_recomputes_when_clamped() {
        // Aggressive single-asset cap is 60; an 80% cbBTC proposal must be trimmed.
        let raw = json!({ "cbBTC": 80.0, "USDC": 20.0 })
            .as_object()
            .unwrap()
            .clone();
        let out = finalize_allocation(
            RawAllocation {
                weights: &raw,
                reasoning: "Max conviction: 80% cbBTC.",
                confidence: 0.9,
                expected_max_drawdown_pct: Some(9.0),
            },
            None,
            &["USDC", "cbBTC"],
            derive_guardrails("aggressive", "neutral", 0.0),
        );
        assert!(out.allocation.get("cbBTC").copied().unwrap() <= 60.0 + 0.05);
        // The adjustment is disclosed in reasoning, not hidden.
        assert!(!out.adjustments.is_empty());
        assert!(out.reasoning.contains("Adjusted to risk limits"));
        assert!(out.reasoning.contains("cbBTC"));
        // Drawdown is recomputed for the kept mix, not the model's stale guess.
        assert_ne!(out.expected_max_drawdown_pct, Some(9.0));
    }

    #[test]
    fn finalize_discloses_dropped_assets_in_reasoning() {
        // A non-designable token must be dropped AND disclosed — never leaving
        // reasoning that names an asset absent from the final allocation.
        let raw = json!({ "cbBTC": 50.0, "DOGE": 50.0 })
            .as_object()
            .unwrap()
            .clone();
        let out = finalize_allocation(
            RawAllocation {
                weights: &raw,
                reasoning: "Half cbBTC, half DOGE.",
                confidence: 0.9,
                expected_max_drawdown_pct: Some(40.0),
            },
            None,
            &["USDC", "cbBTC"],
            derive_guardrails("aggressive", "neutral", 0.0),
        );
        assert!(!out.allocation.contains_key("DOGE"));
        assert!(out.reasoning.contains("dropped DOGE"));
        assert!(out.allocation.contains_key("cbBTC"));
    }

    #[test]
    fn finalize_low_confidence_falls_back_to_reserve() {
        let raw = json!({ "cbBTC": 100.0 }).as_object().unwrap().clone();
        let out = finalize_allocation(
            RawAllocation {
                weights: &raw,
                reasoning: "All in cbBTC.",
                confidence: 0.2,
                expected_max_drawdown_pct: Some(50.0),
            },
            None,
            &["USDC", "cbBTC"],
            derive_guardrails("aggressive", "neutral", 0.0),
        );
        // Low confidence + no stored target → conservative USDC reserve.
        assert_eq!(out.allocation.get("USDC").copied(), Some(100.0));
        assert!(!out.allocation.contains_key("cbBTC"));
        assert!(out.reasoning.to_lowercase().contains("usdc reserve"));
    }

    #[test]
    fn finalize_low_confidence_uses_stored_target_when_present() {
        let empty = serde_json::Map::new();
        let stored = json!({ "USDC": 70.0, "cbBTC": 30.0 })
            .as_object()
            .unwrap()
            .clone();
        let out = finalize_allocation(
            RawAllocation {
                weights: &empty,
                reasoning: "",
                confidence: 0.2,
                expected_max_drawdown_pct: None,
            },
            Some(&stored),
            &["USDC", "cbBTC"],
            derive_guardrails("aggressive", "neutral", 0.0),
        );
        // Empty/low-confidence output prefers the user's stored target over USDC.
        assert!(out.allocation.get("cbBTC").copied().unwrap_or(0.0) > 0.0);
    }

    #[test]
    fn clamp_allocation_enforces_cap_floor_executable_and_sum() {
        let raw = json!({ "USDC": 10.0, "cbBTC": 90.0, "SOL": 50.0 })
            .as_object()
            .unwrap()
            .clone();
        let executable = ["USDC", "cbBTC", "WETH"];
        let g = derive_guardrails("moderate", "neutral", 0.0); // cap 45, floor 20
        let out = clamp_allocation(&raw, &executable, g);

        // SOL is not executable → dropped.
        assert!(!out.contains_key("SOL"));
        // cbBTC respects the cap even after normalization.
        assert!(out.get("cbBTC").copied().unwrap_or(0.0) <= g.single_asset_cap + 0.05);
        // Sums to ~100.
        let sum: f64 = out.values().sum();
        assert!((sum - 100.0).abs() < 0.05, "sum={sum}");
        // Stable/yield floor is met.
        let stable: f64 = out
            .iter()
            .filter(|(k, _)| is_stable_symbol(k))
            .map(|(_, w)| *w)
            .sum();
        assert!(stable >= g.stable_floor - 0.05, "stable={stable}");
    }

    #[test]
    fn clamp_allocation_empty_or_garbage_falls_back_to_usdc() {
        let empty = serde_json::Map::new();
        let out = clamp_allocation(
            &empty,
            &["USDC"],
            derive_guardrails("conservative", "neutral", 0.0),
        );
        assert_eq!(out.get("USDC").copied(), Some(100.0));

        // All non-executable → still a valid USDC-only target.
        let garbage = json!({ "DOGE": 100.0 }).as_object().unwrap().clone();
        let out = clamp_allocation(
            &garbage,
            &["USDC", "cbBTC"],
            derive_guardrails("aggressive", "neutral", 0.0),
        );
        assert_eq!(out.get("USDC").copied(), Some(100.0));
    }

    #[test]
    fn derive_guardrails_tightens_in_risk_off_and_high_vol() {
        let base = derive_guardrails("aggressive", "neutral", 0.0);
        let off = derive_guardrails("aggressive", "risk_off", 0.0);
        assert!(off.stable_floor > base.stable_floor);
        assert!(off.volatile_cluster_cap < base.volatile_cluster_cap);
        let high_vol = derive_guardrails("aggressive", "neutral", 1.2);
        assert!(high_vol.volatile_cluster_cap < base.volatile_cluster_cap);
        // RISK-2 hard ceiling always holds.
        assert!(base.single_asset_cap <= 60.0);
    }

    #[test]
    fn clamp_allocation_caps_volatile_cluster() {
        // conservative + risk_off → a low cluster cap; an all-crypto raw is
        // heavily trimmed back into the stable reserve.
        let raw = json!({ "cbBTC": 50.0, "WETH": 50.0 })
            .as_object()
            .unwrap()
            .clone();
        let g = derive_guardrails("conservative", "risk_off", 0.0);
        let out = clamp_allocation(&raw, &["USDC", "cbBTC", "WETH"], g);
        let cluster: f64 = out
            .iter()
            .filter(|(k, _)| !is_stable_symbol(k))
            .map(|(_, w)| *w)
            .sum();
        assert!(
            cluster <= g.volatile_cluster_cap + 0.1,
            "cluster={cluster} cap={}",
            g.volatile_cluster_cap
        );
    }
}
