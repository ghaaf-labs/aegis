-- Upgrade the single-portfolio invariant from a unique INDEX (0034) to a true
-- UNIQUE CONSTRAINT, matching 0019's original intent (dropped by 0023). The
-- agent-decided-allocation model has exactly one portfolio per user; the
-- constraint makes that a named, schema-level guarantee.
WITH ranked AS (
    SELECT
        p.id,
        ROW_NUMBER() OVER (
            PARTITION BY p.user_id
            ORDER BY p.updated_at DESC, p.created_at DESC
        ) AS rn
    FROM portfolios p
)
DELETE FROM portfolios
WHERE id IN (SELECT id FROM ranked WHERE rn > 1);

DROP INDEX IF EXISTS portfolios_single_user_idx;

ALTER TABLE portfolios
    DROP CONSTRAINT IF EXISTS portfolios_user_id_unique;
ALTER TABLE portfolios
    ADD CONSTRAINT portfolios_user_id_unique UNIQUE (user_id);
