-- F-PEG-7 — extend agent_decisions.triggered_by to allow 'peg_alert'.
--
-- The peg-defense monitor (apps/api/src/modules/risk_engine/peg_monitor.rs)
-- now persists a synthetic agent_decisions row when it builds a defensive
-- rebalance plan. The existing CHECK constraint from 0001+0002 didn't
-- include 'peg_alert', so this migration widens it.

ALTER TABLE agent_decisions
    DROP CONSTRAINT IF EXISTS agent_decisions_triggered_by_check;

ALTER TABLE agent_decisions
    ADD CONSTRAINT agent_decisions_triggered_by_check
        CHECK (triggered_by IN (
            'market_movement',
            'drift_threshold',
            'risk_breach',
            'scheduled',
            'user_request',
            'regime_flip',
            'abstain',
            'peg_alert'
        ));
