-- SM-1 — Strategies marketplace MVP.
--
-- One row per published strategy template. Curated rows are seeded by
-- `apps/api/src/bin/seed_curated_strategies.rs` (idempotent on the stable
-- UUIDs below). User-published strategies are a v2 follow-up — the schema
-- already supports them via author_user_id (nullable for curated rows).
CREATE TABLE IF NOT EXISTS strategies (
    id                    UUID            PRIMARY KEY DEFAULT gen_random_uuid(),
    name                  TEXT            NOT NULL,
    description           TEXT            NOT NULL,
    risk_band             TEXT            NOT NULL CHECK (risk_band IN ('low','medium','high')),
    min_horizon_months    INTEGER         NOT NULL CHECK (min_horizon_months >= 1),
    target_allocation     JSONB           NOT NULL,
    is_curated            BOOLEAN         NOT NULL DEFAULT FALSE,
    author_user_id        UUID            REFERENCES users(id) ON DELETE SET NULL,
    created_at            TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_at            TIMESTAMPTZ     NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_strategies_curated_risk
    ON strategies (is_curated, risk_band);

-- Reuse the existing set_updated_at() trigger from 0001_initial.sql.
CREATE TRIGGER strategies_updated_at
BEFORE UPDATE ON strategies
FOR EACH ROW EXECUTE FUNCTION set_updated_at();
