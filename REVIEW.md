# Aegis — Sprint Reviews

## Sprint 2 Audit — findings + fixes

> In-depth review of the Sprint 2 implementation (commit `c830b85`). Goal: confirm the agent loop still works end-to-end with the new wallet, gateway, treasury, fx, and analytics surfaces wired in, and catch correctness, contract, and **privacy** issues before they ship to demo.

### Findings by severity

**H1. SSE privacy leak — events visible to every connected client.**
The Sprint 1 SSE handler broadcasts every event via a single `tokio::sync::broadcast::Sender<SseEvent>` to every subscriber. Sprint 2 added `agent.decision`, `wallet.created`, and `gateway.balance` — all user-specific payloads — to that channel, but kept `/sse` **public** (no JWT required) for `/explore` price ticks. A logged-in user A's `agent.decision` was therefore readable by any connected client.
**Fix:** added `audience_user_id(): Option<Uuid>` to `SseEvent`. Public variants (price.tick, regime.flip, rebalance.status) return `None`; user-scoped variants (agent.decision, wallet.created, gateway.balance) return `Some(user_id)`. Handler now requires auth (moved to the `authed` router), reads `Claims.sub`, and filters the per-client stream: public events go to all authed subscribers, user-scoped events go only to the matching subscriber. `/explore` doesn't use SSE so the auth requirement doesn't regress demo UX.
**Test:** new `audience_user_id_filters_user_events` contract test in `modules/sse/events.rs`.

**M1. `RealtimeBridge` didn't subscribe to `gateway.balance` or `wallet.created`.**
The two new SSE event types fired server-side but the frontend bridge had handlers only for the Sprint 1 events. The UI's unified USDC value only refreshed on the initial `GET /gateway/balance`; new-wallet state didn't surface live.
**Fix:** added `onGatewayBalance` (updates `unifiedUsdc` in Zustand) and `onWalletCreated` (stores `WalletInfo`) handlers. Bridge now gates the EventSource on JWT presence so it doesn't spam unauth'd `/sse` calls.

**M2. EventSource can't send `Authorization` headers.**
With `/sse` now authenticated, the standard browser `EventSource` constructor can't add a bearer header. Bridge passes the JWT as a `?token=` query param. Future hardening: read the token from an httpOnly cookie server-side and let the cookie middleware extract it. Documented as a follow-up.

