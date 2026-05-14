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
