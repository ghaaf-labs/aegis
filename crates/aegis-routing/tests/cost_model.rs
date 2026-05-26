//! Metric 4 — decomposed Decimal cost model with a convex + monotone price-impact
//! curve, calibrated within 25 bps of the quoter.
//! Run: `cargo test -p aegis-routing cost_model -- --nocapture`.

mod common;

use aegis_routing::cost::{BucketedCurve, ConstProductCurve, CostCurve, ValueUsd};
use common::{cents, dec};
use proptest::prelude::*;
use rust_decimal::Decimal;

fn v(n: i64) -> ValueUsd {
    ValueUsd::usd(dec(n))
}

#[test]
fn cost_model_populates_every_decomposed_component() {
    let c = ConstProductCurve::new(dec(1_000_000), dec(5), cents(40)).with_protocol_fee(dec(25));
    let cost = c.cost(v(100_000));
    assert!(cost.amm_fee > Decimal::ZERO, "amm_fee must be populated");
    assert!(
        cost.price_impact > Decimal::ZERO,
        "price_impact must be populated"
    );
    assert!(
        cost.protocol_fee > Decimal::ZERO,
        "protocol_fee must be populated"
    );
    assert!(cost.gas_usdc > Decimal::ZERO, "gas_usdc must be populated");

    // all_in is exactly the component sum (no hidden f64 term).
    let sum = cost.amm_fee
        + cost.price_impact
        + cost.bridge_fee
        + cost.gateway_fee
        + cost.forwarding_fee
        + cost.protocol_fee
        + cost.gas_usdc
        + cost.slippage_budget;
    assert_eq!(cost.all_in(), sum);
    println!(
        "cost_model: amm={} impact={} protocol={} gas={} all_in={}",
        cost.amm_fee,
        cost.price_impact,
        cost.protocol_fee,
        cost.gas_usdc,
        cost.all_in()
    );
}

#[test]
fn cost_model_bucketed_curve_tracks_quoter_within_25bps() {
    // Ground-truth "quoter" = exact constant product. Calibrate the production
    // bucket curve from its samples, then check accuracy on an off-bucket grid.
    let truth = ConstProductCurve::new(dec(5_000_000), dec(5), cents(40));
    // Geometric bucket boundaries (×1.5 from an $8k base to $400k): a real
    // QuoterV2 calibrator samples densely where the convex curve bends (small
    // trades), sparsely where it flattens. Uniform spacing fails here — a single
    // chord from 0 grossly over-prices tiny trades whose true impact is ~0.
    let mut boundaries = vec![Decimal::ZERO];
    let cap = dec(400_000);
    let mut s = dec(8_000);
    while s < cap {
        boundaries.push(s);
        s = s * dec(3) / dec(2);
    }
    boundaries.push(cap);
    let curve = BucketedCurve::calibrate(&truth, &boundaries, dec(5), cents(40)).unwrap();

    let mut worst_bps = Decimal::ZERO;
    let mut s = dec(500);
    while s <= dec(400_000) {
        let approx = curve.cost(ValueUsd::usd(s)).price_impact;
        let exact = truth.exact_impact(ValueUsd::usd(s));
        let err_bps = (approx - exact).abs() / s * dec(10_000);
        worst_bps = worst_bps.max(err_bps);
        s += dec(733); // off-bucket stride
    }
    println!("cost_model: bucketed-vs-quoter worst impact error {worst_bps} bps (cap 25)");
    assert!(
        worst_bps <= dec(25),
        "calibration error {worst_bps} bps exceeds 25 bps"
    );
}

#[test]
fn cost_model_rejects_a_non_convex_calibration() {
    // A concave sample set (decreasing slope) must be refused, never routed on.
    let samples = vec![
        (dec(0), dec(0)),
        (dec(100), dec(50)), // slope 0.5
        (dec(200), dec(60)), // slope 0.1 — decreased ⇒ not convex
    ];
    let err = BucketedCurve::from_samples(samples, dec(5), Decimal::ZERO);
    assert!(err.is_err(), "non-convex calibration must be rejected");
    println!("cost_model: non-convex calibration rejected ({err:?})");
}

proptest! {
    // Convex + monotone: bigger trade ⇒ no cheaper marginal and no lower total.
    #[test]
    fn cost_model_impact_is_convex_and_monotone(
        s1 in 1i64..500_000,
        delta in 1i64..500_000,
        step in 1i64..5_000,
    ) {
        let curve = ConstProductCurve::new(dec(5_000_000), dec(5), cents(40));
        let s2 = s1 + delta;
        let step_d = dec(step);
        let eps = Decimal::new(1, 6); // tolerate sub-µ$ Decimal rounding

        let m1 = curve.marginal(ValueUsd::usd(dec(s1)), step_d);
        let m2 = curve.marginal(ValueUsd::usd(dec(s2)), step_d);
        prop_assert!(m2 + eps >= m1, "marginal must be non-decreasing: m({s1})={m1} m({s2})={m2}");

        let c1 = curve.all_in(ValueUsd::usd(dec(s1)));
        let c2 = curve.all_in(ValueUsd::usd(dec(s2)));
        prop_assert!(c2 + eps >= c1, "all-in cost must be monotone increasing");
    }
}
