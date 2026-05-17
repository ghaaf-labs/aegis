# 11 — Agent Outcome Policy

The agent makes decisions that move real money. This page sets explicit, plain-English expectations for what happens when those decisions go wrong, how a user gets help, and how a user takes back control.

If you only read one line: **we refund protocol fees on agent-caused failure, never on market losses, and any user can pause the agent in one click.**

## 1. What we'll refund

| Situation                                                                                   | Refund posture                                                              |
| ------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| Rebalance failed mid-execution (CCTP attestation timeout, RPC outage, on-chain revert).     | **Full protocol-fee refund.** No service was delivered, so no fee is owed.  |
| Rebalance succeeded but the post-trade allocation lost value due to market movement.        | **No refund.** Market risk is the user's; the agent was asked to act.       |
| Rebalance succeeded but the agent's recommendation was off-policy (constitution violation). | **Full protocol-fee refund** plus a written explanation of the violation.   |
| Cross-chain leg landed but a downstream Park/Redeem couldn't complete because of liquidity. | **Pro-rata refund** for the legs that didn't execute. Confirmed legs stand. |
| User changed their mind after approving and before the chain confirmed.                     | **Full refund** if the first leg has not yet settled. Otherwise no refund.  |
| Test-mode (`EXECUTION_MOCK=true`) or pre-paid-tier traffic.                                 | No fees charged; no refund applies.                                         |

Refunds settle on-chain via the same Nanopayments rail that collected the fee. They are visible in [`docs/03-circle-stack.md`](./03-circle-stack.md) §Nanopayments and in `billing_events.facilitator_status`.

## 2. What we won't do

Beyond the explicit anti-features in [CLAUDE.md](../CLAUDE.md):

- **We don't refund asset losses.** Aegis is an agent over your portfolio — it does not insure or guarantee returns.
- **We don't charge for failed executions.** A reverted rebalance is a failure of our service, not a billable event.
- **We don't take a hidden swap spread.** All fees are surfaced in the approval modal; the on-chain trace shows every leg.
- **We don't require KYC at signup.** Identity verification only enters the flow at a future fiat off-ramp event, with clear consent.
- **We don't move money without your approval modal.** Every rebalance shows the legs, the fee, and the constitution-clauses applied before execution. Auto-execute (for peg-defense and similar) is opt-in per rule and tier-gated.

## 3. How a user takes back control

- **Pause the agent** — `/settings` → **Pause agent**. One toggle. Stops every scheduled trigger (drift watcher, regime flip, peg monitor) immediately. The agent never trades while paused.
- **Cancel an in-flight rebalance** — pre-approval is a no-op (no chain state changed). Once approved, individual legs can be left to complete; pending legs that haven't yet broadcast will not be retried.
- **Withdraw funds** — Aegis is non-custodial. Your USDC sits in your own modular smart account wallet ([Circle Wallets](https://developers.circle.com/wallets)); we never custody it. Withdraw at any time from the wallet UI.
- **Delete your account** — `/settings` → **Delete account**. Drops your portfolio, allocations, decisions, and PII. On-chain history remains on-chain (we cannot rewrite it).

## 4. Dispute escalation

In order:

1. **Self-service trace** — every decision has a public page at `/decision/<id>` showing the model, prompt tokens, latency, constitution clauses evaluated, and the resulting plan with on-chain tx hashes. Most "what happened?" questions answer themselves here.
2. **Email support** — `support@aegis.local` (placeholder; will be set on the production domain at first paid user). Reply within 1 business day. Include the rebalance UUID or the decision page URL.
3. **Operator escalation** — if a support reply doesn't resolve the issue within 5 business days, the case is reviewed manually by the operator on call and either refunded under §1 or denied with a written rationale.

We do not yet have a regulator-recognized arbitration path. This will change when we onboard a first design partner with explicit compliance scope (see [`docs/06-traction.md`](./06-traction.md)).

## 5. Constitution clauses

The agent's hard constraints are versioned and published at [`/about/constitution`](https://aegis.local/about/constitution). Every critic veto cites a clause ID. The full policy is in [`docs/02-agent-design.md`](./02-agent-design.md) §Constitution.

A short summary of the categories:

- **Concentration limits** — no single asset may exceed a per-tier ceiling.
- **Volatility floor / ceiling** — portfolio target volatility kept inside the goal-stated band.
- **Drift threshold** — no rebalance proposed below the configured drift (default 5%).
- **Wash sale guard** — tax-loss harvest won't trigger a buy of a substantially identical asset within 30 days.
- **Peg defense** — depegged-asset weight rotates into the rule's `target_asset` automatically only for Pro/Business tiers with pre-approved rules. Otherwise an alert fires and waits for user approval. **`PEG_DEFENSE_ENABLED=true` is default-on as of 2026-05-17 (FF-3)**; configure rules at `/settings/peg`. The Pro/Business auto-execute path (`F-PEG-8`) remains gated until the tier integration test lands.

## 6. Why this page exists

Per [`docs/05-open-questions.md`](./05-open-questions.md), the first operational break at scale is the refund / dispute conversation, not engineering. This document is the operational floor. It is intentionally short so it can stay correct. When we have a real lawyer-reviewed terms of service, this page links to that as the controlling document; until then this page IS the controlling document for outcome handling.

Last reviewed: 2026-05-16.
