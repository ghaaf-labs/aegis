-- Durable rebalance saga — idempotency, retry accounting, stranded recovery.
--
-- The executor walks a plan's legs sequentially and halts on the first
-- failure (intentional; parallel execution is a documented later enhancement).
-- These columns make that walk durable, idempotent, and recoverable:
--
--   idempotency_key  A deterministic per-leg fingerprint
--                    (rebalance_id : leg_index : kind : src>dest : rounded-amount)
--                    so a retried or resumed walk recognizes an already-submitted
--                    leg instead of double-submitting it. NULL until first submit.
--   attempt_count    Bumped on every submit so retries are observable and a
--                    runaway leg can be capped.
--   stranded_asset   TRUE when funds moved (e.g. a bridge mint landed USDC) but
--                    the leg's final action failed. The minted USDC stays in the
--                    user's wallet; we surface it as idle cash and let a follow-up
--                    rebalance replan the still-needed delta, rather than bricking
--                    the whole plan.
--
-- UNIQUE (rebalance_id, idempotency_key): one logical leg per plan can only ever
-- hold one idempotency key, so a concurrent or resumed walk that recomputes the
-- same key collides instead of inserting/keying a duplicate. Partial index skips
-- the NULL keys carried by not-yet-submitted legs (a UNIQUE over NULLs would let
-- many NULLs coexist anyway, but the WHERE keeps the index tight and intent clear).

ALTER TABLE rebalance_legs
    ADD COLUMN IF NOT EXISTS idempotency_key TEXT,
    ADD COLUMN IF NOT EXISTS attempt_count   INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    ADD COLUMN IF NOT EXISTS stranded_asset  BOOLEAN NOT NULL DEFAULT FALSE;

CREATE UNIQUE INDEX IF NOT EXISTS uq_rebalance_legs_idempotency
    ON rebalance_legs(rebalance_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

-- Recovery surfacing: a follow-up replan and the status UI both need to find
-- the stranded legs of a plan quickly.
CREATE INDEX IF NOT EXISTS idx_rebalance_legs_stranded
    ON rebalance_legs(rebalance_id)
    WHERE stranded_asset = TRUE;
