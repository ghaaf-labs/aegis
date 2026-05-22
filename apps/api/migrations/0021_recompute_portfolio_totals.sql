-- ── 0021 — Repair stale portfolio totals ──────────────────────────────────
--
-- Older target-edit flows could delete/recreate zero-valued allocations while
-- leaving portfolios.total_value_usd untouched. The dashboard then showed an
-- invested headline with zero holdings. Make allocations the source of truth.

UPDATE allocations a
SET current_weight = CASE
    WHEN totals.total_value_usd > 0 THEN (a.value_usd / totals.total_value_usd) * 100
    ELSE 0
END,
updated_at = NOW()
FROM (
    SELECT portfolio_id, COALESCE(SUM(value_usd), 0)::DOUBLE PRECISION AS total_value_usd
    FROM allocations
    GROUP BY portfolio_id
) totals
WHERE a.portfolio_id = totals.portfolio_id;

UPDATE portfolios p
SET total_value_usd = COALESCE(totals.total_value_usd, 0),
    updated_at = NOW()
FROM (
    SELECT portfolio_id, COALESCE(SUM(value_usd), 0)::DOUBLE PRECISION AS total_value_usd
    FROM allocations
    GROUP BY portfolio_id
) totals
WHERE p.id = totals.portfolio_id;

UPDATE portfolios p
SET total_value_usd = 0,
    updated_at = NOW()
WHERE NOT EXISTS (
    SELECT 1 FROM allocations a WHERE a.portfolio_id = p.id
);
