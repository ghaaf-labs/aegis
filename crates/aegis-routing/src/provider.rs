//! The `RouteProvider` abstraction (spec §9) — the Strategy/open-closed seam.
//!
//! Each settlement rail (a DEX venue, CCTP, Gateway, USYC, …) is a provider that
//! *contributes typed edges* to the graph. The solver, cost model, and DAG never
//! name a concrete rail — so adding a venue or chain is a new `RouteProvider`
//! impl plus config, with **zero** changes to `solve`/`min_cost_flow`/the DAG
//! (metric 5 / M10). The synthetic-provider test proves exactly this.

use crate::graph::{GraphBuilder, LiquidityGraph};
use crate::ProviderId;

/// A source of liquidity edges. apps/api implements one per Circle rail; tests
/// implement synthetic ones — the solver cannot tell the difference.
pub trait RouteProvider {
    /// Stable id (folded into the graph fingerprint + plan audit).
    fn provider_id(&self) -> ProviderId;

    /// Add this provider's nodes + edges (with their cost curves) to the graph
    /// under construction.
    fn contribute(&self, builder: &mut GraphBuilder);
}

/// Assemble a [`LiquidityGraph`] from a set of providers. This is the *only*
/// place rails meet the graph; everything downstream is rail-agnostic.
pub fn assemble(providers: &[&dyn RouteProvider]) -> LiquidityGraph {
    let mut builder = GraphBuilder::new();
    for p in providers {
        p.contribute(&mut builder);
    }
    builder.build()
}
