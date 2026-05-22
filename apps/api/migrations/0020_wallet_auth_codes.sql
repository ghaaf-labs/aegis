-- ── 0020 — Email verification for wallet auth ─────────────────────────────
--
-- Login/signup may not mint an Aegis JWT from email knowledge alone. Store
-- short-lived verification challenges here; only a verified code can continue
-- to Circle wallet setup or session restore.

CREATE TABLE IF NOT EXISTS wallet_auth_codes (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email           TEXT NOT NULL,
    intent          TEXT NOT NULL CHECK (intent IN ('signup', 'login')),
    code_hash       TEXT NOT NULL,
    referrer_handle TEXT,
    attempts        INTEGER NOT NULL DEFAULT 0,
    expires_at      TIMESTAMPTZ NOT NULL,
    consumed_at     TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_wallet_auth_codes_email_created
    ON wallet_auth_codes(email, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_wallet_auth_codes_live
    ON wallet_auth_codes(email, intent, expires_at)
    WHERE consumed_at IS NULL;
