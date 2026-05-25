//! The typed liquidity graph (spec §7.1): a directed multigraph whose nodes are
//! `(chain, token)` assets and whose edges are typed settlement rails carrying a
//! convex [`CostCurve`]. Node order is deterministic (`BTreeMap`), adjacency is
//! CSR-style, and the whole graph collapses to a 32-byte SHA-256 fingerprint so
//! an identical graph hashes identically regardless of build order (INV-6).

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use crate::cost::CostCurve;
use crate::domain::{Asset, EdgeKind, ProviderId};

pub type NodeIdx = usize;
pub type EdgeIdx = usize;

/// One directed rail between two assets.
pub struct Edge {
    pub from: NodeIdx,
    pub to: NodeIdx,
    pub kind: EdgeKind,
    pub provider: ProviderId,
    pub curve: Box<dyn CostCurve>,
}

/// The immutable, fingerprinted liquidity graph the solver runs over.
pub struct LiquidityGraph {
    nodes: Vec<Asset>,
    index: BTreeMap<Asset, NodeIdx>,
    edges: Vec<Edge>,
    adjacency: Vec<Vec<EdgeIdx>>,
    fingerprint: [u8; 32],
}

impl LiquidityGraph {
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
    pub fn nodes(&self) -> &[Asset] {
        &self.nodes
    }
    pub fn edge(&self, e: EdgeIdx) -> &Edge {
        &self.edges[e]
    }
    /// Edges leaving `node`, in deterministic (sorted) order.
    pub fn out_edges(&self, node: NodeIdx) -> &[EdgeIdx] {
        &self.adjacency[node]
    }
    pub fn node_index(&self, asset: &Asset) -> Option<NodeIdx> {
        self.index.get(asset).copied()
    }
    pub fn asset(&self, node: NodeIdx) -> &Asset {
        &self.nodes[node]
    }
    pub fn contains(&self, asset: &Asset) -> bool {
        self.index.contains_key(asset)
    }
    /// The deterministic graph id (`graph_id` in the spec). Two graphs built
    /// from the same assets + edges in any order produce the same bytes.
    pub fn fingerprint(&self) -> [u8; 32] {
        self.fingerprint
    }
    pub fn fingerprint_hex(&self) -> String {
        use std::fmt::Write as _;
        self.fingerprint.iter().fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
    }
}

/// A pending edge before node indices are assigned.
struct EdgeSpec {
    from: Asset,
    to: Asset,
    kind: EdgeKind,
    provider: ProviderId,
    curve: Box<dyn CostCurve>,
}

/// Accumulates a universe of assets + provider edges, then materializes a
/// deterministic [`LiquidityGraph`].
#[derive(Default)]
pub struct GraphBuilder {
    assets: BTreeSet<Asset>,
    edges: Vec<EdgeSpec>,
}

impl GraphBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare an asset node (idempotent). Endpoints of any added edge are
    /// declared automatically, so this is only needed for isolated nodes.
    pub fn add_asset(&mut self, asset: Asset) -> &mut Self {
        self.assets.insert(asset);
        self
    }

    /// Add a directed edge, declaring both endpoints.
    pub fn add_edge(
        &mut self,
        from: Asset,
        to: Asset,
        kind: EdgeKind,
        provider: ProviderId,
        curve: Box<dyn CostCurve>,
    ) -> &mut Self {
        self.assets.insert(from.clone());
        self.assets.insert(to.clone());
        self.edges.push(EdgeSpec {
            from,
            to,
            kind,
            provider,
            curve,
        });
        self
    }

    pub fn asset_count(&self) -> usize {
        self.assets.len()
    }

    /// Materialize the graph. Node indices follow `BTreeSet` (sorted) order and
    /// adjacency lists are sorted, so construction is fully deterministic.
    pub fn build(self) -> LiquidityGraph {
        let nodes: Vec<Asset> = self.assets.into_iter().collect();
        let index: BTreeMap<Asset, NodeIdx> = nodes
            .iter()
            .enumerate()
            .map(|(i, a)| (a.clone(), i))
            .collect();

        let mut edges: Vec<Edge> = self
            .edges
            .into_iter()
            .map(|e| Edge {
                from: index[&e.from],
                to: index[&e.to],
                kind: e.kind,
                provider: e.provider,
                curve: e.curve,
            })
            .collect();

        // Deterministic edge order: (from, to, kind, provider, curve params).
        edges.sort_by(|a, b| {
            (a.from, a.to, a.kind, &a.provider, a.curve.fingerprint()).cmp(&(
                b.from,
                b.to,
                b.kind,
                &b.provider,
                b.curve.fingerprint(),
            ))
        });

        let mut adjacency: Vec<Vec<EdgeIdx>> = vec![Vec::new(); nodes.len()];
        for (ei, e) in edges.iter().enumerate() {
            adjacency[e.from].push(ei);
        }

        let fingerprint = Self::compute_fingerprint(&nodes, &edges);
        LiquidityGraph {
            nodes,
            index,
            edges,
            adjacency,
            fingerprint,
        }
    }

    fn compute_fingerprint(nodes: &[Asset], edges: &[Edge]) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b"aegis-routing-graph-v1");
        h.update((nodes.len() as u64).to_le_bytes());
        for a in nodes {
            h.update(a.chain.0.to_le_bytes());
            h.update((a.token.as_str().len() as u64).to_le_bytes());
            h.update(a.token.as_str().as_bytes());
        }
        h.update((edges.len() as u64).to_le_bytes());
        for e in edges {
            h.update((e.from as u64).to_le_bytes());
            h.update((e.to as u64).to_le_bytes());
            h.update([e.kind as u8]);
            h.update(e.provider.as_str().as_bytes());
            h.update(b"|");
            h.update(e.curve.fingerprint());
        }
        h.finalize().into()
    }
}
