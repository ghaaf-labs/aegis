-- ── 0002 — Agent telemetry & memory ────────────────────────────────────────
--
-- Extends agent_decisions with the telemetry needed to render trust signals in
-- the UI (model slug, regime, token usage, latency, critic verdict) and adds
-- an agent_memory table seeded for Sprint 2's per-portfolio memory retrieval.
--
-- Rollback: see down migration `0002_agent_telemetry.down.sql` (drop new
-- columns, drop agent_memory, restore the original triggered_by CHECK).

-- ── agent_decisions: telemetry columns ─────────────────────────────────────
ALTER TABLE agent_decisions
    ADD COLUMN model_slug        TEXT,
    ADD COLUMN regime            TEXT,
    ADD COLUMN prompt_tokens     INTEGER,
    ADD COLUMN completion_tokens INTEGER,
    ADD COLUMN latency_ms        INTEGER,
    ADD COLUMN critic_verdict    JSONB;

ALTER TABLE agent_decisions
    ADD CONSTRAINT agent_decisions_regime_check
        CHECK (regime IS NULL OR regime IN ('risk_on', 'neutral', 'risk_off'));

-- Widen triggered_by to cover the new self-trigger paths.
ALTER TABLE agent_decisions
    DROP CONSTRAINT agent_decisions_triggered_by_check;

ALTER TABLE agent_decisions
    ADD CONSTRAINT agent_decisions_triggered_by_check
        CHECK (triggered_by IN (
            'market_movement',
            'drift_threshold',
            'risk_breach',
            'scheduled',
            'user_request',
            'regime_flip',
            'abstain'
        ));

CREATE INDEX IF NOT EXISTS idx_agent_decisions_regime
    ON agent_decisions(regime)
    WHERE regime IS NOT NULL;

-- ── agent_memory ────────────────────────────────────────────────────────────
-- Seeded for Sprint 2: each row records a 24h-after outcome snapshot of a
-- decision so the strategist can read its own past performance into the
-- prompt without re-querying every dependency.
CREATE TABLE IF NOT EXISTS agent_memory (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    portfolio_id UUID        NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
    decision_id  UUID        NOT NULL REFERENCES agent_decisions(id) ON DELETE CASCADE,
    outcome_24h  JSONB,
    recorded_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (decision_id)
);

CREATE INDEX IF NOT EXISTS idx_agent_memory_portfolio
    ON agent_memory(portfolio_id, recorded_at DESC);
