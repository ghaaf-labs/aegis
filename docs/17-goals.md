# 17 · Discovery Goal — routing engine definition of correct

Paste this as one `/goal` condition. It keeps the evaluator transcript-provable:
the agent must run the commands and paste enough output for a fast model to
confirm the thresholds without running anything itself.

```text
A production routing engine exists for rebalancing: a pure `aegis-routing` crate provides a typed liquidity graph, route finder, min-cost-flow splitter, decomposed Decimal cost model, RouteProvider abstraction, and explicit leg DAG; apps/api uses it for review and execution planning without heuristic leg_index ordering. Proven from the transcript by:

1. `cargo test -- --nocapture` at repo root showing:
- graph covers 100% of executable registry assets and has a stable fingerprint;
- route finder returns a route for every connected pair, `None` for disconnected pairs, and <=0.5% all-in-cost gap vs the brute-force oracle;
- min-cost-flow captures >=95% of achievable convex split saving and is never worse than single path;
- cost model populates amm fee, impact, bridge, gas-USDC, and slippage components as Decimal, has convex+monotone impact, and is within 25 bps of the quoter sample;
- synthetic provider routes through the unchanged solver;
- leg DAG has valid topo order, no false dependency on independent branches, and conservation exactness;
- p95 route planning latency <=50ms.

2. `cargo build -p aegis-routing` and `cargo tree -p aegis-routing` showing no axum/sqlx/reqwest normal dependency.

3. In `apps/api`: `cargo test -- --nocapture`, `cargo clippy --all-targets --features real-swap -- -D warnings`, and `rg "HashMap<String, *f64>" ../../crates/aegis-routing/src` returning nothing.

No cheating: do not hardcode token lists or fixture-only answers, relax oracle thresholds, move web/db types into the crate, treat disconnected pairs as routed, ignore wallet chain balances, skip live quote/balance blockers, or encode dependencies through implicit leg order. If any proof command cannot pass, fix the design or stop with the failing output and root cause. Or stop after 35 turns.
```
