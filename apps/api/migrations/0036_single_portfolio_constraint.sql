-- Upgrade the single-portfolio invariant from a unique INDEX (0034) to a true
-- UNIQUE CONSTRAINT, matching 0019's original intent (dropped by 0023). The
-- agent-decided-allocation model has exactly one portfolio per user; the
-- constraint makes that a named, schema-level guarantee.
--
-- Non-destructive (same rationale as 0034): if any user still has more than one
-- portfolio, bail with an explicit message rather than CASCADE-deleting history.
-- After 0034 this is normally a no-op, but it is re-checked here so the ADD
-- CONSTRAINT below can never silently lose data to satisfy uniqueness.
DO $$
DECLARE
    dup_users BIGINT;
BEGIN
    SELECT COUNT(*) INTO dup_users
    FROM (
        SELECT user_id
        FROM portfolios
        GROUP BY user_id
        HAVING COUNT(*) > 1
    ) d;

    IF dup_users > 0 THEN
        RAISE EXCEPTION
            'single-portfolio invariant: % user(s) have more than one portfolio. Archive or merge the extras manually, then re-run this migration. No rows were deleted.',
            dup_users;
    END IF;
END $$;

DROP INDEX IF EXISTS portfolios_single_user_idx;

ALTER TABLE portfolios
    DROP CONSTRAINT IF EXISTS portfolios_user_id_unique;
ALTER TABLE portfolios
    ADD CONSTRAINT portfolios_user_id_unique UNIQUE (user_id);
