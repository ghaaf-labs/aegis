//! `RoutableSnapshot` — a frozen, content-fingerprinted view of which sleeves
//! can settle *right now*.
//!
//! Executability is discovered live (via the single `route_state_for_token`
//! authority) but **captured once** per decision, so the agent, planner, and
//! approval gate all read the *same* routability rather than re-deriving it at
//! three different moments (the drift that produced phantom targets and dead-end
//! plans). The content `hash` lets approval detect that a rail or material price
//! bucket changed since the plan was built (INV-4: one executability authority;
//! INV-6: a plan is bound to the routing context it was solved against).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

use crate::config::Config;

use super::models::PlannedLeg;
use super::registry::{
    allocation_target_symbols, route_state_for_token, RouteState, RuntimeCapabilities,
};

#[derive(Debug, Clone)]
pub struct RoutableSnapshot {
    /// Live route state of every designable sleeve, frozen at capture. `BTreeMap`
    /// so iteration order — and therefore the fingerprint — is deterministic.
    states: BTreeMap<String, RouteState>,
    captured_at: DateTime<Utc>,
    hash: String,
}

impl RoutableSnapshot {
    /// Freeze the live route state of every designable sleeve.
    pub fn capture(caps: &RuntimeCapabilities, cfg: &Config) -> Self {
        Self::capture_with_prices(caps, cfg, &HashMap::new())
    }

    /// Freeze route state plus the price buckets used to derive quote bounds.
    /// Bucketing at roughly 25 bps avoids invalidating reviews on tiny provider
    /// noise while still catching material price moves that would alter min_out.
    pub fn capture_with_prices(
        caps: &RuntimeCapabilities,
        cfg: &Config,
        prices: &HashMap<String, f64>,
    ) -> Self {
        let states: BTreeMap<String, RouteState> = allocation_target_symbols(cfg)
            .into_iter()
            .map(|symbol| (symbol.to_string(), route_state_for_token(caps, cfg, symbol)))
            .collect();
        let hash = fingerprint(&states, prices);
        Self {
            states,
            captured_at: Utc::now(),
            hash,
        }
    }

    pub fn capture_for_plan(
        caps: &RuntimeCapabilities,
        cfg: &Config,
        prices: &HashMap<String, f64>,
        legs: &[PlannedLeg],
    ) -> Self {
        let relevant = plan_price_symbols(legs);
        let prices = prices
            .iter()
            .filter(|(symbol, _)| relevant.contains(symbol.as_str()))
            .map(|(symbol, price)| (symbol.clone(), *price))
            .collect();
        Self::capture_with_prices(caps, cfg, &prices)
    }

    /// Can this sleeve settle right now? The one query the planner/agent use.
    pub fn is_ready(&self, symbol: &str) -> bool {
        matches!(self.states.get(symbol), Some(RouteState::Ready))
    }

    /// Sleeves that can settle now — the executable target universe.
    pub fn ready_symbols(&self) -> Vec<&str> {
        self.partition(true)
    }

    /// Sleeves that are designable but not settleable now (track-only), with
    /// their reason carried by the `RouteState`. Surfaced honestly, never as a
    /// silent drop.
    pub fn track_only_symbols(&self) -> Vec<&str> {
        self.partition(false)
    }

    /// Stable content hash — same routability ⇒ same hash. An approval compares
    /// this against the plan's stored hash to detect a rail change.
    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn captured_at(&self) -> DateTime<Utc> {
        self.captured_at
    }

    fn partition(&self, ready: bool) -> Vec<&str> {
        self.states
            .iter()
            .filter(|(_, state)| (**state == RouteState::Ready) == ready)
            .map(|(symbol, _)| symbol.as_str())
            .collect()
    }
}

fn plan_price_symbols(legs: &[PlannedLeg]) -> BTreeSet<&str> {
    legs.iter()
        .flat_map(|leg| [leg.src_symbol.as_deref(), leg.dest_symbol.as_deref()])
        .flatten()
        .filter(|symbol| !symbol.eq_ignore_ascii_case("USDC"))
        .collect()
}

/// Has routability changed since a plan was built? Compares the plan's stored
/// fingerprint against a freshly-captured one. A `None` stored hash means the
/// plan predates snapshot binding (legacy/mock) — treated as "no binding", never
/// a false stale (so old plans don't suddenly become un-approvable).
pub fn routability_changed(stored: Option<&str>, current: &str) -> bool {
    stored.is_some_and(|s| s != current)
}

/// Deterministic content hash of the routable set plus optional price buckets.
/// Uses each state's stable serde label, so the hash is reproducible across
/// captures of identical routing context and changes the moment any sleeve flips
/// Ready ⇄ not-Ready or any included price crosses a bucket.
fn price_bucket(price: f64) -> Option<i64> {
    (price.is_finite() && price > 0.0).then(|| (price.ln() / 1.0025_f64.ln()).round() as i64)
}

