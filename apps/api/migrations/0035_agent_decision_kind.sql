-- Agent-decided allocation: distinguish the new "allocation_proposal" decision
-- (the agent designs the target allocation, user approves) from the existing
-- "rebalance" decisions. `recommended_allocation` holds the agent's proposed
-- target weights ({"BTC": 50, "USDC": 30, ...}); `allocation_applied_at` is
-- stamped when the user approves and the target is written to the portfolio.
ALTER TABLE agent_decisions
    ADD COLUMN IF NOT EXISTS kind TEXT NOT NULL DEFAULT 'rebalance',
    ADD COLUMN IF NOT EXISTS recommended_allocation JSONB,
    ADD COLUMN IF NOT EXISTS allocation_applied_at TIMESTAMPTZ;

-- "latest pending allocation proposal per portfolio" lookups.
CREATE INDEX IF NOT EXISTS agent_decisions_portfolio_kind_idx
    ON agent_decisions (portfolio_id, kind, created_at DESC);
