//! Metric 7 — the execution leg DAG: explicit dependencies + a valid topological
//! order, independent routes carry no false dependency, and value is conserved.
//! Run: `cargo test -p aegis-routing leg_dag -- --nocapture`.

mod common;

use aegis_routing::{find_route, FlowAllocation, LegDag, ValueUsd};
use common::{asset, dec, three_chain_graph};
use rust_decimal::Decimal;

/// Build two independent routes — a same-chain swap and a 3-leg cross-chain
/// route — and compile them into one DAG.
fn two_route_dag() -> LegDag {
    let g = three_chain_graph();
    let size = ValueUsd::usd(dec(10_000));

    // Route A: Base ETH → Base USDC (1 leg, same chain).
    let route_a = find_route(&g, &asset(6, "ETH"), &asset(6, common::USDC), size)
        .unwrap()
        .unwrap();
    // Route B: Arc USYC → Base ETH (3 legs: swap, bridge, swap).
    let route_b = find_route(&g, &asset(26, "USYC"), &asset(6, "ETH"), size)
        .unwrap()
        .unwrap();
    assert_eq!(
        route_a.hops(),
        1,
        "route A should be a single same-chain leg"
    );
    assert!(route_b.hops() >= 3, "route B should cross chains (≥3 legs)");

    let allocations = vec![
        FlowAllocation {
            legs: route_a.legs,
            value: size,
        },
        FlowAllocation {
            legs: route_b.legs,
            value: size,
        },
    ];
    LegDag::compile(&g, &allocations)
}

#[test]
fn leg_dag_topological_order_respects_every_dependency() {
    let dag = two_route_dag();
    let order = dag.topological_order().expect("DAG must be acyclic");
    assert_eq!(order.len(), dag.legs.len());

    // Each leg's dependencies appear strictly before it in the order.
    let mut position = vec![0usize; dag.legs.len()];
    for (pos, &id) in order.iter().enumerate() {
        position[id] = pos;
    }
    for leg in &dag.legs {
        for &dep in &leg.depends_on {
            assert!(
                position[dep] < position[leg.id],
                "dependency {dep} must precede leg {} in topo order",
                leg.id
            );
        }
    }
    println!(
        "leg_dag: {} legs, valid topological order {order:?}, roots {:?}",
        dag.legs.len(),
        dag.roots()
    );
}

#[test]
fn leg_dag_independent_branches_have_no_false_dependency() {
    let dag = two_route_dag();
    // Route A is leg 0 (1 leg); route B is legs 1.. .
    let branch_first: Vec<usize> = vec![0];
    let branch_second: Vec<usize> = (1..dag.legs.len()).collect();

    for &a in &branch_first {
        for &b in &branch_second {
            assert!(
                !dag.depends_transitively(a, b),
                "route-A leg {a} must not depend on route-B leg {b}"
            );
            assert!(
                !dag.depends_transitively(b, a),
                "route-B leg {b} must not depend on route-A leg {a}"
            );
        }
    }
    // Two routes ⇒ two independent roots that may start concurrently.
    assert_eq!(dag.roots().len(), 2, "two independent routes ⇒ two roots");
    println!(
        "leg_dag: {} independent branches verified, no false cross-route dependency",
        dag.roots().len()
    );
}

#[test]
fn leg_dag_conserves_value_in_equals_out_plus_fees() {
    let dag = two_route_dag();
    // Tolerance: sub-cent — conservation must hold to the atomic unit modulo
    // Decimal display rounding.
    let tol = Decimal::new(1, 6);
    dag.check_conservation(tol)
        .expect("value in must equal value out + fees");

    let fees: Decimal = dag
        .legs
        .iter()
        .map(|l| l.value_in.amount() - l.value_out.amount())
        .sum();
    println!("leg_dag: conservation holds (Σ fees across legs = {fees}, residual ≤ {tol})");
}
