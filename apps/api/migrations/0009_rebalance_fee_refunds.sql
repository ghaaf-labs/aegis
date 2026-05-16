-- Refund-on-failure path for protocol fees (F-BILL-1).
--
-- The pre-2026-05-15 schema only had a "settled" terminal state, so any
-- rebalance that failed mid-plan still left its 25 bps protocol fee row in
-- rebalance_fees with no way to mark it as reversed. This migration adds a
-- proper status column, a refund timestamp, and a reverse-tx-hash so the
-- billing service can record a Nanopayments /reverse settlement when a leg
-- transitions to 'failed'.

ALTER TABLE rebalance_fees
    ADD COLUMN IF NOT EXISTS refunded_at     TIMESTAMPTZ NULL,
    ADD COLUMN IF NOT EXISTS refund_tx_hash  TEXT NULL,
    ADD COLUMN IF NOT EXISTS status          TEXT NOT NULL DEFAULT 'settled'
        CHECK (status IN ('pending', 'settled', 'refunded', 'failed'));

CREATE INDEX IF NOT EXISTS idx_rebalance_fees_status
    ON rebalance_fees(rebalance_id, status);
