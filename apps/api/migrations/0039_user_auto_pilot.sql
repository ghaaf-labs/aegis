-- Auto-pilot control (per user). When auto_pilot_enabled is TRUE, a scheduled
-- trigger (drift / regime-flip / harvest) makes the agent act on its own —
-- adopt a fresh target and execute the rebalance within the existing safety
-- clamps — instead of only surfacing a review. Mirrors agent_paused_at: a
-- per-user flag the scheduler reads on every tick. Paused users
-- (agent_paused_at set) are skipped regardless of this flag.
ALTER TABLE users ADD COLUMN auto_pilot_enabled BOOLEAN NOT NULL DEFAULT false;
