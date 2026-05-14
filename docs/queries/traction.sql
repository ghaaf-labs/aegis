-- Traction queries for the hackathon submission.
-- All numbers come from our own Postgres — no third-party analytics.

-- ── Real users ─────────────────────────────────────────────────────────────
-- Distinct wallets created (= real signups).
SELECT COUNT(*) AS users
FROM users
WHERE wallet_id IS NOT NULL;

-- Real users in the event window.
SELECT COUNT(*) AS users_in_window
FROM users
WHERE wallet_id IS NOT NULL
  AND created_at BETWEEN '2026-05-11' AND '2026-05-25';

-- ── AUM ────────────────────────────────────────────────────────────────────
-- Sum of portfolio values across all real users.
SELECT SUM(total_value_usd) AS aum_usd
FROM portfolios p
JOIN users u ON u.id = p.user_id
WHERE u.wallet_id IS NOT NULL;

-- ── Activity ───────────────────────────────────────────────────────────────
-- Total agent decisions produced.
SELECT COUNT(*) AS decisions FROM agent_decisions;

-- Approved rebalances.
SELECT COUNT(*) AS approved_rebalances
FROM rebalance_events
WHERE status IN ('approved', 'executing', 'completed');

-- ── Funnel from analytics_events ───────────────────────────────────────────
WITH stages AS (
  SELECT user_id, event_name, MIN(occurred_at) AS first_at
  FROM analytics_events
  WHERE event_name IN (
    'wallet.created',
    'faucet.claimed',
    'goal.completed',
    'analyze.triggered',
    'decision.approved'
  )
  GROUP BY user_id, event_name
)
SELECT event_name, COUNT(DISTINCT user_id) AS users
FROM stages
GROUP BY event_name
ORDER BY CASE event_name
  WHEN 'wallet.created'    THEN 1
  WHEN 'faucet.claimed'    THEN 2
  WHEN 'goal.completed'    THEN 3
  WHEN 'analyze.triggered' THEN 4
  WHEN 'decision.approved' THEN 5
END;

-- ── Retention (7-day) ──────────────────────────────────────────────────────
-- Users who returned ≥7 days after wallet creation.
SELECT COUNT(DISTINCT u.id) AS retained_7d
FROM users u
JOIN analytics_events e ON e.user_id = u.id
WHERE u.wallet_id IS NOT NULL
  AND e.occurred_at - u.created_at > INTERVAL '7 days';
