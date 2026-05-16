-- Accountant share tokens for the 1099-DA tax export (A10 / F-TAX-2).
--
-- A Pro user generates a read-only signed URL for their accountant; the
-- token resolves to (user_id, portfolio_id, year) and serves the CSV
-- without any auth — that's the entire point. Tokens carry an explicit
-- expiry + revocation column so the user can yank access without
-- regenerating their JWT, and we never reuse the token value across
-- generations.
--
-- Migration numbering: 0013 is reserved by A8 (calibrations). A10 takes
-- 0014 per docs/plan-2026-05-16 §3.3 + the orchestrator coordination note.

CREATE TABLE IF NOT EXISTS tax_share_tokens (
    id            UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID         NOT NULL REFERENCES users(id)      ON DELETE CASCADE,
    portfolio_id  UUID         NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
    token         TEXT         NOT NULL UNIQUE,
    year          INTEGER      NOT NULL CHECK (year BETWEEN 2020 AND 2100),
    expires_at    TIMESTAMPTZ  NOT NULL,
    revoked_at    TIMESTAMPTZ,
    created_at    TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

-- The token is the lookup key — bare equality, no LIKE — so a btree on
-- (token) alone is the right index. UNIQUE on token already creates one
-- implicitly, but be explicit so the index is visible in pg_indexes.
CREATE INDEX IF NOT EXISTS idx_tax_share_tokens_token
    ON tax_share_tokens(token);

-- List + revoke flows in the settings UI walk by (user_id, created_at DESC).
CREATE INDEX IF NOT EXISTS idx_tax_share_tokens_user_at
    ON tax_share_tokens(user_id, created_at DESC);
