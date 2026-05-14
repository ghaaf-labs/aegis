-- ── 0003 — Wallets, portfolio goals, cost-basis lots ───────────────────────
--
-- Sprint 2 schema. Adds:
--   • wallet columns on `users` (Circle MSCA per user, one address per chain)
--   • `goal` JSONB on `portfolios` for goal-wizard output
--   • `cost_basis_lots` table (seeded for Sprint 3 tax-loss harvester;
--     not written to in Sprint 2 — just present so the agent service can
--     start emitting lots when execution lands)
--
-- Migration policy: every column has a sensible default so existing rows
-- migrate without backfill. `password_hash` is dropped after the wallet
-- module is shipped (S2.3c).

-- ── Wallet columns ─────────────────────────────────────────────────────────
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS wallet_id    TEXT,
    ADD COLUMN IF NOT EXISTS arc_address  TEXT,
    ADD COLUMN IF NOT EXISTS base_address TEXT;

-- Wallet IDs are globally unique — Circle's MSCA addresses are derived from
-- a wallet_id that we own. UNIQUE allows future lookups by wallet.
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_wallet_id
    ON users(wallet_id)
    WHERE wallet_id IS NOT NULL;

-- ── Portfolio goals ────────────────────────────────────────────────────────
-- Stored as JSONB so the goal shape can evolve (e.g. monthly contribution,
-- multi-currency sleeves) without a migration per new field. The strategist
-- reads this and renders it into the prompt's {{ goal_block }} placeholder.
ALTER TABLE portfolios
    ADD COLUMN IF NOT EXISTS goal JSONB NOT NULL DEFAULT '{}'::jsonb;

-- ── Cost-basis lots (Sprint 3 tax harvester precursor) ─────────────────────
-- One row per acquisition lot. Tax-loss harvester (Sprint 3) reads
-- `disposed_at IS NULL` lots and proposes sells that realize losses without
-- violating wash-sale rules (subject to constraints in docs/05-open-questions).
CREATE TABLE IF NOT EXISTS cost_basis_lots (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    allocation_id UUID        NOT NULL REFERENCES allocations(id) ON DELETE CASCADE,
    acquired_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    quantity      DOUBLE PRECISION NOT NULL CHECK (quantity > 0),
    basis_usd     DOUBLE PRECISION NOT NULL CHECK (basis_usd >= 0),
    disposed_at   TIMESTAMPTZ,

    UNIQUE (allocation_id, acquired_at, basis_usd)
);

CREATE INDEX IF NOT EXISTS idx_cost_basis_lots_allocation
    ON cost_basis_lots(allocation_id, acquired_at);

CREATE INDEX IF NOT EXISTS idx_cost_basis_lots_open
    ON cost_basis_lots(allocation_id)
    WHERE disposed_at IS NULL;

-- ── Drop legacy email/password auth (S2.3c) ───────────────────────────────
-- All auth flows through Circle Wallets (passkey + email-OTP) from Sprint 2
-- onward. `email` stays on `users` as the OTP target / display handle, but
-- becomes nullable so passkey-only users aren't forced to provide one.
ALTER TABLE users
    DROP COLUMN IF EXISTS password_hash;

-- ── analytics_events (self-hosted, no PostHog) ─────────────────────────────
-- Captured events: wallet.created, faucet.claimed, goal.completed,
-- analyze.triggered, decision.approved, decision.rejected. Traction
-- numbers come from SQL queries over this table (see docs/queries/traction.sql).
CREATE TABLE IF NOT EXISTS analytics_events (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID        REFERENCES users(id) ON DELETE SET NULL,
    event_name  TEXT        NOT NULL,
    properties  JSONB       NOT NULL DEFAULT '{}'::jsonb,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_analytics_events_user_at
    ON analytics_events(user_id, occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_analytics_events_name_at
    ON analytics_events(event_name, occurred_at DESC);
