# Aegis · Traction

**RFB 04 judging:** Traction is 30% of the score. We treat this as a hard
constraint: **real users, in the event window, with real (testnet) USDC
moving on chain**.

This document is the running ledger we cite in the submission. Numbers come
from straight SQL against our self-hosted Postgres — no PostHog, no
third-party analytics. The queries that produce each number live in
`apps/api/src/modules/analytics/queries.sql` (and are re-runnable at
submission time).

## Submission numbers

> **Update before final submission.** These placeholders mirror what the
> Agora form asks for.

| Metric                                                       | Source                                                                             | Value                    |
| ------------------------------------------------------------ | ---------------------------------------------------------------------------------- | ------------------------ |
| Real wallets (Circle Wallets MSCAs created via /signup)      | `SELECT count(*) FROM users WHERE wallet_id IS NOT NULL`                           | `${WALLETS_CREATED}`     |
| Decisions executed (non-abstain)                             | `SELECT count(*) FROM agent_decisions WHERE triggered_by != 'abstain'`             | `${DECISIONS_EXECUTED}`  |
| Distinct portfolios                                          | `SELECT count(*) FROM portfolios`                                                  | `${PORTFOLIOS}`          |
| Total USDC routed through executor (testnet)                 | `SELECT sum(amount_usdc) FROM rebalance_legs WHERE status = 'confirmed'`           | `${TESTNET_USDC_ROUTED}` |
| Daily digest subscribers                                     | `SELECT count(*) FROM digest_subscriptions`                                        | `${DIGEST_SUBSCRIBERS}`  |
| Referrals credited                                           | `SELECT count(*) FROM referrals WHERE paid_at IS NOT NULL`                         | `${REFERRALS_CREDITED}`  |
| Models routed (distinct OpenRouter slugs in agent_decisions) | `SELECT count(DISTINCT model_slug) FROM agent_decisions`                           | `${MODELS_ROUTED}`       |
| Chains touched (in confirmed legs)                           | `SELECT count(DISTINCT src_chain) FROM rebalance_legs WHERE src_chain IS NOT NULL` | `${CHAINS_TOUCHED}`      |

## Distribution channels

| Channel                                       | When              | Outcome            |
| --------------------------------------------- | ----------------- | ------------------ |
| Canteen Discord — RFB 04 thread               | Day 7 of Sprint 4 | ${DISCORD_OUTCOME} |
| X / crypto-twitter — 6-tweet thread           | Day 7 of Sprint 4 | ${X_OUTCOME}       |
| Direct DMs to the 20 closest builders we know | Day 8 of Sprint 4 | ${DM_OUTCOME}      |
| `/leaderboard` shareable link in every X post | Continuous        | ${LB_OUTCOME}      |
| Daily-digest opt-in (re-engagement)           | After Day 7       | ${DIGEST_OUTCOME}  |

The `${…}` placeholders get filled in at submission time. The point of
freezing the table here is to make the submission a 30-second update rather
than a scramble.

## Live testnet evidence

> The scaffolding for the first end-to-end real-mode rebalance lands in
> HS-4 / HS-5; the live execution itself is a user-driven smoke that
> requires real testnet USDC + a valid `CIRCLE_API_KEY`. Recipe is
> reproducible from any clean DB.

### HS-4 · first real CCTP V2 rebalance (Base Sepolia → Arc testnet) — pending