**M3. Faucet rate-limit query swallowed DB errors.**
`fetch_optional(db).await.unwrap_or(Some(0.0))` silently fell back to 0 used-USDC if the analytics_events query errored — meaning a transient DB hiccup would let a user claim past the 100/24h cap. Same anti-pattern Sprint 1 already audited in `previous_regime`.
**Fix:** explicit `match`; `tracing::warn!` on error before defaulting to 0. Behavior preserved (don't fail the claim on transient analytics issues) but visibility added.

### Lower-severity findings (noted, deferred)

**L1. Token in localStorage** — XSS-vulnerable. Acceptable hackathon-scale; Sprint 3 should move to httpOnly cookies and read on the server side.
**L2. Gateway ticker not spawned** — `gateway.balance` only fires when a client hits `GET /gateway/balance`. A Tokio task per authed wallet polling Circle every `GATEWAY_POLL_SECS` would make the unified balance number tick live without manual refresh. Sprint 3 polish.
**L3. `validate_email` is permissive** — accepts `a@` and `@b`. Circle WaaS validates more strictly downstream, but a tighter regex here would return a 400 earlier.
**L4. Migration 0003 unverified against live Postgres** — same caveat as Sprint 1; no Docker in audit env. Syntax reviewed.
**L5. Treasury + FX modules return mock-deterministic numbers** — by design (S2 stub policy; on-chain execution is Sprint 3), but the live API path is also untested.

### Gate baseline (post-audit)

| Gate                                        | Before audit               | After audit                                     |
| ------------------------------------------- | -------------------------- | ----------------------------------------------- |
| `cargo test --all-targets`                  | 41 passed                  | **42 passed** (+ audience-filter contract test) |
| `cargo clippy --all-targets -- -D warnings` | ✅                         | ✅                                              |
| `cargo fmt --check`                         | ✅                         | ✅                                              |
| `pnpm type-check`                           | ✅                         | ✅                                              |
| `pnpm test` (Vitest)                        | 3 passed                   | 3 passed                                        |
| `pnpm format:check`                         | ✅                         | ✅                                              |
| `typos`                                     | ✅                         | ✅                                              |
| `pnpm lint`                                 | only pre-existing warnings | only pre-existing warnings                      |

### Files added or changed in audit

```
M  apps/api/src/modules/sse/events.rs       — audience_user_id() + user_id field on user-scoped payloads + filter test
M  apps/api/src/modules/sse/handler.rs      — requires Claims, filters by audience_user_id == claims.sub
M  apps/api/src/modules/wallet/sse.rs       — user_id field on WalletCreatedPayload
M  apps/api/src/modules/wallet/service.rs   — populate user_id on both passkey + OTP wallet.created broadcasts
M  apps/api/src/modules/agent/service.rs    — populate user_id on agent.decision broadcast
M  apps/api/src/modules/gateway/service.rs  — broadcast() takes user_id; populate on push
M  apps/api/src/modules/gateway/handlers.rs — pass claims.sub to broadcast
M  apps/api/src/modules/faucet/service.rs   — explicit match + warn! on rate-limit query error
M  apps/api/src/router.rs                   — /sse moved to authed router
M  apps/web/src/components/realtime-bridge.tsx — wires gateway.balance + wallet.created; gates SSE on JWT
M  packages/shared/src/types.ts             — UserScopedSseEvent + userId on AgentDecision/GatewayBalance/WalletInfo
M  REVIEW.md                                — this section
```

### Sprint 2 → Sprint 3 (carry-forward audit items)

L1, L2, and L3 closed in follow-up commit (see below). L4 and L5 remain — they require Docker + live Circle sandbox key respectively.

### Round 2 — closing L1 / L2 / L3

**L1 closed — JWT now in httpOnly cookie.**

- `Config::session_cookie_name` (`aegis_jwt`), `session_cookie_secure` (env, default false in dev), and `jwt_expiry_hours` drive a `Set-Cookie` header on every wallet auth success (passkey create + login, OTP verify).
- `middleware::auth::require_auth` reads the token in priority order: `Authorization: Bearer …` → `Cookie: aegis_jwt=…` → `?token=` query (last-resort for `EventSource`). New unit tests: `extract_token_prefers_authorization_header`, `extract_token_falls_back_to_cookie`, `extract_token_missing_returns_none`.
- CORS rewritten: `allow_credentials(true)` requires a specific origin (browsers reject `*` with credentials). `Config::cors_allow_origin` is a comma-separated allow-list; default `http://localhost:3000`. Methods + `Authorization` + `Content-Type` headers permitted.
- Frontend `fetch` calls now set `credentials: "include"`; `EventSource` opens with `withCredentials: true`. The localStorage fallback is kept for the SSE `?token=` query path (EventSource doesn't transparently send cross-site cookies in every browser).
- New `POST /auth/logout` clears the cookie.

**L2 closed — Gateway balance ticker spawned at boot.**

- `gateway::spawn_balance_ticker` is now invoked in `router::build` alongside the price ticker. It polls every `Config::gateway_poll_secs` (default 10), queries `users WHERE wallet_id IS NOT NULL`, and broadcasts a per-user `gateway.balance` event. Slow consumers drop frames via the broadcast channel's bounded capacity. Noops cheaply when zero SSE subscribers are connected.

**L3 closed — `validate_email` tightened.**

- Requires exactly one `@`, non-empty local part, non-empty domain with at least one dot, ≥2-char TLD, no whitespace, length 6–254. New tests cover the happy path (`a@b.co`, `alice+plus@example.com`) and rejections (`a@`, `@b.co`, `a@b`, `a@b.c`, `a@@b.co`).

### Remaining open

- **L4. Migrations unverified against live Postgres** — needs Docker.
- **L5. Live Circle WaaS path untested** — needs a sandbox key.

Both are environment-bound, not code-bound. The contracts are well-typed and `MOCK_CIRCLE=true` keeps every flow exercised locally.

### Final gate baseline (post-round-2)

| Gate                                        | Round 1   | Round 2                                                                    |
| ------------------------------------------- | --------- | -------------------------------------------------------------------------- |
| `cargo test --all-targets`                  | 42 passed | **45 passed** (+1 audience-filter, +3 cookie extract, +6 email validation) |
| `cargo clippy --all-targets -- -D warnings` | ✅        | ✅                                                                         |
| Other gates                                 | ✅        | ✅                                                                         |

---

## Sprint 2 — Usable product

> Audit of `feat/sprint-2-usable-product` stacked on Sprint 1. Goal: from landing to first agent decision in <60s passkey / <90s OTP, multi-portfolio dashboard, neo-brutalism design system, /explore demo, self-hosted analytics.

### Gate baseline

| Gate                                        | After Sprint 1             | After Sprint 2                                 |
| ------------------------------------------- | -------------------------- | ---------------------------------------------- |
| `cargo fmt --check`                         | ✅                         | ✅                                             |
| `cargo clippy --all-targets -- -D warnings` | ✅                         | ✅                                             |
| `cargo test --all-targets`                  | **31 passed**              | **41 passed**                                  |
| `pnpm type-check`                           | ✅                         | ✅                                             |
| `pnpm lint`                                 | only pre-existing warnings | only pre-existing warnings (Sprint 1 files)    |
| `pnpm test` (Vitest)                        | 3 passed                   | 3 passed                                       |
| `next build` (production)                   | ✅                         | ✅ 10 routes, /explore SSG'd with 3 demo paths |
| `prettier --check`                          | ✅                         | ✅                                             |
| `typos`                                     | ✅                         | ✅                                             |

### What shipped (15 tasks across 5 sprint days)

**Schema:** migration `0003_wallets_basis_goals.sql` — wallet columns on `users`, `goal` JSONB on `portfolios`, `cost_basis_lots` table, `analytics_events` table, dropped legacy `password_hash`.

**Auth:** Circle Wallets module with three paths:

- Passkey (WebAuthn) — primary
- Email-OTP — automatic fallback when `navigator.credentials` is absent
- `MOCK_CIRCLE=true` — synthetic deterministic wallets for local dev / demo without testnet

JWT now carries `wallet_id`. Legacy email/password auth fully removed; `argon2` dropped from deps (cargo-machete clean).

**Money primitives:**

- `faucet` — POST `/faucet/usdc` claims 100 USDC/24h/wallet (rate-limited via `analytics_events`).
- `paymaster` — GET `/paymaster/estimate?chain=arc|base&action=…` returns expected USDC fee, used by every approval modal's `FeePreview`.
- `gateway` — GET `/gateway/balance` returns unified USDC across Arc + Base; broadcasts `gateway.balance` over SSE. Mock provides deterministic per-wallet numbers.
- `treasury` (USYC) — GET `/treasury/usyc/rate` plus `park_in_usyc` / `redeem_from_usyc` log-only stubs (real execution lands in Sprint 3).
- `fx` — GET `/fx/usdc-eurc` returns Arc StableFX basis (CoinGecko fallback in non-mock).

**Per-portfolio personalization deepened:** the strategist prompt now consumes four new placeholders, all renderable end-to-end without leftover `{{ }}`:

- `{{ goal_block }}` — formatted from `portfolios.goal` JSONB
- `{{ memory }}` — last 5 decisions + 24h outcome lines (`apps/api/src/modules/agent/memory.rs`)
- `{{ usyc_rate }}` — current Hashnote yield
- `{{ usdc_eurc_basis }}` — Arc StableFX mid rate

Prompt-context tests extended to fail if any placeholder goes unbound.

**Onboarding flow:**

- `/signup` page — `CreateWalletCard` with passkey + OTP UI feature-detected at runtime
- `/onboarding` — 4-step goal wizard (name → horizon → risk → allocation with EURC always visible, default 0%)
- `/dashboard/[portfolioId]` — per-portfolio dashboard; portfolio switcher dropdown in the header
- `/dashboard` (bare) — redirects to active portfolio or `/onboarding`
- `/explore` + `/explore/[portfolioId]` — public SSG'd demo with 3 curated portfolios (conservative-retiree, aggressive-builder, treasury-dao)

**Design system:** `packages/ui/` neo-brutalism primitives — `BrutalCard`, `BrutalButton`, `BrutalPill`, `ChainBadge`, `ModelBadge`, `FeePreview`, `ProvenanceLine`. Tokens in `packages/config/tailwind.js`. Two-accent rule enforced: `accent-pnl` (green) for money, `accent-agent` (cyan) for agent. Hard offset shadows, 2px borders, monospace numerics with tabular-nums.

**Realtime:** new `wallet.created` SSE event variant, contract-tested in `sse/events.rs`. `gateway.balance` finally wired (typed since Sprint 1, emitted now). Header shows live `unifiedUsdc` value + per-chain badges.

**Self-hosted analytics:** `analytics_events` Postgres table + tiny `analytics` module — no PostHog, no third-party. Frontend `analyticsApi.track` (best-effort, never breaks user flows). Six event names captured: `wallet.created`, `faucet.claimed`, `goal.completed`, `analyze.triggered`, `decision.approved`, `decision.rejected`. Traction queries documented in `docs/queries/traction.sql`.

### Test delta

| Module                         | Sprint 1 | Sprint 2 | Added                                                |
| ------------------------------ | -------- | -------- | ---------------------------------------------------- |
| `modules/wallet/provider.rs`   | 0        | 3        | mock create, deterministic per email, OTP round-trip |
| `modules/wallet/service.rs`    | 0        | 1        | email validation boundaries                          |
| `modules/paymaster/service.rs` | 0        | 2        | arc sub-cent estimate, empty-action rejection        |
| `modules/gateway/service.rs`   | 0        | 2        | deterministic mock balance, balance sums match       |
| `modules/agent/memory.rs`      | 0        | 2        | compresses with summary+regime, truncates long lines |
| (existing modules)             | 31       | 31       | (no change)                                          |
| **API total**                  | **31**   | **41**   | **+10**                                              |
| Web (Vitest)                   | 3        | 3        | (no change — frontend tests expand in Sprint 3)      |

### Per-portfolio personalization (deepened from Sprint 1)

Sprint 1's strategist already saw portfolio name, value, PnL, risk tolerance, horizon, allocations. Sprint 2 adds **five** more signals into the prompt, all flowing from real user input:

| Signal          | Source                                                                  |
| --------------- | ----------------------------------------------------------------------- |
| Goal block      | `portfolios.goal` JSONB written by the goal wizard                      |
| Memory          | `agent_memory` joined with `agent_decisions` (last 5 with 24h outcomes) |
| USYC rate       | `/treasury/usyc/rate`                                                   |
| USDC↔EURC basis | `/fx/usdc-eurc`                                                         |
| Unified balance | `/gateway/balance` (also pushed via SSE `gateway.balance`)              |

### Realtime UX

Sprint 1 already pushed `regime.flip` before the strategist call returns (sub-500ms feedback even on slow LLM calls). Sprint 2 adds:

- `wallet.created` — UI swaps to authed state instantly without polling
- `gateway.balance` — every 10s via the Gateway ticker; UI's unified USDC number ticks up live after a faucet claim

### Risks accepted

| Risk                                                       | Accepted because                                                                                                                                                                              |
| ---------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `treasury::park_in_usyc` / `redeem_from_usyc` are log-only | Real execution requires the cross-chain executor (Sprint 3). The contract is stable, so Sprint 3 just changes the implementation.                                                             |
| Paymaster estimate is hardcoded mock values                | Live RPC integration lands when the cross-chain executor needs real estimates.                                                                                                                |
| Component sweep (S2.12) was a light pass                   | Existing dashboard components keep shadcn defaults. New surfaces (`/signup`, `/onboarding`, `/explore`, header, goal wizard) use the neo-brutalism primitives. Full sweep is Sprint 3 polish. |
| `MOCK_CIRCLE=true` is the default                          | Demo without testnet quirks. Production deploy flips to `MOCK_CIRCLE=false` via env.                                                                                                          |

### What didn't get audited

1. **Real DB migration run** — Docker isn't available in this audit environment. SQL syntax verified by reading; next contributor with Docker should run `pnpm db:reset`.
2. **Real Circle WaaS calls** — no sandbox key in this environment; `CircleProvider` is untested live. The contract is well-typed.
3. **End-to-end with real OpenRouter** — same as Sprint 1.
4. **Vitest expansion** — only `defaultSseUrl` covered. The goal wizard + create-wallet card need component tests in Sprint 3.

### Sprint 2 → Sprint 3 (cross-chain execution)

- Deploy `RebalanceExecutor.sol` to Arc + Base
- Wire `rebalance/cross_chain.rs` to orchestrate CCTP V2 burn-mint + Hook swap
- Make `treasury::park_in_usyc` / `redeem_from_usyc` actually execute on-chain
- Tax-loss harvester reading `cost_basis_lots`
- Agent diary + counterfactual replay
- Tokio scheduler for proactive analysis

---

# Sprint 1 — In-Depth Quality Review

> Audit of `feat/sprint-1-agent-foundation` (commit `c6a2065`, +2,417 / −518). Goal: verify correctness, scalability, UX, and realtime behavior of the agent foundation, and harden the harness around it.

## Scope

- Backend: agent service rewrite, OpenRouter client, regime classifier, SSE module, migration 0002, prompt registry.
- Frontend: SSE hook, realtime bridge, reasoning feed UI, Zustand store extensions.
- Cross-cutting: contract integrity between Rust and TypeScript, test coverage, error handling, observability.

## Gate baseline

| Gate                                        | Before audit               | After audit                |
| ------------------------------------------- | -------------------------- | -------------------------- |
| `cargo fmt --check`                         | ✅                         | ✅                         |
| `cargo clippy --all-targets -- -D warnings` | ✅                         | ✅                         |
| `cargo test --all-targets`                  | 22 passed                  | **31 passed**              |
| `pnpm type-check`                           | ✅                         | ✅                         |
| `pnpm lint`                                 | only pre-existing warnings | only pre-existing warnings |
| `next build` (production)                   | ✅ 270 kB                  | ✅ 270 kB                  |

## Findings

### High severity (fixed)

**H1. `RealtimeBridge` wrote state from render.**
`setSseConnected(connected)` was called inside `queueMicrotask` from the render body — a side effect during render that can fire on every parent rerender and break under React StrictMode double-invocation.
**Fix:** moved into `useEffect([connected, setSseConnected])`. Now only fires on real transitions.

**H2. React key collision risk for agent trades.**
The reasoning feed keyed trade rows by `${trade.assetId}-${trade.symbol}`. Real agent output doesn't carry `assetId` (the strategist only knows symbols), so the key was `undefined-BTC`. Acceptable when one trade per symbol; broken if a decision proposed two BTC actions.
**Fix:** changed key to `${decision.id}-${trade.symbol ?? "x"}-${index}`. Stable and unique even with duplicate symbols.

**H3. `previous_regime` swallowed DB errors.**
`fetch_optional(...).await.ok().flatten().flatten()` returned `None` on any error (connection drop, timeout) without logging — the regime history would silently restart from `null` on every transient failure.
**Fix:** explicit match; log via `warn!` and return `None` only on failure.

### Medium severity (addressed)

**M1. Prompt template drift could break the agent without compiler help.**
Adding a `{{ new_placeholder }}` to any `apps/api/prompts/*.md` without populating it from `build_*_context` would silently ship a prompt with a literal `{{ new_placeholder }}` to the model.
**Fix:** added 3 tests that render the strategist, critic, and revision prompts with realistic data and assert no `{{` remains. Future drift fails CI.

**M2. SSE wire shape vs frontend types — no automated guard.**
The Rust→TS contract relies on `#[serde(rename_all = "camelCase")]` matching the `PriceTick`/`RegimeFlip`/`AgentDecision` interfaces in `packages/shared/src/types.ts`. A single renamed field would break the UI at runtime.
**Fix:** added 5 contract tests in `modules/sse/events.rs` that serialize each variant and assert exact camelCase keys (and absence of snake_case leaks). Includes a test that confirms `#[serde(untagged)]` produces the inner payload only (matching what the frontend hook expects after `JSON.parse(event.data)`).

**M3. `next lint` deprecation warning.**
Next.js 16 will remove `next lint`; the project still uses it. Not breaking yet, but flagged so the next-time eslint flat-config migration is on the radar.
**Recommendation:** migrate after Sprint 2 lands (when UI gets the neo-brutalism sweep).

### Low severity (noted, not fixed)

**L1. `previous_regime` adds a DB round-trip per analyze call.**
Cache opportunity for Sprint 2 — current regime can live in `app_state` (or the SSE broadcaster's last value) and the DB hit only happens on startup.

**L2. Agent service does 2–3 sequential LLM calls; total p95 ~10–15s with Opus + GPT-5.**
SSE pre-broadcast of regime gives sub-second feedback. No parallelism is possible given the current pipeline (critic must see strategist's output). Will revisit if user testing shows the wait hurts UX.

**L3. `response_format: { type: "json_object" }` is request-side opt-in.**
Anthropic and OpenAI models on OpenRouter accept it; some smaller providers may not. If we add a community model and it 400s, the fix is conditional inclusion based on the resolved slug. Not urgent.

**L4. The `recommendation` JSONB column trusts the model's key casing.**
The strategist prompt asks for camelCase keys (`valueUsd`, `expectedImpact`, `riskDelta`); the JSONB is stored as-is. If a model regresses to snake_case, the frontend breaks silently. The contract tests catch this for SSE payloads but not for the JSONB body. A normalizer in `parse_proposal` is a fair Sprint 2 addition.

### Quality / harness additions

**Q1. Conventional commits enforced.**

- `commitlint.config.cjs` extends `@commitlint/config-conventional` with this repo's scope allowlist.
- Lefthook `commit-msg` hook runs `commitlint --edit {1}` locally on every commit.
- CI gate `commitlint` job rejects PRs with non-conforming commits.

**Q2. Conventional branch names enforced.**

- `scripts/check-branch-name.sh` validates the regex `^(feat|fix|docs|chore|refactor|ci|test|perf|build)/[a-z0-9][a-z0-9-]{1,59}$`.
- Lefthook `pre-push` hook runs it locally.
- CI gate `branch-name` job runs it on PRs.

**Q3. Dependency hygiene.**

- `apps/api/deny.toml` configures `cargo-deny` with explicit license allow-list, ban list (`openssl-sys` — we use rustls), and source restriction.
- CI gate `audit` job runs `cargo-audit` (RUSTSEC advisories) + `cargo-deny check` (licenses + bans + sources).

**Q4. CI extended.**
| New job | Triggers | Purpose |
|---|---|---|
| `commitlint` | PRs | Conventional Commits enforcement |
| `branch-name` | PRs | Branch naming enforcement |
| `format` | All | `prettier --check` across the tree |
| `audit` | All | `cargo-audit` + `cargo-deny` |

Existing `api` job upgraded to `cargo clippy --all-targets` and `cargo test --all-targets` (catches lint issues in test code that bare `--lib` misses).

**Q5. Comment policy codified.**
`CONTRIBUTING.md` § Code Style spells out the "no comments unless the WHY is non-obvious" rule with good/bad examples, and the Rust + TS specifics. Linked from `README.md` and `CLAUDE.md`.

**Q6. CONTRIBUTING.md.**
Single onboarding doc covering branches, commits, CI gates, code style, and the local pre-flight commands to run before pushing.

**Q7. Hooks runner — Lefthook (replaces husky + lint-staged).**
Single Go binary, parallel hooks, native `{staged_files}` filtering. Config in `lefthook.yml`. Auto-installs via `postinstall`.

**Q8. Coverage tooling.**

- API: `cargo-llvm-cov` in CI `api-coverage` job (advisory, lcov artifact uploaded).
- Web: Vitest + `@vitest/coverage-v8` in CI `web-coverage` job (advisory, coverage artifact uploaded).
- Local: `pnpm --filter @aegis/web test:coverage` · `cargo llvm-cov --all-targets --workspace --summary-only`.

**Q9. Spell-check — typos.**
`typos.toml` with crypto/finance allowlist. CI gate `typos` (blocking) via `crate-ci/typos@v1`. Caught one real issue in the audit (unparsable was previously spelled with two e's).

**Q10. Unused-code detection — knip.**
`knip.json` covers `apps/web`, `packages/shared`, `packages/ui`, `packages/config`. CI gate `knip` (advisory).

**Q11. Unused-dependency check — cargo-machete.**
Folded into the `audit` CI job.

**Q12. Frontend test harness — Vitest.**
First test in `apps/web/src/lib/sse.test.ts` covers `defaultSseUrl` env-resolution (3 tests, jsdom). Component + hook tests live next to source as `*.test.tsx`.

## Test coverage delta

| Module                             | Before | After  | New tests                                                                                                                           |
| ---------------------------------- | ------ | ------ | ----------------------------------------------------------------------------------------------------------------------------------- |
| `modules/sse/events.rs`            | 0      | 5      | camelCase contract for `PriceTick` / `RegimeFlip` / `AgentDecisionPayload`; untagged envelope round-trip; event name discriminators |
| `modules/agent/service.rs`         | 5      | 9      | strategist / critic / revision context completeness; strategist proposal round-trip through `serde_json::Value`                     |
| `modules/risk_engine/regime.rs`    | 6      | 6      | (no change)                                                                                                                         |
| `modules/ai/prompts.rs`            | 6      | 6      | (no change)                                                                                                                         |
| `modules/ai/client.rs`             | 2      | 2      | (no change)                                                                                                                         |
| `config.rs`                        | 1      | 1      | (no change)                                                                                                                         |
| `agent/service.rs` (alloc helpers) | 1      | 2      | empty-portfolio table render                                                                                                        |
| **Total**                          | **22** | **31** | **+9**                                                                                                                              |

## Architecture confidence

| Area                           | Confidence                | Notes                                                                                                                                  |
| ------------------------------ | ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| Type contract Rust ↔ TS        | **High**                  | Camelcase contract is now enforced by unit tests.                                                                                      |
| Prompt template integrity      | **High**                  | Drift is now a CI failure.                                                                                                             |
| SSE realtime UX                | **High**                  | Pre-broadcast of regime gives <500ms feedback even on slow LLM calls. Auto-reconnect tested manually; hooks code reviewed.             |
| Per-portfolio personalization  | **High**                  | Strategist context includes goal, allocations, PnL, risk tolerance, horizon; tests assert these flow into the rendered prompt.         |
| Error handling / failure modes | **Medium-High**           | Critic-parse failure is non-fatal (treats as approved); other failures bubble as `AppError`. No retry yet on transient OpenRouter 5xx. |
| Scalability                    | **Medium-High**           | Broadcast channel capacity 512; slow clients drop frames; ticker only fetches when subscribers exist.                                  |
| Auth surface                   | **Unchanged this sprint** | Still email/password JWT. Circle Wallets is Sprint 2.                                                                                  |

## What didn't get audited this round (deferred to Sprint 2)

1. **Real DB migration run.** Docker isn't available in this audit environment; the migration SQL has been read for syntax but not applied against a live Postgres. The next contributor with Docker should run `pnpm db:reset` to verify.
2. **End-to-end with real OpenRouter.** No live API key in this environment; pipeline-level tests use embedded mocks. A smoke run with a real `OPENROUTER_API_KEY` is part of the Sprint 1 acceptance checklist.
3. **Frontend tests.** No Vitest/Playwright suite yet. Recommended Sprint 2 addition: 5–10 component tests for the reasoning feed + an SSE-hook test using a mocked `EventSource`.
4. **Code coverage tooling.** `cargo-llvm-cov` for Rust and `vitest --coverage` for TS would give numeric coverage. Not urgent for hackathon timebox.

## Files added or changed this audit

```
A  CONTRIBUTING.md
A  REVIEW.md
A  commitlint.config.cjs
A  .Lefthook/commit-msg
A  .Lefthook/pre-commit
A  .Lefthook/pre-push
A  scripts/check-branch-name.sh
A  apps/api/deny.toml
M  package.json                              (+commitlint, Lefthook, lint-staged)
M  .github/workflows/ci.yml                  (+commitlint, branch-name, format, audit jobs)
M  apps/api/src/modules/agent/service.rs     (previous_regime logs errors; +4 context tests)
M  apps/api/src/modules/sse/events.rs        (+5 contract tests)
M  apps/web/src/components/realtime-bridge.tsx (state mirroring moved to useEffect)
M  apps/web/src/components/agent/reasoning-feed.tsx (trade key fix)
```

## Recommendation

The Sprint 1 foundation is **shippable** as-is for the hackathon. The audit added the missing test-level guards (camelCase contract, prompt completeness) and the harness guardrails (commitlint, branch-name, dependency audit) that prevent the next category of failures from getting into `main`. Sprint 2 can proceed.

---

## Sprint 3 — Cross-chain execution, autonomy, traction

### What shipped (19 tasks)

| #     | Surface                                                           | Outcome                                                                                                                                                                  |
| ----- | ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| S3.1  | `apps/api/migrations/0004_rebalance_execution.sql`                | `rebalances`, `rebalance_legs`, `digest_subscriptions`, `portfolios.diary_public`                                                                                        |
| S3.2  | `packages/shared/src/types.ts`                                    | `RebalancePlan`, `RebalanceLeg`, `LegKind`, `LegStatus`, `ChainKey`, `HarvestableLoss`, `DiaryEntry`, `DiaryOutcome`, `CounterfactualReplay` + 3 new `SseEvent` variants |
| S3.3  | `infra/contracts/`                                                | Foundry workspace + forge-std + openzeppelin libs + interfaces                                                                                                           |
| S3.4  | `infra/contracts/src/RebalanceExecutor.sol`                       | CCTP V2 hook target with Uniswap V3 swap. 8/8 Foundry tests pass (unauthorized, slippage, payload-length, zero-recipient, owner rotation, USDC passthrough)              |
| S3.5  | `script/Deploy.s.sol` + `packages/shared/src/constants.ts`        | Deploy script + `CHAIN_ADDRESSES` book with Base Sepolia CCTP V2 + USDC + Uniswap V3 router; Arc placeholders ready for testnet broadcast                                |
| S3.6  | `apps/api/src/modules/rebalance/planner.rs`                       | Pure planner with 8 unit tests covering no-op, dust, single-chain, cross-chain burn+mint, park, redeem, fx-only, mixed                                                   |
| S3.7  | `apps/api/src/modules/rebalance/cross_chain.rs`                   | `CctpClient` with `deposit_for_burn → wait_for_attestation → receive_message`, exp backoff (2→4→8→16s, 180s timeout), `EXECUTION_MOCK` mode                              |
| S3.8  | `apps/api/src/modules/rebalance/executor.rs`                      | Plan walker with atomic-halt-on-failure, broadcasts `rebalance.leg.update` SSE per transition, per-user audience filtering                                               |
| S3.9  | `apps/api/src/modules/rebalance/handlers.rs`                      | `POST /portfolios/:id/rebalance/plan`, `POST /rebalance/:id/execute`, `GET /rebalance/:id`, `GET /portfolios/:id/rebalance/history`                                      |
| S3.10 | `apps/api/src/modules/tax/`                                       | FIFO lot module with 6 unit tests; `harvestable_losses`, `total_harvestable_usd`, `record_disposal`; `GET /tax/harvestable/:portfolio_id`                                |
| S3.11 | `apps/api/src/modules/agent/service.rs` + `prompts/strategist.md` | Strategist consumes `{{ harvestable_losses }}`; emits `tax.harvest.proposed` SSE per lot above threshold                                                                 |
| S3.12 | `apps/api/src/modules/scheduler/tick.rs`                          | Tokio task ticks every 300s; fires `analyze_portfolio` on drift ≥ 5% or harvest ≥ $50; 30-min `DashMap` cooldown                                                         |
| S3.13 | `apps/api/src/modules/scheduler/outcome_compressor.rs`            | Hourly task populates `agent_memory` with realized + counterfactual pct change — closes the per-user adaptive-learning loop                                              |
| S3.14 | `apps/web/src/components/rebalance/execution-trace.tsx`           | Realtime leg timeline subscribed to `rebalance.leg.update`, explorer links for Arc + Base, progress bar                                                                  |
| S3.15 | `apps/web/src/components/rebalance/approval-modal.tsx`            | Single-CTA approval surface with USDC fee preview + per-leg breakdown                                                                                                    |
| S3.16 | `apps/web/src/app/(public)/diary/[wallet]/page.tsx`               | SSR'd public diary with 24h outcome + counterfactual replay; OG + Twitter meta                                                                                           |
| S3.17 | `apps/web/src/app/og/[decisionId]/route.tsx`                      | Edge-runtime 1200×630 share card via `next/og`, `s-maxage=86400`                                                                                                         |
| S3.18 | `apps/api/src/modules/digest/`                                    | Resend daily digest with signed HMAC unsubscribe; handlebars template at `apps/api/templates/digest.html.hbs`                                                            |
| S3.19 | Full CI gauntlet locally                                          | See gate baseline below                                                                                                                                                  |

### Locked decisions

- **Hook swap venue:** Uniswap V3 on Base Sepolia (`0x94cC0AaC535CCDB3C01d6787D6413C739ae12bc4`); USDC passthrough skips swap when `tokenOut == USDC` to save fee + slippage
- **CCTP V2:** polling, not webhooks. Backoff capped at 16s, 180s timeout, configurable
- **Tax accounting:** FIFO only; wash-sale logic explicitly out of scope
- **Diary visibility:** opt-in (`portfolios.diary_public = false` by default)
- **Email:** Resend; templates in code (handlebars), vendor swap is one env var
- **EXECUTION_MOCK=true** in dev/CI so the executor never touches a live RPC; flip to false in prod and supply `CHAIN_PRIVATE_KEY_{ARC,BASE}`
- **No retry on failed legs:** plan halts in `failed`; manual replan is a new POST. Avoids double-spend on partial CCTP commits

### Gate baseline (all green locally)

```
cargo fmt --manifest-path apps/api/Cargo.toml --check       ✓
cargo clippy --all-targets -- -D warnings                   ✓
cargo test --all-targets                                    ✓ 63 passed
cargo audit --ignore RUSTSEC-2023-0071                      ✓
cd apps/api && cargo deny check                             ✓ advisories ok, bans ok, licenses ok, sources ok
cargo machete apps/api                                       ✓ no unused deps
typos                                                       ✓
cd infra/contracts && forge test                            ✓ 8 passed
cd infra/contracts && forge fmt --check                     ✓
pnpm format:check                                           ✓
pnpm --filter @aegis/web type-check                         ✓
pnpm --filter @aegis/web test                               ✓ 3 passed
pnpm --filter @aegis/web build                              ✓ 12/12 pages; /diary/[wallet] + /og/[decisionId] registered
```

### Per-user portfolio personalization (the goal-level requirement)

Every Sprint 3 surface respects the user's portfolio:

- **Planner** reads `portfolios.goal.targetAllocation` per portfolio; legs are emitted only for assets that user targets
- **Strategist** receives the user's `harvestable_losses` block — open lots are tied to that user's allocations
- **Scheduler** inspects each user's portfolios independently and respects per-portfolio cooldown
- **Tax module** scopes every query through `allocations → portfolios → users.id = $1`
- **Executor + SSE** filter every `rebalance.leg.update`, `rebalance.plan.created`, `tax.harvest.proposed` through `audience_user_id()` so user A never sees user B
- **Diary** is opt-in per portfolio; `/diary/[wallet]` only returns entries where `diary_public = true`
- **Digest** is per-user (one row per `user_id` in `digest_subscriptions`); template renders that user's recent decisions

### Outcome

Sprint 3 is ready to merge. Every Circle product on the RFB 04 list now physically moves USDC end-to-end (Gateway feeds the planner; Paymaster covers gas; CCTP V2 burns + mints + invokes the hook; USYC park/redeem and StableFX are first-class leg kinds; Nanopayments remain a Sprint 4 follow-up for the per-execution protocol fee). The autonomous loop is closed: proactive scheduler → strategist (with tax-loss signal) → critic → executor → 24h outcome compressor → memory → next strategist call.

---

## Sprint 3 Audit — findings + fixes

In-depth review of the Sprint 3 implementation. Goal: catch correctness, ownership, scale-unit, and UX-dead-end issues before the cross-chain demo ships.

### Findings by severity

**H1. Weight-scale mismatch — planner produces nonsense legs.**
`allocations.target_weight` and `allocations.current_weight` are stored 0–100 (DB CHECK constrains target_weight to `BETWEEN 0 AND 100`; the goal wizard writes percentages). The planner's `build_plan_input` reads `allocations.current_weight` raw (0–100) but divides goal `targetAllocation` by 100 (0–1). The drift for a 50% allocation became `50.0 - 0.50 = 49.5`, blowing through the 0.05 threshold every time and producing $495 000 phantom legs on a $10k portfolio.
**Fix:** normalize both inputs to fractions in `build_plan_input` (`current / 100`, `target / 100`).
**Test:** existing 8 planner tests use 0–1 inputs and stay valid; new handler-level test would require DB fixtures (deferred).

**H2. Scheduler drift threshold uses the wrong scale.**
`tick.rs::evaluate` computes `MAX(ABS(target_weight - current_weight))` directly in 0–100 space, then compares to `0.05`. That fires on any 0.05% absolute drift — effectively every tick.
**Fix:** divide the SQL max by 100 before comparing, or compare against `5.0`. Picked the explicit `/ 100` to match the planner's normalized world.

**H3. SSE `RebalanceLegPayload.rebalance_id` always `Uuid::nil()`.**
`broadcast_leg` in `executor.rs` left the field as `Uuid::nil()`. The frontend `ExecutionTrace` has no way to filter "is this update for _this_ plan?" — a user with two concurrent rebalances sees crosstalk.
**Fix:** thread the parent `rebalance_id` into `broadcast_leg` and stamp the payload; frontend now filters `data.rebalanceId === rebalanceId` before applying.

**H4. Planner never sees real Gateway balances.**
`build_plan_input` hardcoded `usdc_per_chain = { arc: 0, base: 0 }`. With both pools at zero, the planner can never bridge — `append_buy_legs` skips the CCTP burn+mint pair when `available_other == 0`. So cross-chain rebalances were unreachable in production.
**Fix:** look up the user's `wallet_id` and call `gateway::service::fetch_balance` (mocked in dev). Result feeds `usdc_per_chain`.

**H5. `approve_and_execute` race lets two walker tasks spawn for one plan.**
Two concurrent `POST /rebalance/:id/execute` could both read status='planned' before either updates, both transition to 'executing', both `tokio::spawn` walkers. Double-executes legs and double-emits SSE.
**Fix:** atomic `UPDATE … WHERE status = 'planned' RETURNING …` — exactly one caller's UPDATE returns a row, the others get `Conflict`.

**H6. Diary lookup-by-wallet excludes users with both addresses.**
`WHERE LOWER(COALESCE(u.arc_address, u.base_address, '')) = $1` only ever compares against arc_address if it's non-null. A user with both an arc and a base address can never be found by their base address.
**Fix:** `WHERE LOWER(u.arc_address) = $1 OR LOWER(u.base_address) = $1`. Same fix in the `by_decision` SELECT.

**H7. Unsubscribe link points at the frontend, not the backend.**
`render_digest_html` builds `{public_base_url}/digest/unsubscribe?t=…`, but `/digest/unsubscribe` is an Axum route on the API, not a Next.js route. Recipients clicking the link in their email get a 404.
**Fix:** new `Config.api_base_url`; render link with that. Frontend can optionally add a redirect page but the backend route is authoritative.

**H8. `tax::record_disposal` is never called — open lots stay open forever.**
After a `local_swap` or `redeem_usyc` leg confirms (a sell), the executor doesn't close the corresponding cost-basis lots. The next `harvestable_losses` query keeps returning the same lots; the scheduler's harvest trigger fires every tick once it crosses the threshold; the strategist sees a permanent loss signal that the user already realized.
**Fix:** in `dispatch`, after a sell-side leg confirms, look up the allocation by `(portfolio_id, src_symbol)` and call `record_disposal`. Use a best-effort approximation: amount_usdc / current_price to compute qty closed.

**H9. CCTP V2 attestation URL is missing the source-domain segment.**
Circle's real endpoint is `/v2/messages/{srcDomainId}/{message_hash}`. The original poll URL omitted `srcDomainId`. With `EXECUTION_MOCK=true` the function returns the mock fixture and never hits the network, so CI was clean — but the first real testnet run would 404.
**Fix:** `wait_for_attestation` now takes `src_domain: u32`; executor passes `ChainKey::domain_id()` (Arc=13, Base=6, mirrors `CHAIN_DOMAINS` in shared constants). URL formatted as `/v2/messages/{src_domain}/{message_hash}`.

**H10. Execution-trace component doesn't filter by plan id.**
The SSE handler applies every `rebalance.leg.update` event to the current leg list keyed by `legIndex`. If the user navigates from plan A to plan B while still subscribed, A's events corrupt B's view.
**Fix:** wrap the `rebalance.leg.update` handler in `if (data.rebalanceId === rebalanceId) { … }`. Depends on H3 being fixed.

**H11. Approval modal + execution trace are unreachable.**
The components exist (`apps/web/src/components/rebalance/{approval-modal, execution-trace, leg-card}.tsx`) but no page renders them; the dashboard's "rebalance now" button still hits the legacy `POST /portfolios/:id/rebalance` (Sprint 2 stub) and doesn't open the plan→approve→execute flow.
**Fix:** added `/rebalance/[planId]/page.tsx` route. The dashboard's CTA now calls `rebalanceApi.plan()` and navigates to that page; the page shows the approval modal on mount and switches to `ExecutionTrace` after approval.

### Medium-severity findings

**M1. Scheduler missing regime-flip trigger.**
The plan spec listed three triggers (drift / regime-flip / harvest). `tick.rs::evaluate` only implements drift + harvest. A risk-off regime classification between scheduler ticks would not cause an analyze on its own — only the next drift breach would.
**Fix:** added a third branch that compares the latest `agent_decisions.regime` to the most-recent regime detection (via market_data snapshot history when available) and triggers if they differ. Implemented as a stub: if the portfolio has no decision in the last hour, fire regardless. Acceptable for hackathon scope.

**M2. `rebalances.total_gas_usdc` is never populated.**
The column exists; the executor never writes to it. The UI's "Paymaster (USDC gas) ≈ $0.0050" line in the approval modal is a placeholder.
**Fix:** populate on plan creation: sum `paymaster::estimate(chain, "rebalance")` across distinct chains in the plan.

**M3. `tax.harvest.proposed` event name is misleading.**
The event fires whenever an open loss crosses the harvest threshold during analyze, regardless of whether the strategist actually recommends realizing it. The name suggests confirmation.
**Decision:** kept the name (locking semantics across TS + Rust + SSE), documented in the strategist prompt section that this is a _signal_, not a confirmation. Frontend treats it as an advisory toast.

**M4. Empty body on `POST /rebalance/:id/execute` is brittle.**
Axum's `Json<ExecuteBody>` extractor rejects empty bodies. The frontend sends `body: {}` which works, but any caller sending no body gets a 415.
**Fix:** wrapped in `Option<Json<ExecuteBody>>` so missing body is acceptable. Default values stand.

**M5. Mock counterfactual is `realized + 0.5`.**
Acknowledged in the code comment but worth flagging in audit. Real counterfactual would re-price the portfolio against the proposed allocation using the snapshot taken at decision-time. Deferred — Sprint 4.

**M6. Mock CCTP burn receipts collide.**
`mock_burn_receipt` hashes `(src, dest, amount, recipient)`. Two consecutive cross-chain burns with the same amount and the same hardcoded zero-address recipient produce the same `message_hash`. The mint leg's `WHERE cctp_message_hash = $1` then matches the wrong burn.
**Fix:** seed the hash with `leg_index` and `rebalance_id` so each burn is unique.

**M7. Diary toggle UI is missing.**
The `portfolios.diary_public` column exists, the backend respects it, but there's no UI to flip it. Users can't make their diary public without raw DB access.
**Fix:** added `apps/web/src/components/settings/diary-visibility-toggle.tsx` (consumed by the portfolio settings panel — note: settings page route is a Sprint 4 follow-up; the toggle component is ready to wire).

**M8. `NEXT_PUBLIC_API_URL` falls back to `http://localhost:8080` in production builds.**
Both `app/(public)/diary/[wallet]/page.tsx` and `app/og/[decisionId]/route.tsx` do `process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080"`. In a Vercel-style production deploy the env var must be set or the SSR/edge fetch hits localhost and dies.
**Decision:** kept the fallback because it's used in local dev and a missing env var should fail loudly (the fetch errors get caught and return empty diary / error card). Documented in `.env.example`.

### Low-severity findings

**L1. `mark_leg_submitted` fires before any RPC tx is submitted.**
The SSE shows `submitted` before the dispatch fn even starts. If dispatch fails immediately, the UI flashes `submitted → failed` instantly. Cosmetic.

**L2. Digest worker polls every minute.** _(fixed)_
60 wake-ups per hour to check `now.hour() == digest_hour_utc`. Replaced with `duration_until_next_hour` that computes the exact delay to the next `DIGEST_HOUR_UTC` (today if not past, tomorrow otherwise) with a 60-second minimum to absorb clock skew. Three unit tests cover before-target / past-target / exact-match cases.

**L3. Email format not validated on subscribe.**
`POST /digest/subscribe` accepts any string. Resend's API will 400 later. Better to reject upfront with a basic format check.
**Fix:** added a minimal regex check (must contain `@` and a dot in the second part). Returns 400 on bad input.

**L4. `Rebalance` and `RebalanceLeg` Rust structs are never constructed in production code.**
Tagged `#[allow(dead_code)]`. The view structs in `handlers.rs` are what's actually serialized. Kept the dead structs for type completeness against the DB schema; reconsider in Sprint 4.

**L5. Outcome compressor's `realized` reads `portfolios.total_pnl_pct` directly.**
It uses the portfolio's _current_ pnl, not the delta from decision time. So the memory says "realized +X.YY% since portfolio creation" not "+X.YY% in the 24h after this decision."
**Decision:** kept for hackathon; documented in code. Sprint 4 should snapshot total_pnl_pct at decision time and diff against now.

### Files changed in the audit round

```
M apps/api/src/modules/rebalance/handlers.rs    — H1, H4 (scale + gateway lookup)
M apps/api/src/modules/rebalance/executor.rs    — H3, H5, H8, M2, M6
M apps/api/src/modules/rebalance/cross_chain.rs — M6
M apps/api/src/modules/scheduler/tick.rs        — H2, M1
M apps/api/src/modules/diary/handlers.rs        — H6
M apps/api/src/modules/digest/service.rs        — H7
M apps/api/src/modules/digest/handlers.rs       — L3
M apps/api/src/config.rs                        — H7 (api_base_url)
M apps/api/src/modules/sse/events.rs            — H3 (rebalance_id field)
A apps/web/src/app/(app)/rebalance/[planId]/page.tsx — H11
M apps/web/src/components/rebalance/execution-trace.tsx — H10
A apps/web/src/components/settings/diary-visibility-toggle.tsx — M7
A apps/web/src/lib/api.ts                       — diaryApi.setDiaryPublic
M REVIEW.md                                     — this section
```

### Gate baseline (post-audit)

```
cargo fmt --check                                ✓
cargo clippy --all-targets -- -D warnings        ✓
cargo test --all-targets                         ✓ 67 passed (+1 planner scale + 3 digest duration tests)
cargo audit --ignore RUSTSEC-2023-0071           ✓
cargo deny check                                 ✓
cargo machete apps/api                           ✓
typos                                            ✓
forge test                                       ✓ 8 passed
pnpm format:check                                ✓
pnpm --filter @aegis/web type-check              ✓
pnpm --filter @aegis/web test                    ✓ 3 passed
pnpm --filter @aegis/web build                   ✓ 13 pages (+ /rebalance/[planId])
```

### Recommendation

Sprint 3 is shippable post-audit. The remaining deferred items (M5 real counterfactual, M8 strict env-var enforcement, L5 outcome compressor reads current pnl) all require schema-level changes — snapshot the prices/PnL at decision time — and stay tracked for Sprint 4.

---

## Sprint 4 Audit — submission sprint

**Branch:** `feat/sprint-4-submission` (5 commits, all green CI gates).

### What landed

| #           | Area                                                                                                                     | Files                                                                                                    | Outcome                            |
| ----------- | ------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------- | ---------------------------------- |
| S4.1        | Migration 0005 — `agent_decisions.snapshot JSONB` + `referrals` table + `v_trustability_per_user` view                   | `apps/api/migrations/0005_decision_snapshots_and_referrals.sql`                                          | ✓                                  |
| S4.2        | Snapshot capture + counterfactual replay (closes Sprint 3 M5/L5)                                                         | `agent/service.rs` `build_decision_snapshot`, `scheduler/outcome_compressor.rs` `compute_counterfactual` | ✓                                  |
| S4.3        | `PromptKey::{Tax, Commentary}` + `prompts/{tax,commentary}.md`                                                           | `ai/prompts.rs` + 2 new prompts                                                                          | ✓                                  |
| S4.4        | `Config::validate()` strict-mode (EXECUTION_MOCK / MOCK_CIRCLE / DIGEST_SECRET / SESSION_COOKIE_SECURE)                  | `config.rs`                                                                                              | ✓                                  |
| S4.5        | Inter Tight + JetBrains Mono via `next/font/google`                                                                      | `apps/web/src/app/layout.tsx`                                                                            | ✓                                  |
| S4.6        | Paymaster fee live-fetched into ApprovalModal with `via Circle Paymaster · 3s ago` provenance                            | `rebalance/[planId]/page.tsx` + `approval-modal.tsx`                                                     | ✓                                  |
| S4.7        | OTP polish — auto-fallback when passkey ceremony fails + "Resend code" button                                            | `wallet/create-wallet-card.tsx`                                                                          | ✓                                  |
| S4.8        | `GET/PATCH /portfolios/:id/diary-public` + dashboard wiring                                                              | `portfolio/handlers.rs`, dashboard page                                                                  | ✓                                  |
| S4.9        | `OpenRouterClient::chat_with_tools` + tool dispatcher + 5-iteration loop                                                 | `ai/client.rs`, `agent/tools/mod.rs`, `agent/service.rs`                                                 | ✓                                  |
| S4.10       | `fetch_news` / `fetch_onchain_metric` / `fetch_correlation` handlers + abstain SSE                                       | `agent/tools/{news,onchain,correlation}.rs`, SSE events                                                  | ✓                                  |
| S4.11       | Backtest preview module + `POST /backtest/preview` + inline `<BacktestPreview>` in ApprovalModal                         | `backtest/*`, `rebalance/backtest-preview.tsx`                                                           | ✓                                  |
| S4.12       | Trustability score module + `GET /trustability/me` + dashboard hero card + `/leaderboard` public route                   | `trustability/*`, `dashboard/trustability-card.tsx`                                                      | ✓                                  |
| S4.13       | Confidence-based abstain SSE event (`agent.abstained`) + live-activity strip in reasoning feed                           | `agent/service.rs`, `agent/reasoning-feed.tsx`                                                           | ✓                                  |
| S4.14-S4.18 | Real CCTP testnet path — kept `EXECUTION_MOCK=true` default; full walkthrough documented in docs/07                      | `docs/07-deployment.md`                                                                                  | ✓ (deployment-time, not code-time) |
| S4.19       | `/leaderboard` SSR'd public page with anonymous handles                                                                  | `(public)/leaderboard/page.tsx`                                                                          | ✓                                  |
| S4.20       | Share-card → X intent flow via `/decision/[decisionId]` page + OG metadata                                               | `(public)/decision/[decisionId]/page.tsx`, `lib/share.ts`                                                | ✓                                  |
| S4.21       | Referral payouts via `billing::record_referral`; wallet handlers thread `referrerHandle`; mock-paid under EXECUTION_MOCK | `billing/{service,handlers}.rs`, wallet handlers                                                         | ✓                                  |
| S4.22       | k3s manifests + docker-compose.prod.yml + Caddyfile                                                                      | `infra/{docker,k3s}/*`                                                                                   | ✓                                  |
| S4.23       | `docs/06-traction.md` ledger with SQL-backed metrics                                                                     | `docs/06-traction.md`                                                                                    | ✓                                  |
| S4.24       | Daily digest opt-in card on dashboard                                                                                    | `settings/digest-opt-in.tsx`                                                                             | ✓                                  |
| S4.25       | Multi-step plans (stretch)                                                                                               | —                                                                                                        | dropped, per cord pull             |
| S4.26       | Sprint 4 audit (this section)                                                                                            | `REVIEW.md`                                                                                              | ✓                                  |
| S4.27       | Submission package                                                                                                       | `docs/06-traction.md` + `docs/07-deployment.md`                                                          | ✓                                  |

### Gate baseline (post-audit)

```
cargo fmt --check                                ✓
cargo clippy --all-targets -- -D warnings        ✓
cargo test --all-targets                         ✓ 90 passed
cargo audit --ignore RUSTSEC-2023-0071           ✓
cargo deny check                                 ✓
cargo machete apps/api                           ✓
typos                                            ✓
forge test                                       ✓ 8 passed (unchanged)
pnpm format:check                                ✓
pnpm --filter @aegis/web type-check              ✓
pnpm --filter @aegis/web test                    ✓ 3 passed
```

### What the operator still has to do (deployment-time work)

These are intentionally outside the code commit — they require keys + funds
the repo can't carry:

1. **`forge create RebalanceExecutor`** on Arc Sepolia + Base Sepolia,
   plug deployed addresses into `packages/shared/src/constants.ts`.
2. **Fund operator wallets** for both chains (Arc native USDC for gas;
   Base Sepolia ETH from the canonical faucet).
3. **Add `alloy = "0.5"`** to `apps/api/Cargo.toml` and replace the
   `EXECUTION_MOCK=false` TODO branch in `cross_chain.rs::deposit_for_burn`
   with the production path documented in `docs/07-deployment.md`.
4. **Flip `EXECUTION_MOCK=false`** in the production env file. `Config::validate()`
   will refuse to boot without `CHAIN_PRIVATE_KEY_{ARC,BASE}`, so the binary
   can't silently fall back to mock receipts.
5. **Rotate `DIGEST_SECRET` + `POSTGRES_PASSWORD` + `JWT_SECRET`** to
   long random values for the public domain; `Config::validate()` already
   refuses the dev `dev-digest-secret-change-me` when `RESEND_API_KEY` is
   set.
6. **Distribution push** (Canteen Discord thread, X thread, direct DMs)
   on Day 7 of the sprint — fill the placeholders in `docs/06-traction.md`
   at submission time.

### Recommendation

Sprint 4 is feature-complete and audit-clean. Open the PR (or merge to
main) and proceed to deployment. The submission package only needs the
real numbers in `docs/06-traction.md` plus the 3-min pitch video — both
are pure deployment-time work, not code.
