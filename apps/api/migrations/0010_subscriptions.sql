-- ── Subscription / billing v2 schema ───────────────────────────────────────
--
-- Schema-only migration for tiered SaaS billing (Wave 1, agent A2). The
-- runtime that consumes these tables (handlers, middleware, AUM stream)
-- lands in subsequent agents; tables here are always present even with
-- BILLING_V2_ENABLED=false so we never have a partial migration on prod.

-- ── plan_tiers ─────────────────────────────────────────────────────────────
-- Static catalogue of pricing tiers. Seeded once; price changes are a new
-- migration (we never silently rewrite a paying user's tier definition).
CREATE TABLE IF NOT EXISTS plan_tiers (
    code                   TEXT PRIMARY KEY,
    monthly_usd            NUMERIC      NOT NULL,
    aum_cap_usd            NUMERIC,
    portfolios_cap         INT,
    decisions_cap_monthly  INT,
    per_rebalance_bps      INT          NOT NULL,
    aum_annual_bps         INT          NOT NULL
);

INSERT INTO plan_tiers (code, monthly_usd, aum_cap_usd, portfolios_cap, decisions_cap_monthly, per_rebalance_bps, aum_annual_bps)
VALUES
    ('free',     0,   5000, 1,    5,    25, 0),
    ('pro',      19,  NULL, 5,    240,  15, 25),
    ('business', 199, NULL, NULL, NULL, 10, 15)
ON CONFLICT (code) DO NOTHING;

-- ── subscriptions ──────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS subscriptions (
    id                     UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id                UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    tier                   TEXT         NOT NULL REFERENCES plan_tiers(code),
    status                 TEXT         NOT NULL CHECK (status IN ('trialing','active','past_due','canceled')),
    started_at             TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    current_period_start   TIMESTAMPTZ  NOT NULL,
    current_period_end     TIMESTAMPTZ  NOT NULL,
    cancel_at              TIMESTAMPTZ,
    billing_anchor_day     INT          NOT NULL CHECK (billing_anchor_day BETWEEN 1 AND 28),
    created_at             TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at             TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_subscriptions_user_status
    ON subscriptions(user_id, status);

-- A user may have many historical canceled rows but only one live subscription.
CREATE UNIQUE INDEX IF NOT EXISTS uq_subscriptions_user_live
    ON subscriptions(user_id)
    WHERE status IN ('trialing','active','past_due');

-- ── invoices ───────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS invoices (
    id              UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    subscription_id UUID         REFERENCES subscriptions(id) ON DELETE SET NULL,
    period_start    TIMESTAMPTZ  NOT NULL,
    period_end      TIMESTAMPTZ  NOT NULL,
    line_items      JSONB        NOT NULL,
    subtotal_usdc   NUMERIC      NOT NULL,
    total_usdc      NUMERIC      NOT NULL,
    status          TEXT         NOT NULL CHECK (status IN ('open','paid','void','past_due')),
    paid_at         TIMESTAMPTZ,
    paid_tx_hash    TEXT,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_invoices_user_period
    ON invoices(user_id, period_end DESC);

-- ── usage_meters ───────────────────────────────────────────────────────────
-- Per-user counters keyed by billing period start (DATE so a UTC anniversary
-- is unambiguous). Updated incrementally by the agent + AUM poller.
CREATE TABLE IF NOT EXISTS usage_meters (
    user_id          UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    period_start     DATE         NOT NULL,
    decisions_count  INT          NOT NULL DEFAULT 0,
    aum_usd_avg      NUMERIC      NOT NULL DEFAULT 0,
    updated_at       TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, period_start)
);

-- ── performance_fees ───────────────────────────────────────────────────────
-- Accrual ledger: one row per (user, period) per benchmark. Settled monthly
-- via Nanopayments; settled_at + settlement_tx_hash filled in then.
CREATE TABLE IF NOT EXISTS performance_fees (
    id                    UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id               UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    decision_id           UUID         REFERENCES agent_decisions(id) ON DELETE SET NULL,
    period                TEXT         NOT NULL CHECK (period IN ('monthly')),
    benchmark             TEXT         NOT NULL CHECK (benchmark IN ('tbill_3m','susds')),
    realized_gain_usd     NUMERIC      NOT NULL,
    accrued_bps           INT          NOT NULL,
    accrued_usdc          NUMERIC      NOT NULL,
    settled_at            TIMESTAMPTZ,
    settlement_tx_hash    TEXT,
    created_at            TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

-- Partial index to quickly find pending accruals at settlement time.
CREATE INDEX IF NOT EXISTS idx_performance_fees_user_unsettled
    ON performance_fees(user_id)
    WHERE settled_at IS NULL;

-- ── updated_at triggers (reuse existing set_updated_at() from 0001) ────────
CREATE TRIGGER subscriptions_updated_at
    BEFORE UPDATE ON subscriptions
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER usage_meters_updated_at
    BEFORE UPDATE ON usage_meters
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
