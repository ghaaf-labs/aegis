-- The RoutableSnapshot fingerprint the plan was built against (INV-6).
--
-- On approval the executor re-captures routability and refuses if this hash
-- changed — a rail flipped Ready⇄track-only since the plan was reviewed, so the
-- approved legs may no longer be settleable. NULL for legacy/mock-mode plans
-- that predate the snapshot (treated as "no binding", never a false stale).
ALTER TABLE rebalances ADD COLUMN routable_snapshot_hash text;
