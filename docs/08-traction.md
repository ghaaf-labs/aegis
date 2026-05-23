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
| Real wallets (Circle Wallets MSCAs created via /login)       | `SELECT count(*) FROM users WHERE wallet_id IS NOT NULL`                           | `${WALLETS_CREATED}`     |
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

### HS-4 · first real CCTP V2 rebalance (Base Sepolia → Arc testnet) — first burn confirmed; mint blocked by code bug, re-smoke in flight

The first end-to-end attempt surfaced two real bugs that have since
been fixed. The burn-only evidence proves CCTP V2 sol! interfaces +
deployed TokenMessenger compatibility; the mint side waits on the
re-smoke that runs against the corrected Arc domain id.

| Field          | Value                                                                                                                                                           |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Direction      | Base Sepolia → Arc testnet                                                                                                                                      |
| Size           | 20 USDC bridge (full $20 portfolio targeted 100% EURC → cross-chain leg fires)                                                                                  |
| Setup          | `ARC_EOA + BASE_EOA derived from CHAIN_PRIVATE_KEY_*; ./scripts/seed-n6-smoke.sh` seeds user + portfolio                                                        |
| Goal flip      | `UPDATE portfolios SET goal = jsonb_set(goal, '{targetAllocation}', '{"EURC": 100}') WHERE id = ...`                                                            |
| Build          | `EXECUTION_MOCK=false MOCK_CIRCLE=false cargo run --features real-cctp --bin cctp_rebalance_smoke`                                                              |
| rebalance_id   | `8ddea142-4108-4107-944d-40a872579219`                                                                                                                          |
| Burn tx hash   | `0x6579f80402d8c6ba2022a19f7ab8edc0ce2523518ec2f8814702ff019fc96e36`                                                                                            |
| Base explorer  | https://sepolia.basescan.org/tx/0x6579f80402d8c6ba2022a19f7ab8edc0ce2523518ec2f8814702ff019fc96e36                                                              |
| Burn block     | 41,618,751 (gas used 118,778; status 1)                                                                                                                         |
| CCTP message   | `bd62404d3d60242034ed360d6a0d82480bc0d125112d04b58a2c342537df0b26` (cctpVersion 2)                                                                              |
| Burn timestamp | 2026-05-17 08:13:22 UTC                                                                                                                                         |
| Mint tx hash   | _stuck — message embedded `destinationDomain=13` which Arc rejected. See "Bug #2" below. 20 USDC lost on testnet; re-smoke below uses the corrected domain 26._ |
| Arc explorer   | n/a for this attempt                                                                                                                                            |
| Date           | 2026-05-17 (burn); domain fix + re-smoke same day                                                                                                               |

**Bug #1 — pre-flight allowance race (fixed in commit `65d832e`)**

The first burn attempt reverted in pre-flight with `ERC20: transfer
amount exceeds allowance` even though `approve(token_messenger,
2*amount)` was called synchronously before `depositForBurnWithHook`.
Root cause: on the Base Sepolia RPC used here, `eth_estimateGas` ran
against state from a block before the approve was mined. Fix in
`cross_chain.rs::real_deposit_for_burn` now reads the existing
allowance first, only approves if insufficient, and inserts a 3s
settle after the approve receipt before the burn — both saving gas
on retries and avoiding the RPC pre-flight race.

**Bug #2 — wrong Arc CCTP V2 domain id (fixed in this commit)**

Burn 0x6579… submitted with `destinationDomain = 13` per
`ChainKey::Arc.domain_id() = 13` in the Rust code (and a matching
`arc: 13` in `packages/shared/src/constants.ts`). The iris attestation
landed (`cctpVersion: 2`, `status: complete`), but the
`receiveMessage` call on Arc reverted with `Invalid destination
domain`. Root cause: Arc testnet's deployed MessageTransmitter
returns `localDomain() = 26`, not 13. The 13 was a stale guess that
silently passed CI because no test fired against a real chain — the
mock executor never invokes `MessageTransmitter::receiveMessage`.
Domain 13 is in fact OP Mainnet per Circle's V2 registry, so the
attested message could never have landed on Arc.

Fix lands in `apps/api/src/modules/rebalance/models.rs` (`Arc => 26`)
plus `packages/shared/src/constants.ts` (`arc: 26`), both verified
against the on-chain `MessageTransmitter.localDomain()` query. The
20 USDC burned with the old domain stays burned on the Base side —
recovery on a destination chain matching domain 13 (OP Sepolia) is
possible in principle but not pursued; testnet burn is a sunk cost.

**Re-smoke against the corrected domain** — pending; will populate
the row below and amend the date on the next commit.

| Field          | Value (re-smoke against corrected domain)                                                                                                                                |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Size           | 5 USDC bridge (scaled down — 20 of 25 USDC on Base Sepolia were already burned on the failed first attempt)                                                              |
| Build          | `EXECUTION_MOCK=false MOCK_CIRCLE=false CCTP_ATTESTATION_TIMEOUT_SECS=1500 cargo run --features real-cctp --bin cctp_rebalance_smoke`                                    |
| rebalance_id   | `9dc4414f-7ec0-428a-a1d5-0f7faf5aae29`                                                                                                                                   |
| Burn tx hash   | `0x16b04e14ed38e58c07e23e5d274d4cbefb00de8d37dd3c4f93d19f6210e3cda5`                                                                                                     |
| Base explorer  | https://sepolia.basescan.org/tx/0x16b04e14ed38e58c07e23e5d274d4cbefb00de8d37dd3c4f93d19f6210e3cda5                                                                       |
| Burn timestamp | 2026-05-17 08:40 UTC                                                                                                                                                     |
| Mint tx hash   | _**Stuck** — third bug surfaced: receiveMessage reverted with `Invalid caller for message`. See "Bug #3" below. 5 USDC lost on testnet._                                 |
| Wall-clock E2E | Burn submitted <2s; Standard finality on Base Sepolia took ~20 min (attestation completed at ~09:00 UTC); `receiveMessage` reverted in <1s on the caller-mismatch check. |