fn fingerprint(states: &BTreeMap<String, RouteState>, prices: &HashMap<String, f64>) -> String {
    let mut hasher = Sha256::new();
    for (symbol, state) in states {
        let label = serde_json::to_string(state).unwrap_or_default();
        hasher.update(symbol.as_bytes());
        hasher.update(b"=");
        hasher.update(label.as_bytes());
        hasher.update(b";");
    }
    let mut price_buckets: BTreeMap<&str, i64> = BTreeMap::new();
    for (symbol, price) in prices {
        if let Some(bucket) = price_bucket(*price) {
            price_buckets.insert(symbol.as_str(), bucket);
        }
    }
    for (symbol, bucket) in price_buckets {
        hasher.update(b"price:");
        hasher.update(symbol.as_bytes());
        hasher.update(b"=");
        hasher.update(bucket.to_string().as_bytes());
        hasher.update(b";");
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps_and_cfg() -> (RuntimeCapabilities, Config) {
        let cfg = crate::config::test_config();
        let caps = RuntimeCapabilities::from_config(&cfg);
        (caps, cfg)
    }

    #[test]
    fn usdc_is_always_ready_in_any_snapshot() {
        let (caps, cfg) = caps_and_cfg();
        let snap = RoutableSnapshot::capture(&caps, &cfg);
        assert!(snap.is_ready("USDC"), "USDC is the settlement unit");
    }

    #[test]
    fn ready_and_track_only_partition_the_designable_universe() {
        let (caps, cfg) = caps_and_cfg();
        let snap = RoutableSnapshot::capture(&caps, &cfg);
        let designable = allocation_target_symbols(&cfg).len();
        assert_eq!(
            snap.ready_symbols().len() + snap.track_only_symbols().len(),
            designable,
            "every designable sleeve is either ready or track-only, never both/neither"
        );
    }

    #[test]
    fn routability_changed_honors_legacy_null_binding() {
        // Legacy/mock plan (no stored hash) is never treated as stale.
        assert!(!routability_changed(None, "abc"));
        // Same hash ⇒ unchanged; different hash ⇒ a rail flipped ⇒ changed.
        assert!(!routability_changed(Some("abc"), "abc"));
        assert!(routability_changed(Some("abc"), "def"));
    }

    #[test]
    fn fingerprint_is_deterministic_across_identical_captures() {
        let (caps, cfg) = caps_and_cfg();
        let prices = HashMap::from([("ETH".to_string(), 2_100.0)]);
        let a = RoutableSnapshot::capture_with_prices(&caps, &cfg, &prices);
        let b = RoutableSnapshot::capture_with_prices(&caps, &cfg, &prices);
        assert_eq!(
            a.hash(),
            b.hash(),
            "same routability must fingerprint identically"
        );
        assert_eq!(a.hash().len(), 64, "sha256 hex");
    }

    #[test]
    fn fingerprint_changes_when_price_bucket_changes() {
        let (caps, cfg) = caps_and_cfg();
        let a = RoutableSnapshot::capture_with_prices(
            &caps,
            &cfg,
            &HashMap::from([("ETH".to_string(), 2_100.0)]),
        );
        let b = RoutableSnapshot::capture_with_prices(
            &caps,
            &cfg,
            &HashMap::from([("ETH".to_string(), 2_130.0)]),
        );
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn plan_snapshot_ignores_unrelated_price_moves() {
        use super::super::models::{decimal_usd, LegKind, PlannedLeg};
        use super::super::ChainKey;

        let (caps, cfg) = caps_and_cfg();
        let legs = vec![PlannedLeg {
            leg_index: 0,
            deps: vec![],
            kind: LegKind::LocalSwap,
            src_chain: Some(ChainKey::Base),
            dest_chain: Some(ChainKey::Base),
            src_symbol: Some("USDC".into()),
            dest_symbol: Some("ETH".into()),
            amount_usdc: decimal_usd(100.0),
            min_out: None,
        }];
        let a = RoutableSnapshot::capture_for_plan(
            &caps,
            &cfg,
            &HashMap::from([("ETH".to_string(), 2_100.0), ("BTC".to_string(), 90_000.0)]),
            &legs,
        );
        let b = RoutableSnapshot::capture_for_plan(
            &caps,
            &cfg,
            &HashMap::from([("ETH".to_string(), 2_100.0), ("BTC".to_string(), 100_000.0)]),
            &legs,
        );

        assert_eq!(a.hash(), b.hash());
    }
}
