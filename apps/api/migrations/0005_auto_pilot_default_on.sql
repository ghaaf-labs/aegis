-- Auto-pilot becomes the default posture. Per RFB 04 the agent steers most of
-- the time without a per-move approval: it deploys and rebalances on its own
-- within its guardrails — constitution check, <=60% single-asset cap,
-- stable-reserve floor, $5 dust minimum, peg defense, and the route-registry
-- fail-closed — and only leaves a manual review when a guardrail trips. Users
-- opt out per-account via Settings -> Agent control (the auto_pilot toggle).
--
-- New rows inherit the default; existing accounts are migrated on so behavior is
-- uniform. A user who later turns auto-pilot off keeps that choice (the toggle
-- writes auto_pilot_enabled = false and is not reverted by anything here).
ALTER TABLE users ALTER COLUMN auto_pilot_enabled SET DEFAULT true;

UPDATE users SET auto_pilot_enabled = true WHERE auto_pilot_enabled = false;
