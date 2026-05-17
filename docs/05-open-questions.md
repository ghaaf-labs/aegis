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

## CCTP V2 destinationCaller routing — RESOLVED 2026-05-17

**Tag:** `F-CCTP-5` · **Status:** resolved · **Surfaced:** 2026-05-17 (HS-4 re-smoke) · **Closed:** 2026-05-17

The CCTP V2 burn in `cross_chain.rs::real_deposit_for_burn` set `destinationCaller = executor_on_dest` (the RebalanceExecutor contract address on the destination chain). Per Circle's V2 spec, `destinationCaller != bytes32(0)` restricts who may call `MessageTransmitter.receiveMessage` on the destination chain to that specific address. Our `RebalanceExecutor` (`infra/contracts/src/RebalanceExecutor.sol`) implements `IMessageHandlerV2.handleReceiveMessage` (the downstream hook) but exposes no function that forwards into `MessageTransmitter.receiveMessage`. The message is therefore unmintable from any EOA-driven flow — including our `n6_cctp_resume` binary.

Result on the third HS-4 attempt: burn `0x16b04e1…3cda5` succeeded, iris attestation completed (`status: complete`), `receiveMessage` from our EOA reverted with `Invalid caller for message`. 5 USDC stuck on Base side.

**Fix**: `destinationCaller = bytes32(0)` so any address may relay. The hook body (mint recipient + swap params) is baked into the message at burn time and cannot be manipulated by the relayer, so unrestricted relay preserves the same end-state guarantees. Future burns produced by this code path land mintably from any caller. Committed alongside this F-CCTP-5 entry.

**Followup `F-CCTP-6` (parked)**: in a production sprint, add a `relay()` wrapper to `RebalanceExecutor.sol` that calls `MessageTransmitter.receiveMessage` and re-enable `destinationCaller = address(this)` so only the executor contract can submit. That tightens the trust surface (no MEV-style replay of the same hook with attacker-controlled gas/timing) at the cost of an extra deploy step.

## USYC Teller allowlist gate (`NotPermissioned`)

**Tag:** `F-USYC-1` · **Status:** open · **Surfaced:** 2026-05-17 (HS-5 smoke)

`usyc_park_smoke --amount 5` against Arc testnet (with `--features real-usyc EXECUTION_MOCK=false`) reverts with `NotPermissioned()` (selector `0x7f63bd0f`). The Hashnote USYC Teller at `0x9fdF14c5B14173D74C08Af27AebFf39240dC105A` enforces a contract-level depositor allowlist — expected for an institutional T-Bill product. Aegis's hackathon EOA `0xf22C…aa24` isn't on the list.

The Rust side is proven correct by this revert: `real-usyc` feature compiles against the deployed Teller, `treasury::service::park_in_usyc` plumbing reaches the chain, the alloy call surfaces the raw selector as a clean `AppError::Internal`. The blocker is purely credentialing.

**Action**: submit a Hashnote testnet allowlist request for the Aegis EOA. Re-run `usyc_park_smoke` once the allowlist update lands. Until then:

- The `MOCK_CIRCLE=true` path keeps the UI demo end-to-end.
- The strategist's USYC sleeve surfaces a "pending custodian onboarding" caveat in approval modals when `real-usyc` is enabled (see `apps/api/src/modules/treasury/service.rs::park_in_usyc` — currently it bails on revert; a future iteration could surface the caveat upstream and degrade to a USDC-only plan instead of failing the whole rebalance).

## Arc CCTP V2 domain id — RESOLVED 2026-05-17

**Tag:** `F-CCTP-2` · **Status:** resolved · **Surfaced:** 2026-05-17 (HS-4 smoke) · **Closed:** 2026-05-17

`ChainKey::Arc.domain_id()` returned `13` (and `CHAIN_DOMAINS.arc = 13` in `packages/shared/src/constants.ts`), but Arc testnet's deployed CCTP V2 MessageTransmitter at `0xE737e5cEBEEBa77EFE34D4aa090756590b1CE275` returns `localDomain() = 26`. A real burn submitted from Base Sepolia embedded `destinationDomain = 13`; the iris attestation completed but `receiveMessage` on Arc reverted with `Invalid destination domain`. Per Circle's V2 registry, domain 13 is OP Mainnet, not Arc — the attested message could never have landed on Arc.

The bug passed CI because:

- No on-chain integration test asserts `MessageTransmitter.localDomain() == ChainKey::*.domain_id()` for each chain.
- Mock-mode `wait_for_attestation` + `receive_message` never invoke the destination-chain transmitter.
- The single in-process round-trip test uses `EXECUTION_MOCK=true` and a hand-crafted mock attestation.

Fix lands in `apps/api/src/modules/rebalance/models.rs` (`Self::Arc => 26`) plus `packages/shared/src/constants.ts` (`arc: 26`), verified against the on-chain `localDomain()` query before commit. The 20 USDC burned with the stale domain stays burned on Base — `0x6579f80402d8c6ba2022a19f7ab8edc0ce2523518ec2f8814702ff019fc96e36` is the sunk-cost evidence that the CCTP V2 sol! interface + deployed TokenMessenger work end-to-end on the burn side.

