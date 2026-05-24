-- Leaderboard completeness: surface AUM and turnover per user.
--
-- The leaderboard ranked users by trustability_delta + decision count but
-- exposed neither how much capital the agent manages for them (AUM) nor how
-- actively it trades (turnover). Both are real, un-fakeable signals: AUM sums
-- the user's portfolios.total_value_usd; turnover is the decisions count over
-- the view's existing 7-day window (so it already reads as decisions/week).
--
-- AUM is summed across the user's portfolios in a correlated subquery so the
-- per-decision JOIN fan-out can't multiply the portfolio value.

-- DROP + CREATE (not REPLACE): the new columns change the view's column set/
-- order, which Postgres' CREATE OR REPLACE VIEW forbids ("cannot change name of
-- view column"). Nothing has a DB-level dependency on this view (only queries
-- read it), so a clean recreate is safe.
DROP VIEW IF EXISTS v_trustability_per_user;

CREATE VIEW v_trustability_per_user AS
SELECT
    u.id                                                  AS user_id,
    md5(u.id::text)                                       AS handle_full,
    SUBSTRING(md5(u.id::text), 1, 8)                      AS handle,
    COUNT(d.id)                                           AS decisions_executed,
    COUNT(d.id)                                           AS decisions_per_week,
    COUNT(DISTINCT d.model_slug)                          AS distinct_models,
    COALESCE(AVG((m.outcome_24h->>'realizedPctChange')::float8), 0.0) AS avg_7d_return,
    COALESCE(
        AVG(
            (m.outcome_24h->>'realizedPctChange')::float8
            - (m.outcome_24h->>'counterfactualPctChange')::float8
        ),
        0.0
    )                                                     AS trustability_delta,
    MAX(d.created_at)                                     AS last_decision_at,
    COALESCE(
        (SELECT SUM(p2.total_value_usd)::float8
         FROM portfolios p2
         WHERE p2.user_id = u.id),
        0.0
    )                                                     AS aum_usd
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
