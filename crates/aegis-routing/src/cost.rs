//! The first-class, decomposed cost model (spec §7.2).
//!
//! Every edge prices a trade through a [`CostCurve`]. The all-in cost is the sum
//! of typed `Decimal` components — never an `f64`, never a stringly-typed map.
//! Price impact is **convex and monotone** by construction: that is the
//! mathematical property that makes splitting a trade beneficial and makes the
//! successive-shortest-path solver provably min-cost.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// A USD value. Private field (INV-2): a value is only ever built from an
/// explicit constructor, never `From<f64>` — so a stale float can't leak in as a
/// dollar amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ValueUsd(Decimal);

impl ValueUsd {
    pub const ZERO: Self = Self(Decimal::ZERO);

    /// A raw USD amount.
    pub fn usd(amount: Decimal) -> Self {
        Self(amount)
    }

    /// Mark a quantity at a price → a USD value (the only multiplicative ctor).
    pub fn mark(qty: Decimal, price: Decimal) -> Self {
        Self(qty * price)
    }

    pub fn amount(self) -> Decimal {
        self.0
    }

    pub fn is_positive(self) -> bool {
        self.0 > Decimal::ZERO
    }

    /// Lossy projection to `f64` — only for benchmark/stat reporting, never for
    /// money math (mirrors apps/api's `serde::float` stat boundary).
    pub fn to_f64(self) -> f64 {
        self.0.to_f64().unwrap_or(0.0)
    }
}

/// All-in cost of traversing one edge at a given trade size, decomposed so the
/// UI can show provenance and the solver can tie-break lexicographically. Every
/// component is denominated in USDC (value lost), as a `Decimal`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeCost {
    /// DEX swap fee (the pool's fee tier, e.g. 5 bps).
    pub amm_fee: Decimal,
    /// Slippage from finite liquidity — the convex, size-dependent term.
    pub price_impact: Decimal,
    /// CCTP burn fee (Circle's `minimumFee`, bps of notional).
    pub bridge_fee: Decimal,
    /// Circle Gateway move fee.
    pub gateway_fee: Decimal,
    /// Forwarder execution fee on a hooked dest-swap.
    pub forwarding_fee: Decimal,
    /// Aegis protocol fee (25 bps, single-sourced in apps/api billing).
    pub protocol_fee: Decimal,
    /// Gas paid in USDC via the Paymaster — a fixed (size-independent) charge.
    pub gas_usdc: Decimal,
    /// Reserved slippage budget the executor will not exceed.
    pub slippage_budget: Decimal,
}

impl EdgeCost {
    /// Total cost — the scalar the solver minimizes.
    pub fn all_in(&self) -> Decimal {
        self.amm_fee
            + self.price_impact
            + self.bridge_fee
            + self.gateway_fee
            + self.forwarding_fee
            + self.protocol_fee
            + self.gas_usdc
            + self.slippage_budget
    }
}

/// A size-dependent cost function for one edge. The crate stays open/closed: a
/// new rail supplies its own `CostCurve` impl and the solver is unchanged
/// (metric 5 / M10).
pub trait CostCurve: Send + Sync {
    /// The decomposed cost of pushing `size` of value across this edge.
    fn cost(&self, size: ValueUsd) -> EdgeCost;

    /// Canonical bytes identifying this curve's parameters — folded into the
    /// graph fingerprint so an identical graph hashes identically (INV-6).
    fn fingerprint(&self) -> Vec<u8>;

    /// All-in cost at `size` (the scalar Dijkstra/SSP weight).
    fn all_in(&self, size: ValueUsd) -> Decimal {
        self.cost(size).all_in()
    }

    /// Finite-difference marginal cost of the next `step` of value at current
    /// `flow`: `(C(flow+step) − C(flow)) / step`. Finite differences (not the
    /// analytic derivative) are deliberate — they let the **fixed** gas charge
    /// register on an edge's first increment, so the SSP solver won't open a new
    /// route whose gas outweighs the convex saving.
    fn marginal(&self, flow: ValueUsd, step: Decimal) -> Decimal {
        debug_assert!(step > Decimal::ZERO, "marginal step must be positive");
        let lo = self.all_in(flow);
        let hi = self.all_in(ValueUsd::usd(flow.amount() + step));
        (hi - lo) / step
    }
}

fn bps(value: Decimal, basis_points: Decimal) -> Decimal {
    value * basis_points / Decimal::from(10_000)
}

