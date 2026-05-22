-- ── 0032 — Wallet provisioning retry state ───────────────────────────────
--
-- Background reconciliation needs persisted backoff so a Circle outage does
-- not turn every API process into a tight retry loop.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS wallet_provision_attempts INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS wallet_provision_next_retry_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS wallet_provision_last_error TEXT;

UPDATE users
SET wallet_provision_next_retry_at = COALESCE(wallet_provision_next_retry_at, NOW())
WHERE account_status = 'pending_wallet'
  AND deletion_requested_at IS NULL
  AND anonymized_at IS NULL;

CREATE INDEX IF NOT EXISTS users_wallet_provision_retry_idx
    ON users (wallet_provision_next_retry_at, updated_at)
    WHERE account_status = 'pending_wallet'
      AND deletion_requested_at IS NULL
      AND anonymized_at IS NULL;