| Field            | Value                                                                                                    |
| ---------------- | -------------------------------------------------------------------------------------------------------- |
| Direction        | Base Sepolia → Arc testnet                                                                               |
| Size             | 10 USDC (forces past the planner's $5 dust threshold and the 5% drift threshold)                         |
| Setup            | `DATABASE_URL=… ARC_EOA=0x… BASE_EOA=0x… ./scripts/seed-n6-smoke.sh`                                     |
| JWT mint         | `cargo run --bin forge_test_jwt -- aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa > /tmp/jwt`                      |
| Build            | `cargo run --features real-cctp` with `EXECUTION_MOCK=false MOCK_CIRCLE=false BILLING_V2_ENABLED=false`  |
| Plan endpoint    | `POST /portfolios/bbbbbbbb-…/rebalance/plan` (auth: Bearer JWT)                                          |
| Execute endpoint | `POST /rebalance/<id>/execute`                                                                           |
| Burn tx hash     | _pending — fill in from `rebalance_legs.tx_hash WHERE rebalance_id = ... AND kind = 'cross_chain_burn'`_ |
| Mint tx hash     | _pending — same query, `kind = 'cross_chain_mint'`_                                                      |
| Base explorer    | `https://sepolia.basescan.org/tx/<burn-tx>`                                                              |
| Arc explorer     | `https://testnet.arcscan.app/tx/<mint-tx>`                                                               |
| Wall-clock E2E   | _pending — typical 15-30s per CCTP V2 (3s burn + 8-20s attestation + 2s mint)_                           |
| Date             | _pending_                                                                                                |

Known surprises to call out when filling this in: Paymaster fee preview
will show the Sprint-2 mocked ~$0.117 USD (Arc 0.012 + Base 0.105) vs
actual Base Sepolia chain gas of ~$0.000007. Documented as `F-PAYMASTER-1`
followup; not a blocker for the smoke.

### HS-5 · first real USYC park (Arc testnet) — pending

| Field            | Value                                                                                         |
| ---------------- | --------------------------------------------------------------------------------------------- |
| Action           | `IUsycTeller::deposit` against Hashnote Teller on Arc testnet                                 |
| Size             | 5 USDC                                                                                        |
| Build            | `cargo run --features "real-cctp real-usyc"`                                                  |
| Endpoint         | `POST /portfolios/<id>/treasury/park` `{amountUsdc: 5}`                                       |
| Pre-flight check | `cast call $USYC_TELLER_ARC "asset()(address)" --rpc-url $ARC_RPC_URL` → returns USDC address |
| Deposit tx hash  | _pending_                                                                                     |
| Arc explorer     | `https://testnet.arcscan.app/tx/<deposit-tx>`                                                 |
| USYC balance     | _pending — `cast call $USYC_TOKEN_ARC "balanceOf(address)(uint256)" $ARC_EOA`_                |
| Date             | _pending_                                                                                     |

## Browser smoke walkthrough (N15)

> Run this every time a feature flag flips or a backend module ships. ~15 min
> end-to-end against `EXECUTION_MOCK=false MOCK_CIRCLE=false BILLING_V2_ENABLED=false`
> with a real CCTP-funded testnet wallet (HS-4 / HS-5 scaffolding above).

1. **Boot**: `cargo run --features "real-cctp real-usyc"` from `apps/api/`, then `pnpm dev` from `apps/web/`. Visit `http://localhost:3000`.
2. **Landing → Strategies CTA**: confirm the "Browse strategies" button (SM-4) routes to `/strategies` and the 3 curated cards render.
3. **Sign up**: walk the email-OTP flow (HS-3 path audit gates this). A `users` row should land with both `arc_address` + `base_address` non-NULL.
4. **Adopt a strategy**: click Adopt on `Conservative Treasury`. New portfolio appears; dashboard renders empty-state (no mock leakage per FE-MOCK-1).
5. **Analyze**: click "Run analysis" on the dashboard. Approval modal renders with model slug, confidence bar (cyan per FE-COLOR-1), USDC fee preview, both legs with ChainBadges, EURC caveat banner (HS-6) if EURC is in the plan.
6. **Approve a small rebalance**: confirm two `rebalance_legs.tx_hash` values land + verify on explorers (basescan-sepolia, testnet.arcscan).
7. **Audit trail**: visit `/decision/<id>`. Verify the 5 sections (Inputs, Strategist, Critic, Plan, Execution) all render, model slug + tokens + latency are visible, constitution clauses (if cited) link to `/about/constitution#<id>`.
8. **Regime backtest**: visit `/about/regime/backtest` (FF-1). Chart should render the latest eval samples; empty-state copy fires if no backtest has been run.
9. **Settings → Agent pause**: flip the toggle (FE-PAUSE-1). Watch scheduler logs — drift watcher should skip the user's portfolio while paused. Resume; ticks return.
10. **Settings → Tax export** (FF-2): download CSV. Pre-download confirm modal restates the 1099-DA wallet-by-wallet posture. CSV opens without a 404.
11. **Mobile**: DevTools at 375 × 667. Hamburger drawer (FE-MOB-1) opens, modals reflow (FE-MOB-2), asset table hides columns (FE-MOB-3).
12. **SSE health**: LivePill on the dashboard pulses cyan throughout. Kill the API; pill goes "Offline". Restart; pill returns.

Append screenshots + a Loom recording link to the "Live testnet evidence"
tables above once the walkthrough is captured.

## Design-partner outreach script (N11)

> 8-step guided tour for a design-partner conversation. Walk a candidate
> through these one at a time; pause at each to capture their reaction
> (their words, not yours). Aim for: one comment on agent reasoning,
> one on a trust signal, one suggested improvement.

1. Open `/` → "we let a multi-model agent route real (testnet) USDC across Arc + Base, but every move waits for your tap." (Marketing nav.)
2. Click **Browse strategies** → "three pre-baked portfolios. The agent doesn't pick for you; you pick. The agent does the rebalancing."
3. Adopt **Conservative Treasury** → "now this is yours. Notice: zero USDC moved yet — the agent only sets the target."
4. **Goal wizard** (if not skipped): "this is the only friction. Four steps. We won't ask again."
5. **Analyze** → approval modal. **Pause and let them read the entire modal cold.** "model slug, confidence, USDC fee, both legs, constitution clauses — every trust signal is supposed to be visible without scrolling."
6. **Approve** → execution-trace pane. Capture their reaction to the tx hashes filling in.
7. **Audit trail** → "every decision is a public page. This is how disputes get resolved. Click around."
8. **Pause the agent** → "you're in control. One toggle. Manual rebalances still work. Withdraw any time — your USDC sits in your own Circle Wallet."

After the conversation:

- Capture verbatim quote in the "Quotes from real testers" section below.
- Note any "I wouldn't do X" / "I'd expect Y" — those become the next sprint's followups.
- Ask about willingness to fund a real wallet (even $10) and approve a single rebalance — that's the "real user" definition for the Traction judging.

## Quotes from real testers

> _Three quotes from people outside our team who used Aegis in the event
> window. Capture verbatim, with their handle + role + permission to quote.
> Aim for one quote that praises the agent reasoning, one that praises the
> trust signals (model badge, provenance), and one that suggests something
> we'd actually build next sprint._

1. ${QUOTE_1}
2. ${QUOTE_2}
3. ${QUOTE_3}

## What "real traction" means here

We don't claim AUM. The hackathon settles on testnets — every USDC moved
through the executor is testnet USDC. What we _do_ claim:

- Real Circle Wallets created via passkey or email OTP, by people outside
  our team, who returned for at least one rebalance.
- Real on-chain `MessageSent` + `MessageReceived` events on Arc Sepolia
  and Base Sepolia, with verifiable tx hashes.
- Real `agent_decisions` rows, each with a captured-at-decision price
  snapshot, a critic verdict, and a 24h outcome compressed into agent
  memory.

Every link above resolves to a public URL the judges can click — no demo
videos hiding broken paths.

## Revenue rails

**AUM-fee stream (Pro 25 bps · Business 15 bps).** The Pro and Business
tiers charge an annual AUM fee that streams continuously via Nanopayments
on Arc — the literal pay-per-second metering use case the Nanopayments
demo was built for. Lifecycle: a 24-hour ticker walks every active
subscription, **snapshots** AUM from `portfolios.total_value_usd`,
computes `accrued = aum × bps × Δt / (10_000 × 365.25 × 86400)` in
`Decimal`, and persists an `aum_accruals` row (idempotent on
`(subscription_id, period_start, period_end)`). The row is **rolled up**
into the open invoice for the user's current monthly billing window
(JSONB line item + `subtotal_usdc` bump). At period end the invoice
transitions open → past_due (7-day grace) → **settled** by posting the
total to the same Circle facilitator endpoint the per-rebalance fee
already uses — payer = `users.arc_address`, payTo =
`NANOPAYMENTS_SELLER_ADDRESS`. Gated behind `AUM_STREAM_ENABLED`
(off by default; requires `BILLING_V2_ENABLED`). Sanity: a Pro user with
constant $20k AUM accrues $0.13689/day, $4.107/month.

## How to reproduce the numbers

```bash
# Assumes `kubectl exec` or `docker compose exec` into the postgres pod.
psql -U aegis -d aegis -v ON_ERROR_STOP=1 <<'SQL'
\echo Wallets created:
SELECT count(*) FROM users WHERE wallet_id IS NOT NULL;
\echo Decisions executed:
SELECT count(*) FROM agent_decisions WHERE triggered_by != 'abstain';
\echo USDC routed (testnet):
SELECT round(sum(amount_usdc)::numeric, 2) FROM rebalance_legs WHERE status = 'confirmed';
\echo Models routed:
SELECT count(DISTINCT model_slug) FROM agent_decisions WHERE model_slug IS NOT NULL;
\echo Top 5 leaderboard:
SELECT handle, decisions_executed, round(trustability_delta::numeric, 2) AS delta
  FROM v_trustability_per_user
  ORDER BY trustability_delta DESC NULLS LAST
  LIMIT 5;
SQL
```
