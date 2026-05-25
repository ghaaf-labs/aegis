# CLAUDE.md · AGENTS.md

Guidance for AI coding agents (Claude Code, Codex, …) working on Aegis. **`AGENTS.md` is a symlink to this file — keep them one file.**

**Read [`docs/`](./docs/) first** — especially [`docs/06-harness.md`](./docs/06-harness.md) for how this repo uses agent skills, subagents, and hooks. Branch / commit / CI / coverage: [`CONTRIBUTING.md`](./CONTRIBUTING.md). Env-var hygiene (load order, secrets-vs-public split, pitfalls): [`docs/09-env-hygiene.md`](./docs/09-env-hygiene.md) before editing `.env` / `.env.local`. Running the servers: [`docs/14-dev-runtime.md`](./docs/14-dev-runtime.md). What "good code" means here: [`docs/15-quality-bar.md`](./docs/15-quality-bar.md).

## How to work here

These reduce the common LLM coding mistakes. They bias toward caution over speed — use judgement on trivial tasks.

1. **Think before coding.** State your assumptions; if uncertain, ask. If multiple interpretations exist, surface them — don't silently pick one. If a simpler approach exists, say so and push back when warranted. When something is unclear, stop and name what's confusing rather than guessing.
2. **Simplicity first.** Minimum code that solves the problem, nothing speculative. No features beyond the ask, no abstraction for single-use code, no "flexibility"/config nobody requested, no error handling for impossible states. Three similar lines beat a premature wrapper (abstract on the third occurrence, not the second). If 200 lines could be 50, rewrite it.
3. **Surgical changes.** Touch only what the task requires. Don't reformat or "improve" adjacent code, don't refactor what isn't broken, match existing style even if you'd do it differently. Remove only the imports/symbols _your_ change orphaned; flag pre-existing dead code, don't delete it unasked. Every changed line should trace to the request.
4. **Goal-driven execution.** Turn the task into a verifiable goal and loop until met: "fix the bug" → write a failing test, then make it pass; "add validation" → test the invalid inputs, then make them pass; "refactor X" → tests green before and after. State a short plan for multi-step work and verify each step.

> Working if: smaller diffs, fewer rewrites from overcomplication, and clarifying questions come _before_ implementation rather than after mistakes.

