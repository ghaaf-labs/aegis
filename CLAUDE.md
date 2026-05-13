# CLAUDE.md

This file provides guidance to Claude Code when working with the Aegis codebase.

## Project Overview

**Aegis** is a full-stack AI-powered crypto portfolio manager. It autonomously monitors market conditions, evaluates portfolio risk and drift, and generates plain-English rebalancing recommendations via a GPT-4o agent. Users stay in control — all trades require explicit approval.

The repository is a **Turborepo monorepo** with a Next.js 15 frontend (`apps/web`) and a Rust/Axum backend (`apps/api`).

## Workspace Layout

```
apps/
  web/          Next.js 15 (App Router) — TypeScript, Tailwind, shadcn/ui, Zustand, React Query
  api/          Rust Axum API — Tokio, SQLx, PostgreSQL, WebSocket, OpenAI client
packages/
  shared/       Domain types shared between frontend and backend (TypeScript)
  ui/           Shared UI primitives (cn helper, base components)
  config/       Shared ESLint, TypeScript, and Tailwind configurations
infra/
  docker/       Dockerfiles for api and web
```

## Common Commands

```bash
# Install all workspace dependencies
pnpm install

# ── Frontend (apps/web) ──────────────────────────────────────────────────
pnpm dev                          # Next.js dev server (localhost:3000)
pnpm build                        # production build
pnpm lint                         # ESLint
pnpm type-check                   # tsc --noEmit

# ── Backend (apps/api — run from apps/api/) ──────────────────────────────
cargo run                         # start API server (localhost:8080)
cargo check                       # fast type-check
cargo clippy -- -D warnings       # lint
cargo fmt --all                   # format
cargo test                        # unit tests

# ── Database ─────────────────────────────────────────────────────────────
docker compose up -d postgres     # start Postgres
cargo sqlx migrate run            # run migrations (from apps/api/)
cargo sqlx migrate revert         # rollback one migration

# ── shadcn/ui (from apps/web/) ───────────────────────────────────────────
pnpm dlx shadcn@latest add <component>
```

> **Note:** `pnpm dev` only starts the Next.js frontend. The Rust API (`apps/api`) is not a pnpm workspace and must be started separately with `cargo run` from its directory.

CI gates: `next lint`, `tsc --noEmit`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.

## Architecture

### Frontend (`apps/web`)

- **App Router** — route groups `(app)/` for the authenticated shell; root `page.tsx` is the public landing page.
- **State:** Zustand (`stores/portfolio.ts`) for domain state; React Query for server-fetched data.
- **Mock layer:** `src/lib/mock-data.ts` seeds the Zustand store via `components/providers.tsx`. The dashboard is fully functional without a running backend — iterate on UI first.
- **Components** follow a domain layout: `components/dashboard/`, `components/agent/`, `components/portfolio/`, `components/onboarding/`, `components/layout/`, `components/ui/`.
- **Types:** `packages/shared` is the source of truth for domain types; `src/types/index.ts` re-exports them.

### Backend (`apps/api`)

Module-per-domain structure, single binary:

```
src/
  main.rs           entry point — tracing init, DB connect, migrate, serve
  config.rs         typed env config (Config::from_env)
  error.rs          AppError — unified IntoResponse error type
  db.rs             PgPool setup
  router.rs         Axum router + AppState (Arc<AppStateInner>)
  middleware/
    auth.rs         JWT extraction → Claims extension
  modules/
    auth/           register, login, JWT minting (argon2 + jsonwebtoken)
    portfolio/      CRUD for portfolios and allocations
    market_data/    CoinGecko price fetch + snapshot
    agent/          AI analysis — prompt build → GPT-4o → JSON parse → DB store
    rebalance/      triggers agent::service::analyze_portfolio for a given portfolio
    websocket/      live price broadcast every 5s over /ws
    ai/             OpenAI chat client (reqwest)
    risk_engine/    concentration + volatility + drift scoring → RiskReport
```

### AI agent flow

```
Trigger (drift / market / scheduled / user)
  → fetch portfolio + allocations (SQLx)
  → fetch market snapshot (CoinGecko)
  → risk_engine::evaluate → RiskReport
  → build structured prompt
  → OpenAI GPT-4o (modules/ai)
  → parse JSON: reasoning + trades + confidence
  → store in agent_decisions (JSONB recommendation)
  → push to frontend via WebSocket
  → user reviews → approves → rebalance_events created
```

### Database

Migration at `apps/api/migrations/0001_initial.sql`. Key tables: `users`, `portfolios`, `allocations`, `assets`, `agent_decisions` (JSONB `recommendation`), `rebalance_events`, `market_snapshots`. `updated_at` is maintained by a `set_updated_at()` trigger.

### Shared types (`packages/shared`)

`src/types.ts` is the domain type contract between frontend and backend. `src/constants.ts` holds supported assets, API route builders, and thresholds. Changes here may require updates in both `apps/web` and `apps/api`.

## Git Workflow

### Branches

Use type-prefixed slugs off `main`: `feat/`, `fix/`, `docs/`, `chore/`.

### Commits

Follow [Conventional Commits](https://www.conventionalcommits.org/): `type[(scope)]: imperative description`

- **Types:** `feat`, `fix`, `docs`, `refactor`, `chore`, `ci`
- **Scope** (optional): `web`, `api`, `shared`, `agent`, `portfolio`, `auth`
- Do **not** add `Co-authored-by:` trailers for AI tools or `Made-with:` footers anywhere.

```
feat(agent): add scheduled drift-threshold trigger
fix(api): correct allocation weight rounding on rebalance
chore(ci): cache Rust build artifacts in GitHub Actions
```
