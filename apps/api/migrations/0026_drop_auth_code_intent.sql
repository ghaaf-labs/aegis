-- ── 0026 — Remove legacy auth code intent ────────────────────────────────
--
-- The login/signup contract is now unified. Dropping `intent` keeps the
-- challenge table minimal and removes old, dead branching behavior.

DROP INDEX IF EXISTS idx_wallet_auth_codes_live;

ALTER TABLE wallet_auth_codes
    DROP COLUMN IF EXISTS intent;

CREATE INDEX IF NOT EXISTS idx_wallet_auth_codes_live
    ON wallet_auth_codes(email, expires_at)
    WHERE consumed_at IS NULL;