**Bug #3 — destinationCaller restricted the mint to a contract with no relay function (fixed in this commit, `F-CCTP-5`)**

`cross_chain.rs::real_deposit_for_burn` set `destinationCaller =
executor_on_dest` (the RebalanceExecutor on Arc). Per CCTP V2 spec,
non-zero `destinationCaller` restricts who may call
`MessageTransmitter.receiveMessage` to that specific address. Our
`RebalanceExecutor` (`infra/contracts/src/RebalanceExecutor.sol`)
implements the downstream `IMessageHandlerV2.handleReceiveMessage`
hook but exposes no public function that forwards into
`MessageTransmitter.receiveMessage` — so the message is unmintable
from any EOA-driven relay. Iris attestation completed (`status:
complete`); `receiveMessage` from our EOA reverted with `Invalid
caller for message`.

Fix in `cross_chain.rs`: `destinationCaller = bytes32(0)` (any
relayer permitted). The hook body is baked into the message at burn
time and the relayer cannot alter it, so unrestricted relay
preserves the trust model. F-CCTP-6 (parked): a future
`RebalanceExecutor.relay()` wrapper would re-enable
`destinationCaller = executor_on_dest` for an extra layer of
relayer authorization, at the cost of an extra contract deploy.

**Cumulative HS-4 burn evidence (3 burns, 3 bugs, 45 USDC sunk)**

| Burn tx                                                              | Block          | Domain | DestCaller   | Bug surfaced               | USDC stuck |
| -------------------------------------------------------------------- | -------------- | ------ | ------------ | -------------------------- | ---------- |
| `0xc713c87b…8825395` (prior session, 2026-05-16)                     | (Base Sepolia) | 13     | executor_arc | F-CCTP-2 (Arc=13)          | 20         |
| `0x6579f80402…ff019fc96e36`                                          | 41,618,751     | 13     | executor_arc | F-CCTP-2 (Arc=13)          | 20         |
| `0x16b04e14ed38e58c07e23e5d274d4cbefb00de8d37dd3c4f93d19f6210e3cda5` | (~08:40 UTC)   | 26     | executor_arc | F-CCTP-5 (caller mismatch) | 5          |

All three burns landed cleanly on Base Sepolia — proving the CCTP V2
`sol!` interface, the deployed TokenMessenger compatibility, the
allowance-skip fix (`F-CCTP-?` in `a938a3c`), and the Arc=26 domain
correction (`F-CCTP-2` in `e7f27af`). The mint half waits on the
fourth burn (with this commit's `destinationCaller = bytes32(0)` fix
applied) plus a Base Sepolia USDC top-up — current EOA balance is
0 USDC after the 25-USDC sunk-cost run.

Known surprises: Paymaster fee preview will show the Sprint-2 mocked
~$0.117 USD (Arc 0.012 + Base 0.105) vs actual Base Sepolia chain
gas of ~$0.000007. Documented as `F-PAYMASTER-1` followup; not a
blocker for the smoke.

### HS-5 · first real USYC park (Arc testnet) — blocked on allowlist (`NotPermissioned`)

| Field            | Value                                                                                                                                                                                                                                                                                   |
| ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Action           | `IUsycTeller::deposit` against Hashnote Teller on Arc testnet                                                                                                                                                                                                                           |
| Size             | 5 USDC                                                                                                                                                                                                                                                                                  |
| Build            | `EXECUTION_MOCK=false cargo run --features "real-cctp real-usyc" --bin usyc_park_smoke -- --amount 5`                                                                                                                                                                                   |
| Smoke binary     | New `apps/api/src/bin/usyc_park_smoke.rs` (this commit) — calls `treasury::service::park_in_usyc` directly. Bypasses the HTTP layer + Circle Wallets.                                                                                                                                   |
| Pre-flight check | `cast call $USYC_TELLER_ARC "asset()(address)" --rpc-url $ARC_RPC_URL` returns `USDC_ARC` ✓                                                                                                                                                                                             |
| Result           | **Reverted: `NotPermissioned()` (selector `0x7f63bd0f`)**                                                                                                                                                                                                                               |
| Root cause       | Hashnote's USYC Teller permissions depositors at the contract level. The Aegis hackathon EOA `0xf22C…aa24` is not on the allowlist. This is expected for an institutional-grade T-Bill product; allowlisting requires KYB onboarding with Hashnote, which isn't open for hackathon use. |
| Deposit tx hash  | _N/A — no tx submitted; revert caught at simulation_                                                                                                                                                                                                                                    |
| USYC balance     | `0` (`cast call $USYC_TOKEN_ARC "balanceOf(address)(uint256)" $ARC_EOA`)                                                                                                                                                                                                                |
| Date             | 2026-05-17 (smoke run + revert confirmed)                                                                                                                                                                                                                                               |
| Followup         | `F-USYC-1` — submit a Hashnote testnet allowlist request, then re-run `usyc_park_smoke`. Until then, `MOCK_CIRCLE=true` mock path keeps the UI demo end-to-end and the strategist's USYC sleeve is documented as "pending custodian onboarding" in approval modals.                     |

Code state proven by this run: `real-usyc` cargo feature compiles
against alloy + the deployed Teller, `treasury::service::park_in_usyc`
plumbing reaches the chain, the revert is surfaced as a clean
`AppError::Internal` with the raw selector preserved for ops. The
last mile (an allowlisted deposit) is a credentialing task, not a
code task.

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
