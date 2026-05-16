# CLAUDE.md

Guidance for Claude Code when working on Aegis. **Read [`docs/`](./docs/) first** — especially [`docs/06-harness.md`](./docs/06-harness.md) for how this repo uses Claude Code's skills, subagents, and hooks. For branch / commit / CI / coverage details, see [`CONTRIBUTING.md`](./CONTRIBUTING.md). For environment-variable hygiene (load order, secrets-vs-public split, common pitfalls), read [`docs/09-env-hygiene.md`](./docs/09-env-hygiene.md) before editing `.env` or `.env.local`.

## What this is

**Aegis** is an adaptive crypto portfolio harness for stablecoin-native finance, submitted to **RFB 04: Adaptive Portfolio Manager** at Canteen × Circle's **Agora Agents Hackathon** (May 11–25, 2026). The user steers (sets a goal, approves moves); a multi-model AI agent executes on **Arc + Base** through Circle's stack.

Repository is a **Turborepo monorepo** with a Next.js 15 frontend (`apps/web`) and a Rust/Axum backend (`apps/api`).

## Locked-in decisions (do not silently change)

| Decision                                                                                   | Rationale                                                     | Doc                                                      |
| ------------------------------------------------------------------------------------------ | ------------------------------------------------------------- | -------------------------------------------------------- |
| **OpenRouter** as the AI gateway (not OpenAI direct)                                       | Per-task model routing — Haiku/Opus/Sonnet/Gemini Flash/GPT-5 | [`docs/02-agent-design.md`](./docs/02-agent-design.md)   |
| **Arc + Base** as the two settlement chains (≥2 required)                                  | Arc for native USDC gas; Base for CCTP V2 + Hooks             | [`docs/00-overview.md`](./docs/00-overview.md)           |
| **SSE** for realtime (`/sse`), not WebSocket                                               | Server→client only; native `EventSource`; trivial proxying    | [`docs/01-architecture.md`](./docs/01-architecture.md)   |
| **Circle stack**: Wallets · Gateway · CCTP V2 · USYC · Paymaster · StableFX · Nanopayments | All required for Circle Tool Usage judging                    | [`docs/03-circle-stack.md`](./docs/03-circle-stack.md)   |
| **EURC sleeve** via Arc StableFX                                                           | Multi-currency portfolio with native FX rails                 | [`docs/03-circle-stack.md`](./docs/03-circle-stack.md)   |
| **Dark neo-brutalism** UI with **dual-accent** (green = money, cyan = agent)               | Premium fintech feel; strict separation rule                  | [`docs/04-design-system.md`](./docs/04-design-system.md) |
| **Real users** (not simulated) for the Traction judging dimension                          | 30% of the score                                              | [Plan](#)                                                |
| **Project docs** in OpenAI "harness engineering" style, condensed (~1.5–2.5k words)        | Communicates engineering rigor to judges                      | [`docs/`](./docs/)                                       |

## Workspace layout

```
apps/
  web/          Next.js 15 — TS · Tailwind · neo-brutalism · Zustand · React Query · EventSource
  api/          Rust Axum — Tokio · SQLx · PostgreSQL · SSE · OpenRouter · Circle SDK
packages/
  shared/       Domain types + chain constants (TS)
  ui/           Neo-brutalism primitives (Card, Button, Pill, ModelBadge, ChainBadge…)
  config/       Shared ESLint, TypeScript, Tailwind configurations
infra/
  contracts/    RebalanceExecutor.sol (Foundry)
  docker/       Dockerfiles for api and web
docs/           Project documentation (read first)
```

## Common commands

```bash
# Install all workspace dependencies
pnpm install

# ── Frontend (apps/web) ──────────────────────────────────────────────────
pnpm dev                          # Next.js dev server (localhost:3000)
pnpm build
pnpm lint
pnpm type-check

# ── Backend (apps/api — run from apps/api/) ──────────────────────────────
cargo run                         # API on localhost:8080
cargo check
cargo clippy -- -D warnings
cargo fmt --all
cargo test

# ── Database ─────────────────────────────────────────────────────────────
docker compose up -d postgres
cargo sqlx migrate run            # from apps/api/
cargo sqlx migrate revert

# ── Contracts (infra/contracts) ──────────────────────────────────────────
forge build
forge test

# ── shadcn/ui ────────────────────────────────────────────────────────────
# Avoid for new components — prefer packages/ui/ neo-brutalism primitives.
# Only used to scaffold something quickly, then restyled.
```

> `pnpm dev` only starts the Next.js frontend. The Rust API is not a pnpm workspace and must be started separately with `cargo run` from `apps/api/`.

## Quality gates

| Layer       | Local pre-commit (Lefthook)       | CI (blocking)                                                                                  | CI (advisory)                    |
| ----------- | --------------------------------- | ---------------------------------------------------------------------------------------------- | -------------------------------- |
| Format      | `prettier --write {staged_files}` | `prettier --check`                                                                             | —                                |
| Frontend    | —                                 | `next lint` · `tsc --noEmit` · `vitest run` · `next build`                                     | `vitest run --coverage` · `knip` |
| Backend     | —                                 | `cargo fmt --check` · `cargo clippy --all-targets -- -D warnings` · `cargo test --all-targets` | `cargo llvm-cov`                 |
| Deps        | —                                 | `cargo-audit` · `cargo-deny check` · `cargo-machete`                                           | —                                |
| Spelling    | —                                 | `typos` (crate-ci/typos action)                                                                | —                                |
| Commit msg  | `commitlint --edit {1}`           | `commitlint` job on PRs                                                                        | —                                |
| Branch name | `scripts/check-branch-name.sh`    | `branch-name` job on PRs                                                                       | —                                |

Tooling stack: **Lefthook** (hooks runner) · **commitlint** · **Prettier** · **Vitest** + **@vitest/coverage-v8** · **knip** (unused TS) · **typos** (spell check) · **cargo-llvm-cov** (Rust coverage) · **cargo-audit** · **cargo-deny** (config: `apps/api/deny.toml`) · **cargo-machete** (unused deps).

Bypass any local hook: `git commit --no-verify` or `git push --no-verify` — use sparingly.

## Feature flags

Every flag defaults `false` / mock-on so `main` stays trunk-shippable. Real-execution paths require both the runtime flag _and_ (for chain calls) a matching cargo `--features` build. Source of truth: `apps/api/src/config.rs` + `Config::validate()`.

| Env var                   | Default | Cargo feature | Depends on                | What flipping it does                                                                 |
| ------------------------- | ------- | ------------- | ------------------------- | ------------------------------------------------------------------------------------- |
| `EXECUTION_MOCK`          | `true`  | —             | —                         | When `false`: rebalance executor + treasury skip mocks and use real RPC + signers.    |
| `MOCK_CIRCLE`             | `true`  | —             | —                         | When `false`: Circle Wallets, Gateway, Paymaster, FX hit live Circle APIs.            |
| `BILLING_V2_ENABLED`      | `false` | —             | —                         | Real Nanopayments fee settle/refund, referral payouts, subscription tier gating.      |
| `AUM_STREAM_ENABLED`      | `false` | —             | `BILLING_V2_ENABLED=true` | Nightly AUM-fee accrual ticker — premature pre-revenue.                               |
| `REGIME_BACKTEST_ENABLED` | `false` | —             | —                         | Mounts `/about/regime/backtest` reading the precomputed 5y backtest.                  |
| `PEG_DEFENSE_ENABLED`     | `false` | —             | —                         | Peg monitor proposes a defensive rebalance plan when a stable depegs.                 |
| `TAX_EXPORT_V1_ENABLED`   | `false` | —             | —                         | Mounts `/tax/export.csv` for the 1099-DA per-wallet basis export.                     |
| `CALIBRATED_CONF_ENABLED` | `false` | —             | ≥50 real decisions        | Surfaces calibrated confidence instead of raw model confidence in the approval modal. |
| `CONSTITUTION_ENABLED`    | `false` | —             | —                         | Constitution evaluator runs at decision-time, vetoes off-policy plans.                |
| —                         | —       | `real-cctp`   | `EXECUTION_MOCK=false`    | Compile in alloy + CCTP V2 sol! interfaces. Without this, cross-chain legs no-op.     |
| —                         | —       | `real-usyc`   | `EXECUTION_MOCK=false`    | Compile in Hashnote Teller mint/redeem. Without this, USYC park/redeem are mock.      |

Build matrix recipes:

```bash
# Hermetic default — every flag off, no chain code compiled.
cargo run

# Real CCTP V2 only (cross-chain works, USYC park stays mocked).
cargo run --features real-cctp

# Full real-exec build for first-paid-user readiness.
cargo run --features "real-cctp real-usyc"
```

The runtime-flag matrix lives in `.env.local` (per-developer overrides); the committed `.env` keeps mocks on. Validation runs at boot — flipping `BILLING_V2_ENABLED=true` without the required addresses will fail-fast in `Config::validate()`.

## Architecture

### Frontend (`apps/web`)

- **App Router** — `(app)/` for the authenticated shell; `/explore/:portfolioId` for read-only demo mode (no wallet); `/diary/:wallet` for the public agent diary.
- **State:** Zustand (`stores/portfolio.ts`) for domain state; React Query for server-fetched data.
- **Realtime:** native `EventSource` via `lib/sse.ts` `useEventSource()` hook. Event types: `price.tick`, `regime.flip`, `agent.decision`, `rebalance.status`, `gateway.balance`.
- **Mock layer:** `src/lib/mock-data.ts` seeds the store via `components/providers.tsx` — used by `/explore` demo mode.
- **Design:** dark neo-brutalism, dual-accent (green = PnL/money, cyan = agent activity). Tokens in `apps/web/src/app/globals.css` + `packages/config/tailwind/`. Primitives in `packages/ui/`. Strict separation rule: green never appears in agent surfaces; cyan never in PnL numbers. See [`docs/04-design-system.md`](./docs/04-design-system.md).
- **Trust signals on every screen:** data provenance · chain badges · USDC fee preview · model slug · confidence bar.

### Backend (`apps/api`)

Module-per-domain, single binary:

```
src/
  main.rs           tracing init, DB connect, migrate, serve
  config.rs         typed env (Config::from_env) — OPENROUTER + CIRCLE keys
  error.rs          AppError — unified IntoResponse
  db.rs             PgPool setup
  router.rs         Axum router + AppState
  middleware/
    auth.rs         JWT extraction → Claims extension
  modules/
    auth/           wallet-create + passkey login (Circle Wallets)
    portfolio/      CRUD + goals (JSONB)
    market_data/    CoinGecko price fetch + snapshot
    agent/          strategist + critic pipeline + memory retrieval
    rebalance/      local + cross_chain.rs (CCTP V2 + Hooks)
    risk_engine/    concentration + vol + drift + regime.rs
    sse/            /sse event stream (axum::response::sse)
    ai/             OpenRouterClient + ModelRoute enum
    wallet/         Circle Wallets (modular MSCA) REST wrapper
    gateway/        Unified USDC balance across chains
    yield/          USDC ↔ USYC (atomic API)
    fx/             Arc StableFX (USDC ↔ EURC)
    tax/            Cost-basis lots, harvestable losses
    billing/        Nanopayments — protocol fees + referral payouts
    strategies/     (conditional) marketplace
```

### Agent flow

```
Trigger (user · scheduler · drift · regime flip)
  → regime classifier (Haiku) → MarketRegime
  → strategist (Opus) reads goal + regime + memory + prices
                + harvestable losses + USYC rate + EURC basis
  → critic (GPT-5) adversarial pass; strategist may revise once
  → store proposal in agent_decisions (model_slug + confidence)
  → SSE push agent.decision
  → user reviews single approval modal (USDC fee preview)
  → executor: Gateway delta plan → CCTP V2 + Hook swaps on dest chain
  → Paymaster pays gas in USDC
  → SSE push rebalance.status per leg
  → 24h later: outcome compressed into agent_memory
```

### Database

Migrations:

- `0001_initial.sql` — users, portfolios, allocations, assets, agent_decisions, rebalance_events, market_snapshots
- `0002_cost_basis_and_wallets.sql` — `wallet_id` + `arc_address` + `base_address` on users; `cost_basis_lots`; `agent_memory`; extended `agent_decisions` with `model_slug` + `prompt_tokens` + `latency_ms`; `strategies` (conditional)

`updated_at` is maintained by the `set_updated_at()` trigger.

### Shared types (`packages/shared`)

`src/types.ts` is the contract between frontend and backend. Adds for this build: `MarketRegime`, `WalletInfo`, `CrossChainRoute`, `TaxLot`, `ModelRoute`, `SseEvent`. `src/constants.ts` holds Arc + Base addresses, USYC token address, Paymaster addresses, Circle API URLs.

## Git workflow

Branch naming, commit format, and PR checklist live in [`CONTRIBUTING.md`](./CONTRIBUTING.md). In short:

- Branches: `feat/<slug>` · `fix/<slug>` · `docs/<slug>` · `chore/<slug>` · `refactor/<slug>` · `ci/<slug>` · `test/<slug>` · `perf/<slug>` · `build/<slug>`. Enforced locally by Lefthook `pre-push` and in CI by `branch-name` job.
- Commits: **Conventional Commits**. Enforced by Lefthook `commit-msg` and in CI by `commitlint` job. Config: [`commitlint.config.cjs`](./commitlint.config.cjs).
- **Do not** add `Co-authored-by:` trailers for AI tools or `Made-with:` footers anywhere.

```
feat(agent): add critic pass with gpt-5 routing
feat(sse): replace ws with axum::response::sse stream
feat(fx): add Arc StableFX USDC↔EURC module
fix(gateway): correct unified-balance polling on chain switch
chore(ci): cache Foundry artifacts
docs: add 02-agent-design and 04-design-system
```

## Conventions

- **No comments unless the WHY is non-obvious.** Names + types do the work.
- **No premature abstraction.** A bug fix doesn't need a refactor; three similar lines beat a wrapper.
- **Trust internal code.** Validate at boundaries (user input, external APIs), nowhere else.
- **No backwards-compat shims.** This is a hackathon repo; delete the old code, don't dual-path it.
- **Every agent decision must surface its `model_slug` in the UI.** This is both a trust signal and a debugging aid.
- **Every external value must show provenance** (`via CoinGecko · 2.1s ago`). Trust signals are non-negotiable per [`docs/04-design-system.md`](./docs/04-design-system.md).
