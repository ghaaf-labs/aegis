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

const STABLE_SYMBOLS: &[&str] = &["USDC", "sUSDS", "USYC", "EURC", "aUSDC"];

fn is_stable_symbol(sym: &str) -> bool {
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
