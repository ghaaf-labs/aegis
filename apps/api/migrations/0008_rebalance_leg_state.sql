-- Typed settlement-leg state (the `LegState` FSM), consolidating the prior
-- `(status, stranded_asset)` pair into one auditable, model-checked column.
--
-- Written by the executor at each transition via `LegState::as_str()`; the saga
-- driver and the execution-trace UI read it. `pending` covers in-flight and
-- legacy rows. The CHECK admits every FSM variant (incl. the bridge states the
-- executor emits once walk_legs is re-instrumented) so the schema is forward-
-- compatible with the full saga without another migration.
ALTER TABLE rebalance_legs
  ADD COLUMN leg_state text NOT NULL DEFAULT 'pending'
  CHECK (leg_state IN (
    'pending', 'quoted', 'submitted', 'bridge_in_flight', 'bridge_landed',
    'confirmed', 'failed', 'stranded_reserve', 'compensated_to_usdc'
  ));
