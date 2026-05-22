-- ── 0027 — Public auth IP rate limits ────────────────────────────────────
--
-- docs/12 requires both per-email and per-IP throttles for email-code auth.
-- Store hashed bucket ids, not raw IP addresses.

CREATE TABLE IF NOT EXISTS auth_rate_limits (
    id TEXT PRIMARY KEY,
    hits INTEGER NOT NULL DEFAULT 0,
    reset_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_auth_rate_limits_reset
    ON auth_rate_limits (reset_at);
