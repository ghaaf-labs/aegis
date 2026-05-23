-- Re-assert the single-portfolio product invariant for local/dev databases
-- created before the runtime create path was changed to replace the target.
WITH ranked AS (
    SELECT
        p.id,
        ROW_NUMBER() OVER (
            PARTITION BY p.user_id
            ORDER BY
                p.updated_at DESC,
                p.created_at DESC
        ) AS rn
    FROM portfolios p
)
DELETE FROM portfolios
WHERE id IN (SELECT id FROM ranked WHERE rn > 1);

CREATE UNIQUE INDEX IF NOT EXISTS portfolios_single_user_idx
    ON portfolios(user_id);
