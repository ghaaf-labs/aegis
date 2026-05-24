-- Re-assert the single-portfolio product invariant for local/dev databases
-- created before the runtime create path was changed to replace the target.
--
-- Non-destructive: rather than DELETE-ing every "extra" portfolio (which would
-- CASCADE away its rebalances / agent_decisions / agent_memory / tax_lots /
-- diary history irreversibly), bail with an explicit message if any user still
-- has more than one portfolio, so an operator archives or merges the extras by
-- hand first. In the normal case the runtime already keeps exactly one portfolio
-- per user, so this is a no-op followed by the uniqueness index.
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

CREATE UNIQUE INDEX IF NOT EXISTS portfolios_single_user_idx
    ON portfolios(user_id);
