-- ── 0025 — Unified email auth account state ───────────────────────────────
--
-- Adds the durable account/consent fields required by docs/12 and loosens the
-- old signup-vs-login code intent so the public contract can be one
-- enumeration-safe "Continue with email" entry point.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS account_status TEXT NOT NULL DEFAULT 'active'
        CHECK (account_status IN ('pending_wallet', 'active')),
    ADD COLUMN IF NOT EXISTS custody_model TEXT NOT NULL DEFAULT 'circle_developer'
        CHECK (custody_model IN ('circle_developer', 'circle_user', 'external')),
    ADD COLUMN IF NOT EXISTS wallet_set_id TEXT,
    ADD COLUMN IF NOT EXISTS tos_version TEXT,
    ADD COLUMN IF NOT EXISTS privacy_version TEXT,
    ADD COLUMN IF NOT EXISTS consented_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS marketing_opt_in BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS deletion_requested_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS anonymized_at TIMESTAMPTZ;

UPDATE users
SET account_status = 'pending_wallet'
WHERE wallet_id IS NULL
   OR arc_address IS NULL
   OR base_address IS NULL;

ALTER TABLE wallet_auth_codes
    ALTER COLUMN intent DROP NOT NULL;

CREATE INDEX IF NOT EXISTS users_deletion_pending_idx
    ON users (deletion_requested_at)
    WHERE deletion_requested_at IS NOT NULL AND anonymized_at IS NULL;
