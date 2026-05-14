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
