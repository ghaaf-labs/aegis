-- Aegis is one-portfolio-per-user. Collapse any user with multiple portfolios
-- down to the *actively-used* one — keep the portfolio with the most agent
-- decisions (proxy for "this is the one the user has been driving"), tiebreak
-- by most recent update. The rest are deleted via the ON DELETE CASCADE
-- chains already declared on allocations / rebalances / agent_decisions.
-- Then enforce the invariant with a UNIQUE constraint so the backend can't
-- accidentally regress to multi-portfolio.

WITH ranked AS (
    SELECT p.id, p.user_id,
           ROW_NUMBER() OVER (
               PARTITION BY p.user_id
               ORDER BY (SELECT COUNT(*) FROM agent_decisions WHERE portfolio_id = p.id) DESC,
                        p.updated_at DESC,
                        p.created_at ASC
           ) AS rn
    FROM portfolios p
)
DELETE FROM portfolios
WHERE id IN (SELECT id FROM ranked WHERE rn > 1);

ALTER TABLE portfolios
    ADD CONSTRAINT portfolios_user_id_unique UNIQUE (user_id);