Aegis-specific conventions (trust signals, the dual-accent rule, `model_slug`) are under [Conventions](#conventions). The quality bar (complexity / size / duplication thresholds, abstraction & data-flow rules) is [`docs/15-quality-bar.md`](./docs/15-quality-bar.md).

## Dev runtime — never bare `cargo run` / `pnpm dev`

Several agents share this repo at once. Start servers **only** through the supervisor, so they outlive the tool call, any agent can restart/tail them, and they don't orphan on `:8080` / `:3000`:

```bash
scripts/dev.sh up           # ensure api (:8080) + web (:3000) running (idempotent, health-checked)
scripts/dev.sh status       # what's running, ports, who owns it
scripts/dev.sh logs api     # tail recent output without attaching
scripts/dev.sh restart api  # restart in place after an edit (the hand-off op)
scripts/dev.sh claim "…"    # advisory: tell other agents you're working
scripts/dev.sh down         # stop both, free the ports
```

A linked **worktree** gets deterministic offset ports automatically (`scripts/dev.sh ports`); the main checkout owns `:8080`/`:3000`. Full protocol + the lock/heartbeat model: [`docs/14-dev-runtime.md`](./docs/14-dev-runtime.md). Postgres/Redis still come from `make db-up`.

## What this is

**Aegis** is an adaptive crypto portfolio harness for stablecoin-native finance, submitted to **RFB 04: Adaptive Portfolio Manager** at Canteen × Circle's **Agora Agents Hackathon** (May 11–25, 2026). The user steers (sets a goal, approves moves); a multi-model AI agent executes on **Arc + Base** through Circle's stack. Turborepo monorepo: a Next.js 15 frontend (`apps/web`) and a Rust/Axum backend (`apps/api`).

## Locked-in decisions (do not silently change)

| Decision                                                                                   | Rationale                                                     | Doc                                                      |
| ------------------------------------------------------------------------------------------ | ------------------------------------------------------------- | -------------------------------------------------------- |
| **OpenRouter** as the AI gateway (not OpenAI direct)                                       | Per-task model routing — Haiku/Opus/Sonnet/Gemini Flash/GPT-5 | [`docs/02-agent-design.md`](./docs/02-agent-design.md)   |
| **Arc + Base** as the two settlement chains (≥2 required)                                  | Arc for native USDC gas; Base for CCTP V2 + Hooks             | [`docs/00-overview.md`](./docs/00-overview.md)           |
| **SSE** for realtime (`/sse`), not WebSocket                                               | Server→client only; native `EventSource`; trivial proxying    | [`docs/01-architecture.md`](./docs/01-architecture.md)   |
| **Circle stack**: Wallets · Gateway · CCTP V2 · USYC · Paymaster · StableFX · Nanopayments | All required for Circle Tool Usage judging                    | [`docs/03-circle-stack.md`](./docs/03-circle-stack.md)   |
| **EURC sleeve** via Arc StableFX                                                           | Multi-currency portfolio with native FX rails                 | [`docs/03-circle-stack.md`](./docs/03-circle-stack.md)   |
| **Dark neo-brutalism** UI with **dual-accent** (green = money, cyan = agent)               | Premium fintech feel; strict separation rule                  | [`docs/04-design-system.md`](./docs/04-design-system.md) |
| **Real users** (not simulated) for the Traction judging dimension                          | 30% of the score                                              | Plan                                                     |
| **Project docs** in OpenAI "harness engineering" style, condensed (~1.5–2.5k words)        | Communicates engineering rigor to judges                      | [`docs/`](./docs/)                                       |

## Workspace layout

```
apps/
  web/          Next.js 15 — TS · Tailwind · neo-brutalism · Zustand · React Query · EventSource
  api/          Rust Axum — Tokio · SQLx · PostgreSQL · SSE · OpenRouter · Circle SDK
packages/
  shared/       Domain types + chain constants (TS) — the FE/BE contract
  ui/           Neo-brutalism primitives (Card, Button, Pill, ModelBadge, ChainBadge…)
  config/       Shared ESLint, TypeScript, Tailwind configurations
infra/
  contracts/    RebalanceExecutor.sol (Foundry)
  docker/       Dockerfiles for api and web
scripts/        dev.sh (multi-agent runtime) + seed/env helpers
docs/           Project documentation (read first)
```

## Common commands

```bash
pnpm install                      # install all workspace deps
make setup                        # first run: deps + Postgres + migrate

# Dev servers — via the supervisor (see docs/14), not bare commands:
scripts/dev.sh up                 # api (:8080) + web (:3000)
make db-up                        # Postgres + Redis (Docker)

# Frontend (apps/web)
pnpm build · pnpm lint · pnpm type-check

# Backend (from apps/api/)
cargo check · cargo clippy -- -D warnings · cargo fmt --all · cargo test
cargo sqlx migrate run            # apply migrations  (revert: migrate revert)

# Contracts (infra/contracts)
forge build · forge test
```

> `pnpm dev` only starts the Next.js frontend; the Rust API is a separate `cargo` project. `scripts/dev.sh` starts both correctly and is the right entry point when more than one agent is active. Avoid scaffolding new UI with shadcn — prefer the `packages/ui/` neo-brutalism primitives.

**Cargo build matrix** (real-execution paths are feature-gated; default build is hermetic mocks):

```bash
cargo run                                            # every flag off, no chain code compiled
cargo run --features real-cctp                       # cross-chain works, USYC park + swaps stay mocked
cargo run --features "real-cctp real-usyc real-swap" # full real-exec readiness (incl. Base swaps)
```

## Quality bar

Lint is **real config, not prose in this file** (full table + rationale: [`docs/15-quality-bar.md`](./docs/15-quality-bar.md)):

- **Rust** — `apps/api/Cargo.toml [lints]` enables `clippy::pedantic` + `clippy::nursery` as a _ratchet_ (guard-rails for new code; the lints already firing across the tree are allowed back), plus `clippy.toml` thresholds. CI runs `cargo clippy --all-targets -- -D warnings`, so every Rust lint is a hard gate.
- **TypeScript** — complexity / function-length / file-length / nesting / params rules in `apps/web/eslint.config.mjs`, at `warn` (advisory; `next lint` / `next build` fail only on errors). Duplicate code: `pnpm dlx jscpd`.

**To raise the bar:** delete a clippy allow-back or flip a TS `warn` → `error`, then fix the fallout. **Never loosen a gate to make a red build green.**

## Architecture (quick map; detail in `docs/`)

- **Frontend** (`apps/web`) — App Router: `(app)/` authenticated shell; `/explore/:portfolioId` read-only demo (no wallet); `/diary/:wallet` public agent diary. State: Zustand (`stores/portfolio.ts`) + React Query. Realtime: `EventSource` via `lib/sse.ts` (`price.tick`, `regime.flip`, `agent.decision`, `rebalance.status`, `gateway.balance`). Mock layer: `src/lib/mock-data.ts`. Design + the strict green/cyan separation rule: [`docs/04-design-system.md`](./docs/04-design-system.md).
- **Backend** (`apps/api`) — single binary, module-per-domain under `src/modules/` (`auth`, `portfolio`, `market_data`, `agent`, `rebalance`, `risk_engine`, `sse`, `ai`, `wallet`, `gateway`, `yield`, `fx`, `tax`, `billing`). Typed env in `config.rs` (+ `Config::validate()`); unified errors in `error.rs`; router + `AppState` in `router.rs`. SQLx migrations in `apps/api/migrations/`; `updated_at` is trigger-maintained.
- **Agent flow** — trigger → regime classifier (Haiku) → strategist (Opus: goal + regime + memory + prices + harvestable losses + USYC rate + EURC basis) → adversarial critic (GPT-5), strategist may revise once → store proposal in `agent_decisions` (`model_slug` + confidence) → SSE `agent.decision` → user approves (USDC fee preview) → executor: Gateway delta → CCTP V2 + Hook swaps, Paymaster pays gas in USDC → SSE `rebalance.status` per leg → 24h later the outcome compresses into `agent_memory`. Detail: [`docs/02-agent-design.md`](./docs/02-agent-design.md).
- **Shared contract** — `packages/shared/src/types.ts` (`MarketRegime`, `WalletInfo`, `CrossChainRoute`, `TaxLot`, `ModelRoute`, `SseEvent`, …) and `constants.ts` (Arc/Base addresses, USYC, Paymaster, Circle URLs).

## Feature flags

Most flags default `false` / mock-on so `main` stays trunk-shippable (the `EXECUTION_MOCK` / `MOCK_CIRCLE` "real-by-default" pair and `BILLING_V2_ENABLED` are the exceptions — see the rows below). Real-execution paths require the runtime flag **and** (for chain calls) a matching cargo `--features` build. Source of truth: `apps/api/src/config.rs` + `Config::validate()` (fail-fast at boot).

| Env var                   | Default | Cargo feature | Depends on                | What flipping it does                                                                                                                                                                                                                                         |
| ------------------------- | ------- | ------------- | ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `EXECUTION_MOCK`          | `false` | —             | —                         | **Real by default.** `true` → mock adapters (tests/CI/offline). `false` → real RPC + signers; route registry mints an `ExecutionTicket` per leg or fails closed.                                                                                              |
| `MOCK_CIRCLE`             | `false` | —             | —                         | **Real by default.** `true` → in-process mock Circle. `false` → live Wallets/Gateway/Paymaster.                                                                                                                                                               |
| `USYC_ENABLED`            | `false` | —             | —                         | Kill-switch for the USYC park/redeem sleeve. While `false`, USYC is Track-only (Teller is allowlist/KYB-gated).                                                                                                                                               |
| `BILLING_V2_ENABLED`      | `true`  | —             | —                         | **On by default.** Real Nanopayments fee settle/refund, referral payouts, subscription tier gating; un-subscribed users resolve to `Tier::Free` (5 decisions/mo, 1 portfolio). Boot fails if the seller/treasury addresses are unset. Set `false` to opt out. |
| `AUM_STREAM_ENABLED`      | `false` | —             | `BILLING_V2_ENABLED=true` | Nightly AUM-fee accrual ticker.                                                                                                                                                                                                                               |
| `REGIME_BACKTEST_ENABLED` | `false` | —             | —                         | Mounts `/about/regime/backtest` (precomputed 5y backtest).                                                                                                                                                                                                    |
| `PEG_DEFENSE_ENABLED`     | `true`  | —             | —                         | Peg monitor proposes a defensive plan on depeg. Auto-execute (Pro) stays gated until F-PEG-8.                                                                                                                                                                 |
| `TAX_EXPORT_V1_ENABLED`   | `true`  | —             | —                         | Mounts `/tax/export.csv` (1099-DA per-wallet basis export).                                                                                                                                                                                                   |
| `CALIBRATED_CONF_ENABLED` | `false` | —             | ≥50 real decisions        | Surfaces calibrated confidence instead of raw model confidence in the approval modal.                                                                                                                                                                         |
| `CONSTITUTION_ENABLED`    | `false` | —             | —                         | Constitution evaluator runs at decision-time, vetoes off-policy plans.                                                                                                                                                                                        |
| —                         | —       | `real-cctp`   | `EXECUTION_MOCK=false`    | Compile alloy + CCTP V2 `sol!` interfaces; without it cross-chain legs fail closed (`REAL_CCTP_FEATURE`).                                                                                                                                                     |
| —                         | —       | `real-usyc`   | `EXECUTION_MOCK=false`    | Compile Hashnote Teller mint/redeem; without it (and `USYC_ENABLED=true`) USYC fails closed.                                                                                                                                                                  |
| —                         | —       | `real-swap`   | `EXECUTION_MOCK=false`    | Compile the real Uniswap V3 (Base Sepolia) swap adapter; without it USDC↔token swaps fail closed (`REAL_SWAP_FEATURE`).                                                                                                                                       |

## Conventions

- **Comments only when the WHY is non-obvious.** Names + types do the work.
- **Validate at boundaries** (user input, external APIs), trust internal code elsewhere.
- **No backwards-compat shims** — hackathon repo; delete old code, don't dual-path it.
- **Every agent decision surfaces its `model_slug` in the UI** — trust signal + debugging aid.
- **Every external value shows provenance** (`via CoinGecko · 2.1s ago`). Non-negotiable per [`docs/04-design-system.md`](./docs/04-design-system.md).
- **Money is `Decimal`, never `f64`.** SSE is server→client only.

## Git workflow

Full rules in [`CONTRIBUTING.md`](./CONTRIBUTING.md). In short:

- Branches: `feat/` · `fix/` · `docs/` · `chore/` · `refactor/` · `ci/` · `test/` · `perf/` · `build/<slug>` (enforced by Lefthook `pre-push` + CI `branch-name`).
- Commits: **Conventional Commits** (Lefthook `commit-msg` + CI `commitlint`; config `commitlint.config.cjs`).
- **Do not** add `Co-authored-by:` trailers for AI tools or `Made-with:` footers anywhere. Plan-step labels (e.g. `F-EXEC-1c`) go in commit messages / `docs/05-open-questions.md`, never in filenames or code comments.

```
feat(agent): add critic pass with gpt-5 routing
feat(fx): add Arc StableFX USDC↔EURC module
fix(gateway): correct unified-balance polling on chain switch
```
