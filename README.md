# Aegis — Adaptive Portfolio Harness for Stablecoin-Native Finance

> **The user steers, the agent executes.** A goal-based crypto portfolio manager that reads market regime, proposes rebalances, and settles them across **Arc + Base** in **USDC** through Circle's stack. Every decision is approved by a human in one screen.

> **Hackathon:** RFB 04 — Adaptive Portfolio Manager · [Canteen × Circle Agora Agents](https://agora.thecanteenapp.com/) · May 11–25, 2026.

```
              ┌──────────────────────────────────────────────┐
              │  TRIGGERS  user · scheduler · drift · regime │
              └────────────────────┬─────────────────────────┘
                                   │
              ┌────────────────────▼─────────────────────────┐
              │  REGIME CLASSIFIER  (haiku-4-5)              │
              │  vol · correlation · drawdown                │
              │  → RiskOn / Neutral / RiskOff                │
              └────────────────────┬─────────────────────────┘
                                   │
              ┌────────────────────▼─────────────────────────┐
              │  STRATEGIST  (opus-4-7)                       │
              │  goal + regime + memory + prices + tax       │
              │  + USYC rate + EURC basis → proposal         │
              └────────────────────┬─────────────────────────┘
                                   │
              ┌────────────────────▼─────────────────────────┐
              │  CRITIC  (gpt-5)                             │
              │  adversarial pass → revisions                │
              └────────────────────┬─────────────────────────┘
                                   │
              ┌────────────────────▼─────────────────────────┐
              │  HUMAN APPROVES  one screen, USDC fee preview│
              └────────────────────┬─────────────────────────┘
                                   │
              ┌────────────────────▼─────────────────────────┐
              │  EXECUTOR  Gateway → CCTP V2 + Hook swaps    │
              │  Paymaster pays gas in USDC                  │
              └──────────────────────────────────────────────┘
```

## Stack

| Layer       | Choice                                                                                          |
| ----------- | ----------------------------------------------------------------------------------------------- |
| Frontend    | Next.js 15 · TypeScript · Tailwind · neo-brutalism dark · Zustand · React Query · `EventSource` |
| Backend     | Rust · Axum · Tokio · SQLx · PostgreSQL · **SSE**                                               |
| AI          | **OpenRouter** with per-task model routing (Opus / Sonnet / Haiku / Gemini Flash / GPT-5)       |
| Settlement  | **Arc** (primary) + **Base** (CCTP V2 cross-chain)                                              |
| Wallets     | **Circle Wallets** (modular MSCA — no seed phrase)                                              |
| Cross-chain | **Gateway** unified balance + **CCTP V2** Fast Transfer + Hooks                                 |
| Yield       | **USYC** (tokenized US T-bills) — risk-off sleeve                                               |
| FX          | **Arc StableFX** — USDC↔EURC for the EUR sleeve                                                 |
| Fees        | **Circle Paymaster** (gas in USDC) + **Nanopayments** (protocol fees)                           |
| Infra       | Turborepo · pnpm · Docker · GitHub Actions                                                      |

## Project docs

The engineering writeup lives in [`docs/`](./docs/) — seven short essays in the OpenAI "harness engineering" style:

1. [`00-overview.md`](./docs/00-overview.md) — what Aegis is and the constraint
2. [`01-architecture.md`](./docs/01-architecture.md) — the agent loop and module map
3. [`02-agent-design.md`](./docs/02-agent-design.md) — multi-model routing, prompt files, the "map not manual" rule
4. [`03-circle-stack.md`](./docs/03-circle-stack.md) — how each Circle product earns its place
5. [`04-design-system.md`](./docs/04-design-system.md) — neo-brutalism tokens and the two-accent rule
6. [`05-open-questions.md`](./docs/05-open-questions.md) — the honest unsolved list
7. [`06-harness.md`](./docs/06-harness.md) — Claude Code harness setup for this repo (skills, subagents, hooks)

## Monorepo

```
aegis/
├── apps/
│   ├── web/                       # Next.js 15 frontend
│   │   └── src/
│   │       ├── app/               # App Router (incl. /explore demo mode, /diary/[wallet])
│   │       ├── components/        # dashboard · agent · portfolio · onboarding · ui
│   │       ├── lib/               # api client, sse hook, mock data
│   │       └── stores/            # Zustand
│   └── api/                       # Rust Axum backend
│       └── src/modules/
│           ├── ai/                # OpenRouter client + ModelRoute
│           ├── agent/             # strategist + critic pipeline + memory
│           ├── risk_engine/       # concentration · vol · drift · regime.rs
│           ├── wallet/            # Circle Wallets (modular MSCA)
│           ├── gateway/           # Unified USDC balance
│           ├── rebalance/         # cross_chain.rs (CCTP V2 + Hooks)
│           ├── yield/             # USYC park / redeem
│           ├── fx/                # Arc StableFX (USDC↔EURC)
│           ├── tax/               # Cost-basis lots, harvestable losses
│           ├── sse/               # /sse event stream
│           └── strategies/        # (conditional) marketplace
├── packages/
│   ├── shared/                    # Shared TS types + chain constants
│   ├── ui/                        # Neo-brutalism primitives
│   └── config/                    # ESLint · TS · Tailwind
├── infra/
│   ├── contracts/                 # RebalanceExecutor.sol (Foundry)
│   └── docker/                    # Dockerfiles
└── docs/                          # Project documentation (see above)
```

## Quick start

### Prereqs

- Node 20+, pnpm 9+
- Rust 1.88+
- Docker + Docker Compose
- An **OpenRouter** API key
- A **Circle developer** API key (Wallets, Gateway, USYC, Paymaster)

### Install + run

```bash
cp .env.example .env
# Set at minimum: DATABASE_URL · JWT_SECRET · OPENROUTER_API_KEY · CIRCLE_API_KEY
pnpm install

docker compose up -d postgres redis

cd apps/api && cargo sqlx migrate run && cd ../..

pnpm dev          # Next.js (3000) — Rust API runs separately:
# in another shell:
cd apps/api && cargo run     # Axum on 8080
```

| Service  | URL                          |
| -------- | ---------------------------- |
| Frontend | http://localhost:3000        |
| API      | http://localhost:8080        |
| SSE      | http://localhost:8080/sse    |
| Health   | http://localhost:8080/health |

### Demo mode (no wallet, no backend)

```bash
cd apps/web && pnpm dev
```

Visit http://localhost:3000/explore — the dashboard hydrates from `mock-data.ts` so you can navigate the UI without onboarding.

## API surface

```
GET  /health
POST /auth/wallet/create              # Create Circle Wallet for new user
POST /auth/wallet/login               # Passkey login

GET  /portfolios                      # List portfolios
POST /portfolios                      # Create portfolio (with goal)
GET  /portfolios/:id                  # Portfolio + allocations + Gateway balance
PUT  /portfolios/:id                  # Update goal / allocation targets
DEL  /portfolios/:id

POST /portfolios/:id/analyze          # Trigger agent (regime + strategist + critic)
POST /portfolios/:id/approve/:decisionId  # Execute approved rebalance

GET  /agent/decisions/:portfolio_id   # Decision history (with model + confidence)
GET  /agent/diary/:wallet             # Public agent diary

GET  /market/snapshot
GET  /market/prices
GET  /yield/usyc/rate
GET  /fx/usdc-eurc

GET  /sse                             # Server-sent events stream
                                      # events: price.tick · regime.flip
                                      # · agent.decision · rebalance.status
                                      # · gateway.balance
```

## Agent flow

See [`docs/01-architecture.md`](./docs/01-architecture.md) for the full diagram. In short:

1. **Trigger** — user, scheduler (5 min), drift > θ, or regime flip.
2. **Regime** — Haiku classifier turns vol + correlation + drawdown into `RiskOn` / `Neutral` / `RiskOff`.
3. **Strategist** — Opus reads goal + regime + memory + prices + tax + USYC + EURC → proposal.
4. **Critic** — GPT-5 challenges the proposal; strategist gets one revision.
5. **Approve** — single user-facing modal with USDC fee preview and model slug.
6. **Execute** — Gateway delta plan → CCTP V2 + Hook swaps → Paymaster.
7. **Observe** — `agent_decisions`, `rebalance_events`, `agent_memory`; SSE pushes every step.

## Database schema (after `0002_cost_basis_and_wallets.sql`)

```sql
users               -- email, wallet_id, arc_address, base_address, risk_tolerance
portfolios          -- user_id, goal (JSONB), total_value_usd, risk_score
assets              -- symbol, name, coingecko_id, chain
allocations         -- portfolio_id, symbol, quantity, target_weight, current_weight
cost_basis_lots     -- allocation_id, acquired_at, quantity, basis_usd
agent_decisions     -- portfolio_id, model_slug, prompt_tokens, latency_ms,
                    -- regime, confidence, reasoning, recommendation (JSONB)
agent_memory        -- portfolio_id, decision_id, outcome_24h (JSONB)
rebalance_events    -- decision_id, status, chain, tx_hash (per leg)
market_snapshots    -- assets (JSONB), fear_greed_index, btc_dominance, regime
strategies          -- (conditional) author, name, rules (JSONB), royalty_bps
```

## Environment variables

| Variable              | Description                                    |
| --------------------- | ---------------------------------------------- |
| `DATABASE_URL`        | PostgreSQL connection string                   |
| `JWT_SECRET`          | Secret for signing JWTs (32+ chars)            |
| `OPENROUTER_API_KEY`  | OpenRouter API key (replaces `OPENAI_API_KEY`) |
| `OPENROUTER_BASE_URL` | `https://openrouter.ai/api/v1`                 |
| `CIRCLE_API_KEY`      | Circle developer key (Wallets, Gateway, USYC)  |
| `CIRCLE_ENV`          | `sandbox` (default) or `production`            |
| `ARC_RPC_URL`         | Arc testnet RPC                                |
| `BASE_RPC_URL`        | Base Sepolia RPC                               |
| `POSTHOG_KEY`         | Traction analytics                             |

See `.env.example` for the full list.

## Development

```bash
pnpm dev              # Next.js (Rust API runs separately via cargo run)
pnpm build            # Build all apps
pnpm lint             # ESLint
pnpm type-check       # tsc --noEmit
pnpm clean            # Clean build artifacts

# Database
docker compose up -d postgres
cd apps/api && cargo sqlx migrate run

# shadcn/ui (replaced by neo-brutalism primitives in packages/ui — use sparingly)
cd apps/web && pnpm dlx shadcn@latest add <component>

# Rust
cd apps/api
cargo check
cargo clippy -- -D warnings
cargo fmt --all
cargo test

# Solidity (RebalanceExecutor)
cd infra/contracts && forge build && forge test
```

## Deployment

**Frontend → Vercel.** Set `NEXT_PUBLIC_API_URL` and `NEXT_PUBLIC_SSE_URL`.

**Backend → Railway / Fly.io / Docker.**

```bash
docker build -f infra/docker/Dockerfile.api -t aegis-api apps/api/
docker run -p 8080:8080 --env-file .env aegis-api
```

**Contracts → Arc testnet + Base Sepolia.**

```bash
cd infra/contracts
forge script script/Deploy.s.sol --rpc-url $ARC_RPC_URL --broadcast
forge script script/Deploy.s.sol --rpc-url $BASE_RPC_URL --broadcast
```

---

Built for hackathon velocity. Scale what works, cut what doesn't. **Read [`docs/`](./docs/) before contributing.**
