-- Explicit DAG dependencies for each rebalance leg: the set of leg_index
-- values that must complete (confirmed) before this leg can be dispatched.
-- Empty array (the default) means the leg may start immediately.
--
-- Within a CCTP transfer the mint depends on the burn; a post-bridge swap
-- depends on the mint. These relationships were previously implicit in
-- leg_index ordering; they are now first-class, so the executor can
-- schedule independent legs in parallel and never confuse sequential
-- ordering with true data-dependency.
ALTER TABLE rebalance_legs
    ADD COLUMN depends_on integer[] NOT NULL DEFAULT '{}';
