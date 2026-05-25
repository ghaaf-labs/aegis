# 16 · Settlement Engine (`feat/settlement-engine`)

What this branch did, condensed. It closes the **allocation↔execution gap**: the
agent picked target _weights_, but the executor moved _dollars_ — and the two
disagreed. Symptoms: a red **409 dead-end**, **phantom sells** (selling assets
worth $0), and silently-dropped sleeves. Cure: **one value source, one typed
result, one money-safe state machine.**

---

## 1. The plan pipeline (no more 409)

```
POST /rebalance/plan
        │
        ▼
  build_plan_input ──► read LIVE wallet once (USDC + token holdings)
        │                value ONLY what's sellable · INV-1/2
        ▼
   plan_legs() ──► sells first (free USDC) ─► buys ─► cross-chain bridges
        │
        ▼
   PlanOutcome   ◄── ALWAYS HTTP 200, never a red error
```

`PlanOutcome` is a tagged union — the UI branches on `status`, never throws:

| status                                | meaning                          | UI                      |
| ------------------------------------- | -------------------------------- | ----------------------- |
| `executable`                          | real legs to approve             | → review screen         |
| `partial_deferred`                    | legs **+** sleeves held back     | → review, show deferred |
| `on_target_noop` / `reserve_fallback` | already where you want           | calm ✓                  |
| `unfunded` / `dust_only` / `blocked`  | actionable (fund / route opened) | notice                  |
| `balance_unavailable`                 | Circle hiccup, retry             | retry notice            |

> The old code raised `409 Conflict` for every one of these. Now `Conflict` is
> reserved for _concurrent execution_ / _wallet-not-ready_ only.

---

## 2. Where "value" comes from (the single source — INV-1)

```
            ┌─────────────── real Circle wallet ───────────────┐
            │  idle USDC (Gateway)      sellable token holdings │
            │      │                         │ (only EXECUTABLE, │
            │      │                         │  on its sell chain)│
            └──────┼─────────────────────────┼───────────────────┘
                   ▼                          ▼
              deploy cash              sell / trim positions
                   └──────────► NAV = idle + holdings + frozen ◄── track-only
                                       (frozen = real but un-tradeable)
```

- **ValueUsd** newtype: the _only_ way to make a dollar value is `mark(qty, price)`
  — no `f64`, no stale percentages (INV-2).
- A holding is valued **only on the chain its sell executes on**, and **only if
  it's sellable** → valuing-it and selling-it are one decision → **no phantom sells**.

---

## 3. The execution saga (money is never stranded)

A cross-chain move is 3 legs. Each leg walks a model-checked FSM; the persisted
`leg_state` says **where the funds physically are**:

```
 Pending → Submitted ─┬─ local swap ───────────────► Confirmed        (target asset)
                      │
                      └─ burn ─► BridgeInFlight ─► mint ─► BridgeLanded (USDC on dest)
                                                              │
                                            acquire ──────────┼──► Confirmed
                                            acquire fails ────┴──► StrandedReserve (USDC)
                                            forwarder refund ────► CompensatedToUsdc (USDC)
```

**Fund-Safety theorem (model-checked + forge-tested):** every _terminal_ state
leaves funds as the **target asset**, as **USDC**, or **unmoved** — never stuck
mid-flight or in a junk token.

---

## 4. Invariants (the guard-rails)

| INV | Rule                                               | Enforced by                     |
| --- | -------------------------------------------------- | ------------------------------- |
| 1   | Value has one source: the live wallet              | `build_plan_input` valuation    |
| 2   | A dollar value only comes from `mark(qty,price)`   | `ValueUsd` newtype              |
| 3   | Target & current share one base → no phantom sells | planner test                    |
| 4   | Executable = a live route exists now               | `is_executable` (one authority) |
| 6   | A plan is bound to the routability it was built on | `RoutableSnapshot` hash         |
| 7   | One unforgeable execution seal per leg             | `ExecutionTicket::mint`         |
| 8   | You can only spend `balance − reserved`            | reservation ledger              |

(INV-5 = simulate-before-approve, designed, not yet wired.)

---

## 5. Determinism & safety anchors

- **RoutableSnapshot** — a fingerprint of "what can route right now," stamped on
  the plan at build time. If a rail flips Ready⇄track-only before you approve,
  approval **refuses** (INV-6). Auto-pilot stamps it too.
- **Reservations** — a 2nd concurrent plan sees `settleable = balance − what
in-flight plans already committed`, so two plans can't spend the same USDC (INV-8).
- **Deferred targets** — a sleeve with no live route isn't silently folded into
  USDC; it's returned as _intent_ (`PartialDeferred`/`Blocked`) and shown in review.

---

## 6. Where it lives

```
apps/api/src/
  domain/rebalance/value.rs ......... ValueUsd (INV-2)
  modules/rebalance/
    handlers/outcome.rs ............. PlanOutcome union (no 409)
    handlers/plan_input.rs .......... valuation, sell-side, deferred, reservations
    snapshot.rs ..................... RoutableSnapshot (INV-6)
    reservations.rs ................. settleable = balance − reserved (INV-8)
    planner.rs ...................... weights → legs (sells first)
    executor/
      mod.rs ........................ walk_legs (the saga loop)
      dispatch.rs ................... how ONE leg executes
      leg_state.rs .................. the FSM + fund-safety model-check
      leg_status.rs ................. persist state + SSE per transition
infra/contracts/ .................... RebalanceExecutor.sol (on-chain refund proof)
apps/web/src/
  lib/api.ts ........................ PlanOutcome / leg types
  components/rebalance/ ............. approval modal + live execution trace
```

**Flags:** `EXECUTION_MOCK`/`MOCK_CIRCLE` real-by-default; `default` cargo build
includes `real-cctp`/`real-usyc`/`real-swap`, so dev runs the real adapters.