/// An AMM edge priced by the **exact constant-product** invariant. With a pool
/// balanced to `depth` USD per side (`x = y = depth`, `x·y = k`), pushing `s` USD
/// in moves `x → depth + s`, so the value actually received is `depth·s/(depth+s)`
/// and the impact loss is `s²/(depth+s)` — convex and monotone in `s`. This is
/// the ground-truth "quoter" the calibrated [`BucketedCurve`] is checked against
/// (M8), and a usable production curve when pool reserves are known.
#[derive(Debug, Clone)]
pub struct ConstProductCurve {
    depth_usd: Decimal,
    fee_bps: Decimal,
    gas_usdc: Decimal,
    protocol_fee_bps: Decimal,
}

impl ConstProductCurve {
    pub fn new(depth_usd: Decimal, fee_bps: Decimal, gas_usdc: Decimal) -> Self {
        assert!(depth_usd > Decimal::ZERO, "pool depth must be positive");
        Self {
            depth_usd,
            fee_bps,
            gas_usdc,
            protocol_fee_bps: Decimal::ZERO,
        }
    }

    #[must_use]
    pub fn with_protocol_fee(mut self, protocol_fee_bps: Decimal) -> Self {
        self.protocol_fee_bps = protocol_fee_bps;
        self
    }

    /// The exact constant-product impact loss for a trade of `size` USD.
    pub fn exact_impact(&self, size: ValueUsd) -> Decimal {
        let s = size.amount();
        if s <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        s * s / (self.depth_usd + s)
    }
}

impl CostCurve for ConstProductCurve {
    fn cost(&self, size: ValueUsd) -> EdgeCost {
        let s = size.amount().max(Decimal::ZERO);
        EdgeCost {
            amm_fee: bps(s, self.fee_bps),
            price_impact: self.exact_impact(size),
            protocol_fee: bps(s, self.protocol_fee_bps),
            gas_usdc: self.gas_usdc,
            ..EdgeCost::default()
        }
    }

    fn fingerprint(&self) -> Vec<u8> {
        let mut v = b"const_product".to_vec();
        for d in [
            self.depth_usd,
            self.fee_bps,
            self.gas_usdc,
            self.protocol_fee_bps,
        ] {
            v.extend_from_slice(&d.serialize());
        }
        v
    }
}

/// A bridge / Gateway / USYC edge: a linear bps fee + a flat fee + fixed gas,
/// with no price impact (a 1:1 USDC move). Convex (linear) and monotone.
#[derive(Debug, Clone)]
pub struct BridgeCurve {
    fee_bps: Decimal,
    flat_fee: Decimal,
    gas_usdc: Decimal,
    component: BridgeComponent,
}

/// Which decomposed `EdgeCost` slot a [`BridgeCurve`]'s bps fee lands in.
#[derive(Debug, Clone, Copy)]
pub enum BridgeComponent {
    Bridge,
    Gateway,
    Forwarding,
}

impl BridgeCurve {
    pub fn new(
        fee_bps: Decimal,
        flat_fee: Decimal,
        gas_usdc: Decimal,
        component: BridgeComponent,
    ) -> Self {
        Self {
            fee_bps,
            flat_fee,
            gas_usdc,
            component,
        }
    }
}

impl CostCurve for BridgeCurve {
    fn cost(&self, size: ValueUsd) -> EdgeCost {
        let s = size.amount().max(Decimal::ZERO);
        let fee = bps(s, self.fee_bps) + self.flat_fee;
        let mut c = EdgeCost {
            gas_usdc: self.gas_usdc,
            ..EdgeCost::default()
        };
        match self.component {
            BridgeComponent::Bridge => c.bridge_fee = fee,
            BridgeComponent::Gateway => c.gateway_fee = fee,
            BridgeComponent::Forwarding => c.forwarding_fee = fee,
        }
        c
    }

    fn fingerprint(&self) -> Vec<u8> {
        let mut v = b"bridge".to_vec();
        v.push(match self.component {
            BridgeComponent::Bridge => 0,
            BridgeComponent::Gateway => 1,
            BridgeComponent::Forwarding => 2,
        });
        for d in [self.fee_bps, self.flat_fee, self.gas_usdc] {
            v.extend_from_slice(&d.serialize());
        }
        v
    }
}

