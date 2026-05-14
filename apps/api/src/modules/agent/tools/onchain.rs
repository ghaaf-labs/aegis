//! `fetch_onchain_metric` tool — single on-chain signal for a chain/asset.
//!
//! Returns a deterministic snapshot seeded by `(chain, asset, metric, day)`.
//! Real testnet RPC integration is gated behind `chain_private_key_*`; until
//! the operator opts in we keep the values plausible-but-stable so the
//! agent's reasoning is reproducible.

use serde_json::{json, Value};

use crate::router::AppState;

pub async fn run(_state: &AppState, args: &Value) -> Result<String, String> {
    let chain = required_str(args, "chain")?.to_lowercase();
    let asset = required_str(args, "asset")?.to_uppercase();
    let metric = required_str(args, "metric")?.to_lowercase();

    if !is_known_metric(&metric) {
        return Err(format!(
            "unknown metric '{metric}'; valid: active_addresses_24h, tx_count_24h, fee_revenue_24h"
        ));
    }
    if !is_known_chain(&chain) {
        return Err(format!(
            "unknown chain '{chain}'; valid: arc, base, ethereum, solana"
        ));
    }

    let value = deterministic_value(&chain, &asset, &metric);
    let prev = deterministic_value_prev(&chain, &asset, &metric);
    let pct = if prev > 0.0 {
        ((value - prev) / prev) * 100.0
    } else {
        0.0
    };

    Ok(json!({
        "chain": chain,
        "asset": asset,
        "metric": metric,
        "value": value,
        "prev_24h": prev,
        "change_24h_pct": (pct * 100.0).round() / 100.0,
        "source": "synthetic-stable",
    })
    .to_string())
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("missing required arg: {key}"))
}

fn is_known_metric(m: &str) -> bool {
    matches!(
        m,
        "active_addresses_24h" | "tx_count_24h" | "fee_revenue_24h"
    )
}

fn is_known_chain(c: &str) -> bool {
    matches!(c, "arc" | "base" | "ethereum" | "solana")
}

/// Deterministic per-(chain, asset, metric) base value. We keep it stable so
/// the agent can't get whipsawed by random noise between iterations.
fn deterministic_value(chain: &str, asset: &str, metric: &str) -> f64 {
    let seed = hash_to_unit(&format!("{chain}:{asset}:{metric}"));
    let base = match metric {
        "active_addresses_24h" => 50_000.0 + seed * 450_000.0,
        "tx_count_24h" => 200_000.0 + seed * 1_800_000.0,
        "fee_revenue_24h" => 10_000.0 + seed * 240_000.0,
        _ => 0.0,
    };
    (base * 100.0).round() / 100.0
}

fn deterministic_value_prev(chain: &str, asset: &str, metric: &str) -> f64 {
    let seed = hash_to_unit(&format!("prev:{chain}:{asset}:{metric}"));
    let base = match metric {
        "active_addresses_24h" => 50_000.0 + seed * 450_000.0,
        "tx_count_24h" => 200_000.0 + seed * 1_800_000.0,
        "fee_revenue_24h" => 10_000.0 + seed * 240_000.0,
        _ => 0.0,
    };
    (base * 100.0).round() / 100.0
}

/// Public re-export for sibling tools that want the same deterministic
/// hash distribution without each module reinventing it.
pub fn hash_to_unit_pub(input: &str) -> f64 {
    hash_to_unit(input)
}

/// 64-bit FNV-1a → [0,1). No external dep; not crypto-quality but uniform
/// enough for plausible-looking demo data.
fn hash_to_unit(input: &str) -> f64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in input.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    (h % 1_000_000) as f64 / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_metric() {
        let err = is_known_metric("foo");
        assert!(!err);
    }

    #[test]
    fn deterministic_value_is_stable() {
        let a = deterministic_value("base", "ETH", "tx_count_24h");
        let b = deterministic_value("base", "ETH", "tx_count_24h");
        assert!((a - b).abs() < 1e-9);
    }

    #[test]
    fn hash_to_unit_is_in_range() {
        for s in ["a", "BTC:base:tx_count_24h", ""] {
            let v = hash_to_unit(s);
            assert!((0.0..1.0).contains(&v), "out of range: {v} for {s}");
        }
    }
}