**Followup `F-CCTP-3` (parked)**: add a one-time boot-time check that compares `MessageTransmitter.localDomain()` for each configured chain against `ChainKey::*.domain_id()` and fails fast on mismatch. The check needs RPC access at boot, which conflicts with the current `EXECUTION_MOCK=true` default — gate it behind `--features real-cctp` and only run when `execution_mock == false`.

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

## Circle Wallets API path staleness — RESOLVED 2026-05-17

**Tag:** `F-WALLET-1` · **Status:** resolved · **Surfaced:** 2026-05-16 (N0.9 smoke) · **Closed:** 2026-05-17 (backend-platform-usable)

The earlier 401/404 storm was a host bug, not a permissions bug: `.env` was pointing `CIRCLE_BASE_URL` at `https://api-sandbox.circle.com` (the legacy Payments host) while the W3S Programmable-Wallets product lives at `https://api.circle.com`. Per Circle docs, sandbox vs production is keyed by the `TEST_API_KEY:` / `LIVE_API_KEY:` prefix on the API key — not by separate hosts.

Verification: live `GET /v1/w3s/{config/entity,users,wallets}` against `api.circle.com` with the existing `TEST_API_KEY:cd732deb...` returned **HTTP 200** on all three. The `tests/live_circle_w3s.rs` smoke encodes this check (`#[ignore]` by default; `cargo test --test live_circle_w3s -- --ignored`).

Implementation:

- Flipped default `circle_base_url` to `https://api.circle.com` in `apps/api/src/config.rs` and updated `.env`.
- Rewrote `apps/api/src/modules/wallet/provider.rs` for the W3S User-Controlled flow — `ensure_user` → `issue_user_token` → `fetch_user_wallets`. `UserTokenBundle` returned to the browser carries `userToken + encryptionKey + appId + challengeId` for `@circle-fin/w3s-pw-web-sdk` to consume.
- Deleted the legacy OTP routes (`/auth/wallet/otp/*`) and added `GET /auth/wallet/status` for the browser to poll after the SDK completes the PIN challenge.
- Frontend `apps/web/src/components/wallet/create-wallet-card.tsx` now dynamically imports `W3SSdk`, runs the challenge in the browser, and polls `/auth/wallet/status` until both ARC and BASE addresses are provisioned.
- New `CIRCLE_APP_ID` env required when `MOCK_CIRCLE=false` (validated at boot).

## FX live with CoinGecko fallback — RESOLVED 2026-05-17

**Tag:** `F-FX-1` · **Status:** resolved · **Surfaced:** 2026-05-16 · **Closed:** 2026-05-17 (HS-6)

`fx::service::usdc_eurc_basis` is no longer a hardcoded 0.9217. The default path hits CoinGecko `/api/v3/simple/price?ids=usd-coin,euro-coin&vs_currencies=usd`, derives the mid rate from `usdc_usd / eurc_usd`, and rounds to 4 decimals. 30s in-memory cache fits comfortably under CoinGecko's free-tier ceiling. Any error degrades to the prior steady 0.9217 with `source: "coingecko-fallback"` so the agent prompt always has a number.

New env `STABLEFX_INSTITUTIONAL_ACCESS=false` (default false) carries the flag for the future RFQ-first path — institutional StableFX access is still KYB-gated and not yet open. When flipped, the service logs a debug line and still falls through to CoinGecko because the RFQ wire hasn't landed.

Frontend: approval modal renders a warn-toned caveat banner whenever any leg has `srcSymbol == "EURC" || destSymbol == "EURC"` so users see the institutional-pending posture before approving.

## Budget guard enforcement (call-time warn) — RESOLVED 2026-05-17

**Tag:** `F-COST-2` · **Status:** resolved · **Surfaced:** 2026-05-16 by F-COST-1 · **Closed:** 2026-05-17

Took the warn-path escape valve. `apps/api/src/modules/ai/client.rs::check_budget_guard` runs after every successful OpenRouter completion (both `chat()` and `chat_with_tools()`):

- `usage.cost` is parsed from the OpenRouter response (`Option<f64>`; absent on free routes treated as zero).
- When `cost > config.openrouter_budget_guard_usd`, a structured `tracing::warn!` with `target: "agent.cost.guard_exceeded"` fires carrying `model_slug`, `cost_usd`, `guard_usd`, `latency_ms`. Operator-side alerting can subscribe to that target without any further code change.
- `ChatResponse.cost_usd` + `ChatToolResult::{Final, Calls}.cost_usd` carry the per-call cost forward so per-decision telemetry can aggregate it later (e.g. a future "spend per decision" UI surface) without re-fetching.

Auto-downshift mid-decision (the originally-scoped behavior) was de-scoped: warn-then-watch is the cheap path and the dashboard alert is sufficient until decision volume justifies the complexity. Re-open the ticket if production traffic produces a sustained-overage pattern.

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
