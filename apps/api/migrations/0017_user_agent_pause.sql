-- Global agent-pause control. When agent_paused_at is non-null, every
-- scheduled trigger (drift watcher, regime monitor, peg defense, digest)
-- skips the user. Manual rebalances and /agent/analyze calls remain open
-- so the user can still hand-drive their own portfolio.
ALTER TABLE users ADD COLUMN agent_paused_at TIMESTAMPTZ NULL;
