CREATE TABLE IF NOT EXISTS account_export_jobs (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    archive       JSONB       NOT NULL,
    expires_at    TIMESTAMPTZ NOT NULL,
    delivered_at  TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS account_export_jobs_user_created_idx
    ON account_export_jobs(user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS account_export_jobs_expires_idx
    ON account_export_jobs(expires_at);
