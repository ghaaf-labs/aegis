-- F-CONF-1 — calibrated confidence + counterfactual storage (agent A8).
--
-- Two tables + two columns on agent_decisions:
--   * `calibrations` — one row per fit. Trained from model_evaluation_samples
--     (regime classifier task) and from agent_memory outcomes (strategist
--     confidence task). Method + params live in JSONB so we can swap
--     isotonic ↔ histogram-bin without a migration.
--   * `calibrated_predictions` — optional audit trail wiring a calibration to
--     a specific agent_decisions row, plus the LLM-emitted counterfactual.
--   * `agent_decisions.raw_confidence` / `calibrated_confidence` — both stored
--     so the UI can render the headline (calibrated) and the tooltip (raw)
--     without re-deriving from the audit trail. DOUBLE PRECISION to match the
--     existing `confidence` column and the codebase's ergonomic NUMERIC story.
--
-- Note: NUMERIC was specified by the plan, but the rest of the codebase reads
-- confidence as f64 (DOUBLE PRECISION). Mixing the two would force a new
-- bigdecimal/rust_decimal dependency just for two probability columns;
-- DOUBLE PRECISION keeps the read path consistent with `confidence` and
-- avoids a dependency that no other table needs.

CREATE TABLE IF NOT EXISTS calibrations (
    id                  UUID            PRIMARY KEY DEFAULT gen_random_uuid(),
    model_slug          TEXT            NOT NULL,
    task                TEXT            NOT NULL,
    source_eval_run_id  UUID,
    method              TEXT            NOT NULL
                        CHECK (method IN ('platt','isotonic','brier_bin')),
    params_jsonb        JSONB           NOT NULL,
    fit_samples_count   INT             NOT NULL DEFAULT 0,
    brier_before        DOUBLE PRECISION,
    brier_after         DOUBLE PRECISION,
    created_at          TIMESTAMPTZ     NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_calibrations_task_model_created
    ON calibrations (task, model_slug, created_at DESC);

CREATE TABLE IF NOT EXISTS calibrated_predictions (
    id                      UUID            PRIMARY KEY DEFAULT gen_random_uuid(),
    decision_id             UUID            REFERENCES agent_decisions(id) ON DELETE CASCADE,
    raw_confidence          DOUBLE PRECISION,
    calibrated_confidence   DOUBLE PRECISION,
    calibration_id          UUID            REFERENCES calibrations(id) ON DELETE SET NULL,
    counterfactual          TEXT,
    created_at              TIMESTAMPTZ     NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_calibrated_predictions_decision
    ON calibrated_predictions (decision_id);

ALTER TABLE agent_decisions
    ADD COLUMN IF NOT EXISTS raw_confidence        DOUBLE PRECISION,
    ADD COLUMN IF NOT EXISTS calibrated_confidence DOUBLE PRECISION,
    ADD COLUMN IF NOT EXISTS counterfactual        TEXT;

COMMENT ON TABLE  calibrations IS 'Trained probability calibrators (isotonic or histogram-bin). One row per fit.';
COMMENT ON COLUMN calibrations.task IS 'Free-form task identifier — ''regime_classifier'' or ''strategist_confidence''.';
COMMENT ON COLUMN calibrations.method IS 'Calibration family. Histogram-bin (''brier_bin'') is what A8 currently fits; isotonic/platt reserved.';
COMMENT ON COLUMN calibrations.params_jsonb IS 'Method-specific fitted params. For brier_bin: { "classes": ["risk_on","neutral","risk_off"], "bins": [{ "lo": 0.0, "hi": 0.1, "empirical": { "risk_on": 0.12, ... }, "n": 14 }, ...] }.';
COMMENT ON TABLE  calibrated_predictions IS 'Per-decision audit trail: raw → calibrated confidence + critic counterfactual.';
COMMENT ON COLUMN agent_decisions.raw_confidence IS 'Strategist''s self-reported confidence pre-calibration. Equal to `confidence` for backfill.';
COMMENT ON COLUMN agent_decisions.calibrated_confidence IS 'Confidence after the A8 calibrator is applied. Equal to `raw_confidence` when no calibration exists yet.';
COMMENT ON COLUMN agent_decisions.counterfactual IS 'One-sentence critic counterfactual gated by CALIBRATED_CONF_ENABLED.';
