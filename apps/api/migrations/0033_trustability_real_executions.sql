-- Public metrics must reflect only real, completed executions.
--
-- Before this, `v_trustability_per_user` counted every non-abstain decision in
-- the last 7 days — including mock-mode decisions — so the leaderboard and the
-- trust card could be inflated by simulated runs. We now:
--   1. tag each rebalance with the execution mode it ran in, and
--   2. count a decision only when it produced a real, completed rebalance.

ALTER TABLE rebalances
    ADD COLUMN IF NOT EXISTS execution_mode TEXT NOT NULL DEFAULT 'mock'
        CHECK (execution_mode IN ('mock', 'real'));

-- Existing rows default to 'mock' (conservative — they predate this column and
-- are overwhelmingly test/mock data, so they must not count as real).

CREATE OR REPLACE VIEW v_trustability_per_user AS
SELECT
    u.id                                                  AS user_id,
    md5(u.id::text)                                       AS handle_full,
    SUBSTRING(md5(u.id::text), 1, 8)                      AS handle,
    COUNT(d.id)                                           AS decisions_executed,
    COUNT(DISTINCT d.model_slug)                          AS distinct_models,
    COALESCE(AVG((m.outcome_24h->>'realizedPctChange')::float8), 0.0) AS avg_7d_return,
    COALESCE(
        AVG(
            (m.outcome_24h->>'realizedPctChange')::float8
            - (m.outcome_24h->>'counterfactualPctChange')::float8
        ),
        0.0
    )                                                     AS trustability_delta,
    MAX(d.created_at)                                     AS last_decision_at
FROM users u
JOIN portfolios p          ON p.user_id = u.id
JOIN agent_decisions d     ON d.portfolio_id = p.id
LEFT JOIN agent_memory m   ON m.decision_id = d.id
WHERE d.created_at > NOW() - INTERVAL '7 days'
  AND d.triggered_by != 'abstain'
  -- Only decisions that produced a real, completed on-chain rebalance count.
  AND EXISTS (
      SELECT 1 FROM rebalances rb
      WHERE rb.decision_id = d.id
        AND rb.status = 'completed'
        AND rb.execution_mode = 'real'
  )
GROUP BY u.id;
