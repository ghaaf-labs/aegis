-- ── 0015 — AUM-fee streaming accruals (F-AUM-2) ────────────────────────────
--
-- The Pro/Business tiers charge an annual AUM fee (25 bps / 15 bps) that
-- streams continuously via Nanopayments on Arc. The accrual ticker walks
-- every active subscription daily, snapshots AUM, computes
--
--     accrued_usdc = aum × (bps / 10_000) × (Δt_seconds / seconds_per_year)
--
-- and rolls the row into the open invoice for the current monthly period.
-- A unique constraint on (subscription_id, period_start, period_end) makes
-- the daily tick idempotent — a retried tick UPSERTs the same row.
--
-- NOTE for the merge orchestrator: A4 reserves migration **0015** here
-- because A10 already claimed 0014 (tax_share_tokens). Prerequisite tables
-- `subscriptions`, `invoices`, and `plan_tiers` are created by A2's
-- 0010_subscriptions.sql. If A2 has not yet landed when this migration
-- runs, the `CREATE TABLE IF NOT EXISTS` guards below scaffold a minimal
-- compatible shape so this branch stays runnable in isolation; once A2's
-- migration lands these become no-ops.

-- ── Scaffolding prerequisite tables (A2's 0010 will land first in prod) ───
CREATE TABLE IF NOT EXISTS plan_tiers (
    tier              TEXT PRIMARY KEY,
    monthly_usd       NUMERIC NOT NULL,
    aum_annual_bps    INT NOT NULL,
    rebalance_bps     INT NOT NULL,
    decisions_per_mo  INT NOT NULL,
    aum_cap_usd       NUMERIC,
    portfolios_cap    INT
);

INSERT INTO plan_tiers (tier, monthly_usd, aum_annual_bps, rebalance_bps, decisions_per_mo, aum_cap_usd, portfolios_cap)
VALUES
    ('free',     0,    0, 25,    5,  5000, 1),
    ('pro',     19,   25, 15,  240,  NULL, 5),
    ('business', 199, 15, 10, 100000, NULL, NULL)
ON CONFLICT (tier) DO NOTHING;

CREATE TABLE IF NOT EXISTS subscriptions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    tier            TEXT NOT NULL REFERENCES plan_tiers(tier),
    status          TEXT NOT NULL DEFAULT 'active',
    anchor_day      INT NOT NULL DEFAULT 1,
    started_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    canceled_at     TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_subscriptions_user      ON subscriptions(user_id);
CREATE INDEX IF NOT EXISTS idx_subscriptions_status    ON subscriptions(status);

CREATE TABLE IF NOT EXISTS invoices (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    subscription_id UUID NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
    period_start    TIMESTAMPTZ NOT NULL,
    period_end      TIMESTAMPTZ NOT NULL,
    status          TEXT NOT NULL DEFAULT 'open',
    line_items      JSONB NOT NULL DEFAULT '[]'::jsonb,
    subtotal_usdc   NUMERIC NOT NULL DEFAULT 0,
    total_usdc      NUMERIC NOT NULL DEFAULT 0,
    paid_at         TIMESTAMPTZ,
    paid_tx_hash    TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (subscription_id, period_start, period_end)
);
CREATE INDEX IF NOT EXISTS idx_invoices_user      ON invoices(user_id);
CREATE INDEX IF NOT EXISTS idx_invoices_status    ON invoices(status);

-- ── AUM accruals (this migration's actual subject) ────────────────────────
CREATE TABLE IF NOT EXISTS aum_accruals (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id           UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    subscription_id   UUID NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
    invoice_id        UUID REFERENCES invoices(id) ON DELETE SET NULL,
    period_start      TIMESTAMPTZ NOT NULL,
    period_end        TIMESTAMPTZ NOT NULL,
    aum_snapshot_usd  NUMERIC NOT NULL,
    bps               INT NOT NULL,
    accrued_usdc      NUMERIC NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (subscription_id, period_start, period_end)
);

CREATE INDEX IF NOT EXISTS idx_aum_accruals_user_invoice
    ON aum_accruals(user_id, invoice_id);
CREATE INDEX IF NOT EXISTS idx_aum_accruals_subscription
    ON aum_accruals(subscription_id);
