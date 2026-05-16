# 05 — Open questions

> **The honest list of what we haven't solved.** Some are research problems, some are scope choices we deferred, some are things the hackathon timebox forced us to leave loose.

## 🚨 PRE-DEPLOY BLOCKER — Rotate OpenRouter API key

**Tag:** `PRE-DEPLOY-ROTATE-1` · **Status:** open · **Surfaced:** 2026-05-16

The `OPENROUTER_API_KEY` value currently in `.env.local` was previously committed to `origin/main` inside `.env.bak` (commit `2d768d7`, since scrubbed via `git filter-repo` on 2026-05-16 and force-pushed). The key value is therefore considered **publicly leaked** even though the file is gone from current history — GitHub's orphan-object retention is ~90 days, and any clone or fork made before the force-push retains the secret.

**Action required (last thing to do before any production deploy or first paid user):**

1. Sign in at [openrouter.ai/keys](https://openrouter.ai/keys).
2. **Revoke** the 73-char key whose SHA-256 prefix is `bd1497c3461d` (the OpenRouter dashboard shows the key id + creation date — the one created before 2026-05-16).
3. **Generate a replacement** key.
4. Update `OPENROUTER_API_KEY=...` in your local `.env.local` (never in committed `.env`).
5. Restart the API and verify one decision flows through (any non-empty response from `/agent/analyze`).
6. Delete this section from the doc once the rotation is confirmed.

The matching `JWT_SECRET` (which was also leaked in the same commit) has already been rotated locally as part of `F-ENV-1`. Any active session JWTs issued before that rotation are now invalid — users will be forced to re-login at next request.

Note: scrubbing history via filter-repo cleaned `origin/main`, but the historical `feat/sprint-*` branches and `fix/post-submission-audit` on origin still contain `2d768d7` in their git ancestry. If those branches are not needed, delete them on origin to fully purge. They're stale/merged already.

## CCTP V2 contract surface — RESOLVED 2026-05-16

**Tag:** `F-CCTP-1` (with `F-IRIS-1`) · **Status:** resolved · **Surfaced:** 2026-05-16 · **Closed:** 2026-05-16

The codebase shipped a CCTP V1 `sol!` interface (`depositForBurnWithCaller`, 5-arg + 6-arg hook) but the deployed TokenMessenger at `0x8FE6B999…2DAA` on Base Sepolia implements CCTP V2, whose signatures take two additional parameters (`maxFee`, `minFinalityThreshold`):

- `depositForBurn(amount, domain, recipient, token, caller, maxFee, minFinalityThreshold)` — selector `0x8e0250ee`
- `depositForBurnWithHook(amount, domain, recipient, token, caller, maxFee, minFinalityThreshold, hookData)` — selector `0x779b432d`

Verified live via `cast call` — invoking `depositForBurnWithHook` with empty hook data returned the function-body revert `"Hook data is empty"`, proving the V2 selector resolves on the deployed contract.

After rewriting `sol!` to V2 signatures, the first real CCTP V2 burn landed on Base Sepolia:

```
tx_hash:  0xc713c87b7d8d9a697a8f023aead65338131494277a3b6096a852df8b78825395
iris V2:  cctpVersion=2, status=pending_confirmations, delayReason=null
```

Three sub-fixes shipped together (also closes `F-IRIS-1`):

1. `sol!` interface rewritten to CCTP V2 `depositForBurn` + `depositForBurnWithHook`.
2. `wait_for_attestation` switched from `/v1/attestations/{messageHash}` to `/v2/messages/{srcDomain}?transactionHash={txHash}`. The V2 envelope is `{messages: [{ attestation, message, status, delayReason, … }]}`; lookups are by source-domain + burn tx hash.
3. USDC `approve(token_messenger, 2 * amount)` — razor-thin approves caused `"transfer amount exceeds allowance"` reverts even with `maxFee = 0`, likely due to V2's internal `transferFrom(amount + fee)` semantics. Approving with headroom avoids the race.

**Standard vs Fast Transfer**: Fast (`minFinalityThreshold = 1000`) requires a non-zero `maxFee` or iris responds with `delayReason = "insufficient_fee"`. Standard (`2000`) is free but waits for hard finality (~13 min on Base Sepolia). We ship Standard; Fast can be enabled later by funding a fee budget.

**Remaining orchestration follow-up**: smoke binary's poll timeout was 240s, which dropped the in-process executor task before the 13-minute Standard finality cleared. Bumped to `cctp_attestation_timeout_secs + 60s` so end-to-end runs to completion. For the production API server this is moot — `scheduler::spawn_outcome_compressor` re-polls open rebalances on boot.

## Circle Wallets API path staleness

**Tag:** `F-WALLET-1` · **Status:** open · **Surfaced:** 2026-05-16 (N0.9 smoke)

Smoke against the real Circle API on 2026-05-16 with `CIRCLE_API_KEY=TEST_API_KEY:…` showed:

- TLS + DNS to `https://api-sandbox.circle.com` works.
- `Bearer {api_key}` auth header format is correct (no 401/403 returned).
- The specific paths `apps/api/src/modules/wallet/provider.rs` builds — `/v1/wallets/otp/start`, `/v1/health`, `/v1/ping` — all return Circle's structured 404 `{code:-1, message:"Resource not found"}`.

**Likely cause**: Circle has reorganized Wallets-API endpoints between the v1 Programmable Wallets surface (which the codebase references) and the current Developer-Controlled-Wallets v1/w3s surface. The CircleProvider needs a path audit against the live Circle docs before the next signup flow ships.

**Action** (do during N15 browser smoke, OR earlier if first signup attempt 404s): cross-reference each call site in `apps/api/src/modules/wallet/provider.rs` against `developers.circle.com/w3s` and update path strings. Tests will still pass under `MOCK_CIRCLE=true` so no regression risk locally.

## Budget guard enforcement (call-time downshift)

**Tag:** `F-COST-2` · **Status:** open · **Surfaced:** 2026-05-16 by F-COST-1

F-COST-1 (the DeepSeek price-cliff defuse) added `OPENROUTER_BUDGET_GUARD_USD` (default `$0.05`) to `Config` but did NOT enforce it at call time — the value is read at boot and the field is plumbed through, but no code consults it yet. The TODO is marked at `apps/api/src/config.rs::openrouter_budget_guard_usd` in the doc comment.

**What's missing**: in `apps/api/src/modules/agent/service.rs`, after each `OpenRouterClient::chat` returns, compute the call's cost-USD (OpenRouter returns `usage.cost` in the response — needs plumbing through `ChatToolResult`) and compare to the guard. When exceeded, route the next call in the same decision to a cheaper Haiku tier and persist a `tier_features.downshifted: true` marker in the decision metadata so the UI can surface it.

Cheap escape valve: if enforcement is too risky, just log a `warn!` for now and ticket a dashboard alert. The guard config itself is already valuable as an operator signal.

## Regime classifier accuracy

The classifier turns three statistical inputs (BTC 30d realized vol, 90d cross-asset correlation, max drawdown) into a 1-of-3 label via a small Haiku call. We have no out-of-sample validation. In a longer build we'd:

- Backtest the classifier on the last 5 years of crypto and SPX data and report precision/recall per regime.
- Compare the LLM-final-step against a pure-statistical baseline (e.g., a hidden Markov model on the same features).
- Track classifier outputs over time in production and surface confidence drift.

For the hackathon, we accept the risk and tag every decision with the regime that produced it so users can audit downstream.

## Tax-lot edge cases

Cost-basis tracking is naïve FIFO. We don't model:

- Wash-sale rules (US 30-day window). Crypto isn't a security under current US law, but the rule is contested and may apply.
- Per-lot identification by the user.
- Basis adjustments from token migrations or hard forks.

The tax-loss harvester is best-effort and clearly labeled as such in the UI. Real tax software it is not.

## MEV on cross-chain Hooks

CCTP V2 Hooks execute a destination-chain swap atomically with the burn-mint. The swap target (the DEX router we point to) is not MEV-protected. A large proposal could be sandwiched. Mitigations we considered but didn't ship:

- Routing through a private mempool (Flashbots Protect) on Base.
- Splitting large rebalances into smaller hops and randomizing timing.
- Using a CoW-style batch auction.

For now, we cap single-Hook swap size and surface estimated slippage in the proposal.

## Multi-user agent memory

Each portfolio's memory is private and shallow (last 5 decisions). We don't yet:

- Aggregate anonymized signals across users to improve regime calls.
- Let an agent learn from other portfolios' outcomes.
- Differentiate "the user reverted this decision" from "the market reverted this decision."

The strategy marketplace (if it ships) is the closest thing to cross-user learning we have planned.

## OpenRouter routing stability

Per-task model routing is great when models behave consistently. It's a problem when a provider's model shifts under the same slug, or when OpenRouter's failover hands us a model we didn't pick. We persist `model_slug` and raw response on every decision so we can detect shifts after the fact, but we don't have an automated "regression" alarm.

## Arc testnet fragility

We're settling on Arc _testnet_. The chain itself is solid; the surrounding ecosystem (DEX liquidity, oracle freshness, tooling) is not yet what it will be at mainnet. Cross-chain Hook swaps depend on a destination-chain router that exists on Base and Arc — both should be there for the demo, but if Arc testnet liquidity is thin on demo day, we fall back to Base-only execution.

## "Agentic sophistication" is judged, not measured

There's no objective metric for "how much the AI decided versus automated." Our talking points: per-task model routing, regime-conditioned reasoning, adversarial critic pass, tool-using strategist, confidence-with-abstain, proactive scheduler. Whether judges weight these the way we do is unknown.

## What we'd build next

- A statistical regime model (HMM) with the LLM as a tiebreaker.
- Real strategy backtests against historical price data, displayed inline on every proposal.
- Multi-portfolio support per user (treasury, retirement, speculative — different risk goals).
- Notification preferences (email, Telegram) on a per-event-type basis.
- A plain-English audit log a non-technical user could give to their accountant.

---

> **What this enables:** an honest conversation with judges about what the agent does well and what it doesn't.
>
> **What it doesn't:** any of the above. We're shipping the harness, not a finished product.
