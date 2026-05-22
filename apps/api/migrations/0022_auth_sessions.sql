-- ── 0022 — Server-side session revocation ────────────────────────────────
--
-- Aegis JWTs are still carried in HttpOnly cookies, but each token now has a
-- `jti` backed by this table. Logout revokes the current `jti`; auth
-- middleware rejects revoked or expired rows instead of trusting a copied JWT
-- until its natural expiry.

CREATE TABLE IF NOT EXISTS auth_sessions (
    id           UUID PRIMARY KEY,
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at   TIMESTAMPTZ NOT NULL,
    revoked_at   TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_auth_sessions_user_active
    ON auth_sessions(user_id, expires_at DESC)
    WHERE revoked_at IS NULL;
