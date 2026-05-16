-- F-REG-1 — model evaluation runs + per-sample predictions for the regime
-- classifier backtest harness (and any future task evaluation).
--
-- Two tables:
--   * `model_evaluations` — one row per backtest run. Holds aggregate metrics
--     (precision/recall/F1/accuracy/Brier) plus a 3x3 confusion matrix and a
--     per-regime breakdown as JSON. Surfaced on the public `/about/regime`
--     model card.
--   * `model_evaluation_samples` — every (predicted, realized) pair from the
--     run. Downstream A8 (calibrated-confidence) trains a Brier calibrator
--     against this table.

CREATE TABLE IF NOT EXISTS model_evaluations (
    id                UUID            PRIMARY KEY DEFAULT gen_random_uuid(),
    model_slug        TEXT            NOT NULL,
    eval_run_id       UUID            NOT NULL,
    task              TEXT            NOT NULL,
    period_start      DATE            NOT NULL,
    period_end        DATE            NOT NULL,
    samples_count     INT             NOT NULL,
    accuracy          NUMERIC,
    precision_macro   NUMERIC,
    recall_macro      NUMERIC,
    f1_macro          NUMERIC,
    brier_score       NUMERIC,
    confusion_jsonb   JSONB           NOT NULL,
    per_regime_jsonb  JSONB           NOT NULL,
    created_at        TIMESTAMPTZ     NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_model_evaluations_task_model_created
    ON model_evaluations (task, model_slug, created_at DESC);

CREATE TABLE IF NOT EXISTS model_evaluation_samples (
    id                UUID            PRIMARY KEY DEFAULT gen_random_uuid(),
    eval_run_id       UUID            NOT NULL,
    observed_at       TIMESTAMPTZ     NOT NULL,
    predicted_label   TEXT            NOT NULL,
    predicted_proba   JSONB           NOT NULL,
    realized_label    TEXT            NOT NULL,
    features_jsonb    JSONB           NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_model_evaluation_samples_run_observed
    ON model_evaluation_samples (eval_run_id, observed_at);

COMMENT ON TABLE  model_evaluations IS 'One row per backtest run. Aggregate metrics for the regime classifier and any future model evaluation. Surfaced on the public /about/regime model card.';
COMMENT ON TABLE  model_evaluation_samples IS 'Per-sample (predicted, realized) pair. Consumed by A8 calibrated-confidence to fit a Brier calibrator.';
COMMENT ON COLUMN model_evaluations.task IS 'Free-form task identifier — e.g. ''regime_classifier''.';
COMMENT ON COLUMN model_evaluations.confusion_jsonb IS 'JSON object: { "rows": [ [tp_risk_on, fp_risk_on_as_neutral, fp_risk_on_as_risk_off], ... ] } indexed in label-order risk_on, neutral, risk_off.';
COMMENT ON COLUMN model_evaluations.per_regime_jsonb IS 'JSON object keyed by regime: { "risk_on": { "precision": 0.6, "recall": 0.5, "f1": 0.54, "support": 120 }, ... }.';
COMMENT ON COLUMN model_evaluation_samples.predicted_proba IS 'JSON object: { "risk_on": 0.7, "neutral": 0.2, "risk_off": 0.1 } — sums to ~1.0.';
