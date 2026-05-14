# 01 — Architecture

> **The agent loop is regime → decide → critique → propose → human approves → cross-chain execute → observe.** Each step writes to an event log that streams to the UI over SSE.

## The loop

```
              ┌──────────────────────────────────────────────────┐
              │  TRIGGERS                                        │
              │  user · scheduler (5 min) · drift > θ · regime   │
              │  flip · webhook                                  │
              └────────────────────────┬─────────────────────────┘
                                       │
                       ┌───────────────▼────────────────┐
                       │  REGIME CLASSIFIER             │
                       │  BTC 30d vol · 90d corr · DD    │
                       │  → RiskOn / Neutral / RiskOff  │
                       │  (haiku-4-5)                   │
                       └───────────────┬────────────────┘
                                       │
                       ┌───────────────▼────────────────┐
                       │  STRATEGIST                    │
                       │  goal + regime + memory +      │
                       │  prices + harvestable losses + │
                       │  USYC rate + EURC basis        │
                       │  → proposal (opus-4-7)         │
                       └───────────────┬────────────────┘
                                       │
                       ┌───────────────▼────────────────┐
                       │  CRITIC (gpt-5)                │
                       │  adversarial pass → revisions  │
                       └───────────────┬────────────────┘
                                       │
                       ┌───────────────▼────────────────┐
                       │  HUMAN APPROVES                │
                       │  one-screen modal, USDC fee    │
                       │  preview, model slug visible   │
                       └───────────────┬────────────────┘
                                       │
                       ┌───────────────▼────────────────┐
                       │  EXECUTOR                      │
                       │  Gateway delta plan →          │
                       │  CCTP V2 + Hook swaps →        │
                       │  Paymaster pays gas in USDC    │
                       └───────────────┬────────────────┘
                                       │
                       ┌───────────────▼────────────────┐
                       │  OBSERVE                       │
                       │  agent_decisions · agent_memory│
                       │  · diary · SSE push            │
                       └────────────────────────────────┘
```

## Module map

| Concern | Path | Notes |
|---|---|---|
| AI client | `apps/api/src/modules/ai/` | OpenRouter + `ModelRoute` enum |
| Strategist + critic | `apps/api/src/modules/agent/` | Decision pipeline + memory retrieval |
| Regime classifier | `apps/api/src/modules/risk_engine/regime.rs` | Statistical + LLM hybrid |
| Risk scoring | `apps/api/src/modules/risk_engine/mod.rs` | Concentration · vol · drift · regime |
| Wallets | `apps/api/src/modules/wallet/` | Circle Wallets REST wrapper |
| Cross-chain | `apps/api/src/modules/{gateway,rebalance/cross_chain}.rs` | Gateway + CCTP V2 + Hooks |
| Yield | `apps/api/src/modules/yield/` | USDC↔USYC |
| FX | `apps/api/src/modules/fx/` | Arc StableFX (USDC↔EURC) |
| Tax | `apps/api/src/modules/tax/` | Cost-basis lots, harvestable losses |
| Realtime | `apps/api/src/modules/sse/` | `/sse` event stream |
| Strategy marketplace (cond.) | `apps/api/src/modules/strategies/` | Publish · clone · royalty |

## Data flow

1. **Triggers** are unified: a single internal `AgentTrigger` enum (`User`, `Scheduler`, `DriftAlert`, `RegimeFlip`).
2. The **classifier** runs first because it's cheap and conditions every later prompt.
3. **Memory** retrieval (last N decisions for this user, with outcomes) is bounded — see [02 — Agent design](./02-agent-design.md) for the context budget.
4. The **critic** receives the strategist's proposal as the user message and a different system prompt; the strategist can revise once.
5. **Execution** routes through Gateway: the executor computes per-chain deltas against the unified balance and emits a single user-facing approval, even when 2+ chains are touched.
6. **Observation** writes to `agent_decisions` (the proposal), `rebalance_events` (the execution), and `agent_memory` (compressed outcome 24h later, used by future retrievals).

## Realtime channel

`GET /sse` returns an `EventSource` stream with named events:

| Event | Payload |
|---|---|
| `price.tick` | `{ symbol, price_usd, change_24h, source, fetched_at }` |
| `regime.flip` | `{ from, to, confidence, signals }` |
| `agent.decision` | `{ id, portfolio_id, model, confidence, summary }` |
| `rebalance.status` | `{ id, step, chain, tx_hash?, status }` |
| `gateway.balance` | `{ unified_usdc, per_chain }` |

`KeepAlive::default()` prevents intermediate proxies from closing idle streams; clients reconnect with `retry: 3000`.

---

> **What this enables:** decisions you can rewind step-by-step, multi-chain moves that look like one click, and a UI that updates without a single client→server poll.
>
> **What it doesn't:** mid-execution cancellation (a CCTP hop is committed once submitted) and cross-user coordination (each portfolio is an isolated loop).
