# Aegis

[![CI](https://github.com/ghaaf-labs/aegis/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/ghaaf-labs/aegis/actions/workflows/ci.yml)
[![Agora Agents Hackathon](https://img.shields.io/badge/Agora%20Agents%20Hackathon-2026-cyan?style=flat&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI+PHBhdGggZmlsbD0iI2ZmZiIgZD0iTTEyIDJMMyA3djEwbDkgNSA5LTVWN3oiLz48L3N2Zz4=)](https://agora.thecanteenapp.com/)
[![Powered by Circle](https://img.shields.io/badge/Powered%20by-Circle-00A060?style=flat&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI+PGNpcmNsZSBjeD0iMTIiIGN5PSIxMiIgcj0iMTAiIGZpbGw9IiNmZmYiLz48L3N2Zz4=)](https://www.circle.com/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)

A goal-based crypto portfolio manager for stablecoin-native finance. You set a goal; a multi-model AI agent reads the market regime and rebalances the portfolio on its own, within guardrails, settling in USDC across Arc and Base through Circle's stack. You stay in control: pause the agent or switch it back to approve-every-move from settings whenever you want.

Built for RFB 04 (Adaptive Portfolio Manager) at the [Canteen × Circle Agora Agents hackathon](https://agora.thecanteenapp.com/), May 11–25, 2026.

## What it is

Aegis manages a USDC-denominated portfolio toward a goal you set: an objective, a time horizon, and a risk tolerance. An agent loop classifies the market regime, designs a target allocation, critiques it, and executes the moves across chains. Every decision records the model that produced it, the regime it saw, and its confidence, so you can audit what the agent did and why.

It targets retail and SMB users rather than institutions. Funds are self-custodial, held in your own Circle wallet with no seed phrase, and the Circle stack is wired in end to end: Wallets, Gateway, CCTP V2, USYC, Paymaster, StableFX, and Nanopayments.

## How the agent works

The agent runs on auto-pilot by default. On a trigger (a schedule tick, target drift past a threshold, or a regime change) it classifies the regime, designs a target, runs an adversarial critic over it, checks it against a constitution, then plans and executes the moves.

```
  triggers          user request · scheduler · drift · regime change
     │
     ▼
  regime            Haiku classifier: volatility, correlation, drawdown
                    → RiskOn / Neutral / RiskOff
     │
     ▼
  strategist        Opus: goal + regime + memory + prices + tax lots
                    + USYC rate + EURC basis → proposed allocation
     │
     ▼
  critic            GPT-5 adversarial pass; the strategist may revise once
     │
     ▼
  guardrails        constitution check, single-asset cap, stable-reserve
                    floor, minimum move size, peg defense, route engine
     │
     ├─ auto-pilot on, guardrails clear  → execute
     └─ guardrail trips / auto-pilot off → one-screen review for you
     │
     ▼
  executor          Gateway delta plan → CCTP V2 + hook swaps;
                    Paymaster pays gas in USDC
     │
     ▼
  outcome           24h later the result compresses into agent memory
```

When a guardrail trips (the constitution flags the plan, the route is not executable, a depeg is active, or there is nothing worth moving), the agent stops and leaves a review with the USDC fee shown upfront. Turn auto-pilot off under Settings → Agent control to put every move behind that review, or pause the agent entirely.

The route engine fails closed: if a leg is missing a configured address, a compiled chain feature, or a funded signer, it produces a blocker instead of a fake transaction. So the agent only proposes and executes what it can actually settle.

## Tech stack

| Layer       | Choice                                                                        |
| ----------- | ----------------------------------------------------------------------------- |
| Frontend    | Next.js 15, TypeScript, Tailwind, Zustand, React Query, `EventSource` for SSE |
| Backend     | Rust, Axum, Tokio, SQLx, PostgreSQL, SSE for realtime                         |
| AI          | OpenRouter with per-task model routing (Opus, Sonnet, Haiku, Gemini, GPT-5)   |
| Settlement  | Arc (native USDC gas) and Base (CCTP V2 cross-chain), USDC-denominated        |
| Wallets     | Circle Wallets (modular smart accounts, no seed phrase)                       |
| Cross-chain | Circle Gateway unified balance, CCTP V2 Fast Transfer with hooks              |
| Yield / FX  | USYC (tokenized T-bills) for the risk-off sleeve; Arc StableFX for USDC↔EURC  |
| Fees        | Circle Paymaster (gas in USDC); Nanopayments for the protocol fee             |
| Tooling     | Turborepo, pnpm, Docker, GitHub Actions                                       |

Each Circle product and why it is here: [`docs/03-circle-stack.md`](./docs/03-circle-stack.md).

## Getting started

### Prerequisites

- Node 20+ and pnpm 9+
- Rust 1.88+
- Docker and Docker Compose
- An OpenRouter API key
- A Circle developer API key (Wallets, Gateway, USYC, Paymaster)

### Configure

```bash
cp .env.example .env
```

`.env` holds non-secret defaults and is safe to commit. Put your secrets (API keys, signer keys) in `.env.local`, which is gitignored and loaded ahead of `.env`. At a minimum set `DATABASE_URL`, `JWT_SECRET`, `OPENROUTER_API_KEY`, and `CIRCLE_API_KEY`. The load order and the secret/public split are documented in [`docs/09-env-hygiene.md`](./docs/09-env-hygiene.md).

The backend runs against real RPC and the live Circle API by default. For offline or CI work, set `EXECUTION_MOCK=true` and `MOCK_CIRCLE=true` to swap in in-process mocks.

### Run

```bash
make setup   # install deps, start Postgres + Redis, run migrations
make dev     # start the API (:8080) and web (:3000) via the dev supervisor
```

`make dev` uses a small supervisor so the servers survive a single command and several agents can share the repo. The protocol is in [`docs/14-dev-runtime.md`](./docs/14-dev-runtime.md). Migrations also run on API boot, so a fresh database is set up automatically.

| Service  | URL                            |
| -------- | ------------------------------ |
| Frontend | <http://localhost:3000>        |
| API      | <http://localhost:8080>        |
| SSE      | <http://localhost:8080/sse>    |
| Health   | <http://localhost:8080/health> |

### Demo mode

Visit <http://localhost:3000/explore> for a read-only walkthrough. It hydrates from `apps/web/src/lib/mock-data.ts`, so you can navigate the UI without a wallet or onboarding.

## Project layout

```
aegis/
├── apps/
│   ├── web/                 Next.js 15 frontend
│   │   └── src/
│   │       ├── app/         App Router: authenticated shell, /explore, /diary/[wallet]
│   │       ├── components/  dashboard, agent, portfolio, onboarding
│   │       ├── lib/         API client, SSE hook, mock data
│   │       └── stores/      Zustand
│   └── api/                 Rust Axum backend
│       ├── migrations/      SQLx migrations (0001_baseline + incremental)
│       └── src/
│           ├── domain/      token + chain registry (the single source of truth)
│           ├── config.rs    typed env + feature flags
│           └── modules/     one per domain: agent, rebalance, risk_engine,
│                            wallet, gateway, yield, fx, tax, sse, ai, billing
├── packages/
│   ├── shared/              shared TS types + chain constants (the FE/BE contract)
│   ├── ui/                  neo-brutalism UI primitives
│   └── config/              shared ESLint, TS, Tailwind config
├── infra/
│   ├── contracts/           RebalanceExecutor.sol (Foundry)
│   └── docker/              Dockerfiles
└── docs/                    project documentation
```

## HTTP API

A representative slice; the router is in [`apps/api/src/router.rs`](./apps/api/src/router.rs).

```
GET   /health
POST  /auth/email/start                          request an email login code
POST  /auth/email/verify                          verify the code, open a session

GET   /portfolios                                 list portfolios
POST  /portfolios                                 create a portfolio with a goal
GET   /portfolios/:id                              portfolio, allocations, Gateway balance
PATCH /portfolios/:id/diary-public                 toggle the public agent diary

POST  /agent/analyze                               run the agent (regime + strategist + critic)
POST  /agent/propose-allocation                    propose a target allocation
GET   /agent/decisions/:portfolio_id               decision history (model, regime, confidence)
POST  /agent/decisions/:id/approve-allocation      adopt a proposed allocation
GET   /users/me/agent                              agent status (paused, auto-pilot)
POST  /users/me/agent/auto-pilot                   turn auto-pilot on or off

POST  /portfolios/:id/rebalance/plan               build an execution plan toward the target
POST  /rebalance/:id/execute                       execute an approved plan

GET   /market/snapshot · /market/prices
GET   /treasury/usyc/rate · /fx/usdc-eurc
GET   /diary/wallet/:wallet                        public agent diary

GET   /sse                                         server-sent events:
                                                   price.tick, regime.flip,
                                                   agent.decision, rebalance.status,
                                                   gateway.balance
```

## Data model

Schema lives in [`apps/api/migrations/0001_baseline.sql`](./apps/api/migrations/0001_baseline.sql) plus incremental migrations. The core tables: `users`, `portfolios` (goal as JSONB), `allocations`, `cost_basis_lots`, `agent_decisions` (model slug, regime, confidence, reasoning), `agent_memory` (24h outcome), and `rebalances` with `rebalance_legs` for per-leg execution state. Money columns are PostgreSQL `NUMERIC` mapped to Rust `Decimal`, never floats.

## Documentation

The engineering writeup is in [`docs/`](./docs/), written in a condensed essay style. Start with:

- [`00-overview.md`](./docs/00-overview.md) — what Aegis is and the core constraint
- [`01-architecture.md`](./docs/01-architecture.md) — the agent loop and module map
- [`02-agent-design.md`](./docs/02-agent-design.md) — multi-model routing and prompt design
- [`03-circle-stack.md`](./docs/03-circle-stack.md) — how each Circle product is used
- [`04-design-system.md`](./docs/04-design-system.md) — the neo-brutalism UI and accent rules

The remaining files cover deployment, observability, the outcome policy, auth and onboarding, env hygiene, the dev runtime, and the quality bar.

## Development

```bash
make quality                 # fmt + clippy (Rust), lint + type-check (web)
make db-reset                # wipe and re-migrate the database (destructive)

cd apps/api && cargo test    # Rust tests
cd apps/web && pnpm test     # web tests (Vitest)
cd infra/contracts && forge test
```

The build is feature-gated. The default Cargo build compiles the real Arc and Base execution paths (`real-cctp`, `real-usyc`, `real-swap`); the runtime mock flags above let you run without touching a chain. Branch naming, commits, and the coverage gates are in [`CONTRIBUTING.md`](./CONTRIBUTING.md), and the complexity and duplication thresholds are in [`docs/15-quality-bar.md`](./docs/15-quality-bar.md).

## Contributing

Read [`CONTRIBUTING.md`](./CONTRIBUTING.md) and the [`docs/`](./docs/) first. The repo uses Conventional Commits, branch-name conventions, and a set of CI gates (clippy with warnings denied, type-check, tests, typos, cargo-deny). `CLAUDE.md` (symlinked as `AGENTS.md`) documents how AI coding agents should work in this repo.

## Security

Report vulnerabilities per [`SECURITY.md`](./SECURITY.md). Secrets belong in `.env.local` and never in the repo; the committed `.env` and `.env.example` contain only public addresses and non-secret defaults.

## License

MIT. See [`LICENSE`](./LICENSE). Copyright © 2026 Ghaaf Labs B.V.