/// A production AMM curve **calibrated from size-bucketed quotes** (QuoterV2 in
/// apps/api; the exact pool in tests). Impact is stored as cumulative loss at
/// each bucket boundary and interpolated piecewise-linearly. Convexity is
/// enforced at construction (marginals must be non-decreasing), so the solver's
/// correctness assumptions always hold.
#[derive(Debug, Clone)]
pub struct BucketedCurve {
    /// `(size, cumulative_impact)` ascending in size, with non-decreasing slope.
    buckets: Vec<(Decimal, Decimal)>,
    fee_bps: Decimal,
    gas_usdc: Decimal,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CurveError {
    #[error("a bucketed curve needs at least two points")]
    TooFewBuckets,
    #[error("bucket sizes must be strictly ascending and start at 0")]
    BadBucketSizes,
    #[error("calibrated impact is not convex (marginal decreased) at bucket {0}")]
    NotConvex(usize),
}

impl BucketedCurve {
    /// Build from `(size, cumulative_impact)` samples. Rejects a non-convex
    /// sample set rather than silently routing on a bad curve.
    pub fn from_samples(
        mut buckets: Vec<(Decimal, Decimal)>,
        fee_bps: Decimal,
        gas_usdc: Decimal,
    ) -> Result<Self, CurveError> {
        buckets.sort_by_key(|b| b.0);
        if buckets.len() < 2 {
            return Err(CurveError::TooFewBuckets);
        }
        if buckets[0].0 != Decimal::ZERO {
            return Err(CurveError::BadBucketSizes);
        }
        let mut prev_slope: Option<Decimal> = None;
        for i in 1..buckets.len() {
            let (s0, c0) = buckets[i - 1];
            let (s1, c1) = buckets[i];
            if s1 <= s0 {
                return Err(CurveError::BadBucketSizes);
            }
            let slope = (c1 - c0) / (s1 - s0);
            if let Some(p) = prev_slope {
                if slope < p {
                    return Err(CurveError::NotConvex(i));
                }
            }
            prev_slope = Some(slope);
        }
        Ok(Self {
            buckets,
            fee_bps,
            gas_usdc,
        })
    }

    /// Calibrate from any ground-truth curve by sampling its exact all-in impact
    /// at `boundaries` (the two-pass quoting step, modelled purely).
    pub fn calibrate(
        truth: &ConstProductCurve,
        boundaries: &[Decimal],
        fee_bps: Decimal,
        gas_usdc: Decimal,
    ) -> Result<Self, CurveError> {
        let mut samples: Vec<(Decimal, Decimal)> = boundaries
            .iter()
            .map(|&s| (s, truth.exact_impact(ValueUsd::usd(s))))
            .collect();
        if !samples.iter().any(|(s, _)| *s == Decimal::ZERO) {
            samples.push((Decimal::ZERO, Decimal::ZERO));
        }
        Self::from_samples(samples, fee_bps, gas_usdc)
    }

    fn interp_impact(&self, size: Decimal) -> Decimal {
        if size <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        // Past the last bucket, extrapolate along the final (steepest) slope —
        // conservative for a convex curve (never under-prices a large trade).
        let last = self.buckets.len() - 1;
        if size >= self.buckets[last].0 {
            let (s0, c0) = self.buckets[last - 1];
            let (s1, c1) = self.buckets[last];
            let slope = (c1 - c0) / (s1 - s0);
            return c1 + slope * (size - s1);
        }
        // Locate the bracketing bucket and linearly interpolate.
        for i in 1..self.buckets.len() {
            let (s0, c0) = self.buckets[i - 1];
            let (s1, c1) = self.buckets[i];
            if size <= s1 {
                let slope = (c1 - c0) / (s1 - s0);
                return c0 + slope * (size - s0);
            }
        }
        self.buckets[last].1
    }
}

impl CostCurve for BucketedCurve {
    fn cost(&self, size: ValueUsd) -> EdgeCost {
        let s = size.amount().max(Decimal::ZERO);
        EdgeCost {
            amm_fee: bps(s, self.fee_bps),
            price_impact: self.interp_impact(s),
            gas_usdc: self.gas_usdc,
            ..EdgeCost::default()
        }
    }

    fn fingerprint(&self) -> Vec<u8> {
        let mut v = b"bucketed".to_vec();
        for (s, c) in &self.buckets {
            v.extend_from_slice(&s.serialize());
            v.extend_from_slice(&c.serialize());
        }
        v.extend_from_slice(&self.fee_bps.serialize());
        v.extend_from_slice(&self.gas_usdc.serialize());
        v
    }
}
