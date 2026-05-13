# Aegis — AI-Powered Adaptive Crypto Portfolio Manager

> Autonomously monitors market conditions, evaluates risk, rebalances your portfolio, and explains every decision in plain English.

```
                        ┌─────────────────────────────────┐
                        │         Market Signals          │
                        │  (CoinGecko · Fear/Greed · WS)  │
                        └──────────────┬──────────────────┘
                                       │
                        ┌──────────────▼──────────────────┐
                        │         Risk Engine              │
                        │  Drift · Concentration · Vol    │
                        └──────────────┬──────────────────┘
                                       │
                        ┌──────────────▼──────────────────┐
                        │         AI Agent (GPT-4o)        │
                        │  Reasoning · Recommendation     │
                        └──────────────┬──────────────────┘
                                       │
                        ┌──────────────▼──────────────────┐
                        │       Portfolio Decision         │
                        │  User approves → Execute        │
                        └─────────────────────────────────┘
```

## Stack

| Layer     | Tech                                                  |
|-----------|-------------------------------------------------------|
| Frontend  | Next.js 15 · TypeScript · Tailwind · shadcn/ui · Zustand · React Query · Framer Motion |
| Backend   | Rust · Axum · Tokio · SQLx · PostgreSQL · WebSocket  |
| AI        | OpenAI GPT-4o · modular agent layer                  |
| Infra     | Turborepo · pnpm workspaces · Docker · GitHub Actions |

## Monorepo Structure

```
aegis/
├── apps/
│   ├── web/                    # Next.js 15 frontend
│   │   └── src/
│   │       ├── app/            # App Router pages
│   │       ├── components/     # UI components
│   │       ├── lib/            # API client, mock data, utils
│   │       ├── hooks/          # Custom hooks (WebSocket, etc.)
│   │       └── stores/         # Zustand stores
│   └── api/                    # Rust Axum backend
│       ├── src/
│       │   ├── modules/        # auth · portfolio · agent · market_data
│       │   │                   # rebalance · websocket · ai · risk_engine
│       │   ├── middleware/     # JWT auth
│       │   ├── router.rs       # Axum router
│       │   ├── config.rs       # Env config
│       │   └── error.rs        # Unified error type
│       └── migrations/         # PostgreSQL migrations
├── packages/
│   ├── shared/                 # Shared TypeScript types
│   ├── ui/                     # Shared UI primitives
│   └── config/                 # ESLint · TypeScript · Tailwind configs
├── infra/
│   └── docker/                 # Dockerfiles for API and web
├── .github/workflows/          # CI/CD
├── docker-compose.yml          # Local dev: postgres + redis + api
└── .env.example
```

## Quick Start

### Prerequisites

- Node.js 20+
- pnpm 9+
- Rust 1.82+
- Docker + Docker Compose
- An OpenAI API key

### 1. Install

```bash
cp .env.example .env
# Edit .env — set OPENAI_API_KEY and JWT_SECRET at minimum
pnpm install
```

### 2. Start infrastructure

```bash
docker compose up -d postgres redis
```

### 3. Run migrations

```bash
cd apps/api
cargo sqlx migrate run
```

### 4. Start development servers

```bash
# From root — starts both Next.js and Rust via Turbo
pnpm dev
```

| Service  | URL                    |
|----------|------------------------|
| Frontend | http://localhost:3000  |
| API      | http://localhost:8080  |
| WS       | ws://localhost:8080/ws |
| Health   | http://localhost:8080/health |

### 5. Demo mode

The frontend ships with a complete mock data layer — no backend required to explore the UI:

```bash
cd apps/web
pnpm dev
```

Visit http://localhost:3000 — the dashboard is fully functional with mock portfolio data, AI decisions, and price feeds.

## API Reference

```
GET  /health                          # Service health check
POST /auth/register                   # Register user
POST /auth/login                      # Login
GET  /auth/me                         # Current user (auth required)

GET  /portfolios                      # List portfolios
POST /portfolios                      # Create portfolio
GET  /portfolios/:id                  # Get portfolio with allocations
PUT  /portfolios/:id                  # Update portfolio
DEL  /portfolios/:id                  # Delete portfolio
POST /portfolios/:id/rebalance        # Trigger AI rebalance analysis

GET  /market/snapshot                 # Full market snapshot
GET  /market/prices                   # Asset prices

GET  /agent/decisions/:portfolio_id   # Decision history
POST /agent/analyze                   # Run AI analysis

WS   /ws                              # Real-time price + agent events
```

## Agent Decision Flow

```
1. Trigger (drift · market · scheduled · user)
      │
2. Fetch current portfolio + allocations
      │
3. Fetch live market snapshot (CoinGecko)
      │
4. Risk Engine scores: concentration + volatility + drift
      │
5. Build structured prompt → GPT-4o
      │
6. Parse JSON response: reasoning + trades + confidence
      │
7. Store in agent_decisions table
      │
8. Push to frontend via WebSocket
      │
9. User reviews → approves → rebalance_events created
```

## Database Schema

```sql
users               -- email, password_hash, risk_tolerance
portfolios          -- user_id, total_value_usd, risk_score
assets              -- symbol, name, coingecko_id
allocations         -- portfolio_id, symbol, quantity, target_weight, current_weight
agent_decisions     -- portfolio_id, reasoning, recommendation (JSONB), confidence
rebalance_events    -- portfolio_id, decision_id, status, trades (JSONB)
market_snapshots    -- assets (JSONB), fear_greed_index, btc_dominance
```

## Environment Variables

See `.env.example` for all variables. Required:

| Variable        | Description                          |
|-----------------|--------------------------------------|
| `DATABASE_URL`  | PostgreSQL connection string         |
| `JWT_SECRET`    | Secret for signing JWTs (32+ chars)  |
| `OPENAI_API_KEY`| Your OpenAI API key                  |

## Development Commands

```bash
pnpm dev              # Start all services
pnpm build            # Build all apps
pnpm lint             # Lint all apps
pnpm type-check       # TypeScript check all apps
pnpm clean            # Clean all build artifacts

# Database
docker compose up -d postgres
cargo sqlx migrate run    # from apps/api/
cargo sqlx migrate revert # rollback last migration

# Add shadcn/ui components (from apps/web/)
pnpm dlx shadcn@latest add <component>

# Rust
cargo check           # Fast type check (from apps/api/)
cargo clippy          # Linting
cargo test            # Run tests
```

## Deployment

**Frontend → Vercel**

```bash
cd apps/web
vercel deploy
```

Set env vars in Vercel dashboard: `NEXT_PUBLIC_API_URL`, `NEXT_PUBLIC_WS_URL`

**Backend → Railway / Fly.io / Docker**

```bash
# Docker
docker build -f infra/docker/Dockerfile.api -t aegis-api apps/api/
docker run -p 8080:8080 --env-file .env aegis-api

# Railway: connect repo, set root to apps/api/, set env vars
# Fly.io: fly launch --dockerfile infra/docker/Dockerfile.api
```

## Recommended VS Code Extensions

```json
{
  "recommendations": [
    "rust-lang.rust-analyzer",
    "bradlc.vscode-tailwindcss",
    "esbenp.prettier-vscode",
    "ms-vscode.vscode-typescript-next",
    "biomejs.biome",
    "tamasfe.even-better-toml",
    "usernamehw.errorlens"
  ]
}
```

---

Built for hackathon velocity. Scale what works, cut what doesn't.
