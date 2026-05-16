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
-- Prerequisite tables (`plan_tiers`, `subscriptions`, `invoices`) are owned
-- by A2's 0010_subscriptions.sql which always runs first. The scaffolding
-- block that originally lived here was dropped at merge time because it
-- conflicted with A2's `plan_tiers(code)` PK.

-- ── AUM accruals ──────────────────────────────────────────────────────────
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
