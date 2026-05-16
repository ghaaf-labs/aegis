-- F-PEG-1 — Peg-defense rules + event log.
--
-- A peg rule watches a stablecoin (USDC, EURC, USYC) and fires when the
-- observed price stays below `threshold_price` for `window_seconds`. The
-- emitted `peg_events` row records each firing for audit + throttle.

CREATE TABLE IF NOT EXISTS peg_rules (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- NULL portfolio_id == apply to every portfolio the user owns.
    portfolio_id    UUID REFERENCES portfolios(id) ON DELETE CASCADE,
    asset           TEXT NOT NULL,
    threshold_price NUMERIC NOT NULL CHECK (threshold_price > 0),
    window_seconds  INTEGER NOT NULL DEFAULT 300 CHECK (window_seconds >= 0),
    action_kind     TEXT NOT NULL CHECK (action_kind IN ('alert','propose_rebalance','auto_execute')),
    target_asset    TEXT,
    enabled         BOOLEAN NOT NULL DEFAULT TRUE,
    paused_at       TIMESTAMPTZ,
    last_fired_at   TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_peg_rules_user_enabled
    ON peg_rules(user_id) WHERE enabled = TRUE AND paused_at IS NULL;

CREATE TRIGGER peg_rules_updated_at
    BEFORE UPDATE ON peg_rules FOR EACH ROW EXECUTE FUNCTION set_updated_at();


CREATE TABLE IF NOT EXISTS peg_events (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_id        UUID NOT NULL REFERENCES peg_rules(id) ON DELETE CASCADE,
    asset          TEXT NOT NULL,
    observed_price NUMERIC NOT NULL,
    observed_at    TIMESTAMPTZ NOT NULL,
    action_taken   TEXT NOT NULL,
    rebalance_id   UUID REFERENCES rebalances(id) ON DELETE SET NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_peg_events_rule_observed
    ON peg_events(rule_id, observed_at DESC);
