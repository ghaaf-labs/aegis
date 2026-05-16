-- Record protocol fees (25 bps) and other fees per rebalance for Nanopayments story.
CREATE TABLE IF NOT EXISTS rebalance_fees (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rebalance_id  UUID NOT NULL REFERENCES rebalances(id) ON DELETE CASCADE,
    fee_type      TEXT NOT NULL,           -- 'protocol', 'gas', 'referral', etc.
    amount_usdc   DOUBLE PRECISION NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- One fee row per (plan, type). Lets the executor use ON CONFLICT DO
    -- NOTHING so re-runs of the post-plan billing block don't double-charge.
    UNIQUE (rebalance_id, fee_type)
);

CREATE INDEX IF NOT EXISTS idx_rebalance_fees_rebalance ON rebalance_fees(rebalance_id);