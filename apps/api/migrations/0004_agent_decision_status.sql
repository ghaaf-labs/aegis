-- ════════════════════════════════════════════════════════════════════════════
-- 0004 — Async inference job lifecycle on agent_decisions
-- ════════════════════════════════════════════════════════════════════════════
--
-- Inference (regime → allocator/strategist → critic) runs 5–60s+ and, behind
-- nginx's 60s proxy_read_timeout, can't reliably complete on the request path.
-- We move it off-request: the handler inserts a `queued` row and returns it
-- immediately, a spawned worker runs the pipeline and flips the row to `ready`
-- (or `failed`), and the client learns the outcome over SSE / by polling the row.
--
--   status        queued → running → ready | failed   (legacy rows backfill ready)
--   error         failure reason, set when status = 'failed'
--   started_at    stamped when the worker picks the row up
--   completed_at  stamped on ready | failed
--
-- The partial unique index makes "at most one in-flight job per (portfolio,
-- kind)" a DB invariant, so a double-submit (React StrictMode, double-click, or
-- a retry while one is still running) dedupes to the existing job instead of
-- spawning a duplicate (and billable) model call.
--
-- A single API replica (maxUnavailable:0) means any row still `queued`/`running`
-- after a restart is orphaned — its worker died with the old pod. A boot-time
-- reconciler marks those `failed` so the client recovers via a retry rather than
-- waiting on an SSE event that will never arrive.

ALTER TABLE agent_decisions
    ADD COLUMN IF NOT EXISTS status       text NOT NULL DEFAULT 'ready',
    ADD COLUMN IF NOT EXISTS error        text,
    ADD COLUMN IF NOT EXISTS started_at   timestamp with time zone,
    ADD COLUMN IF NOT EXISTS completed_at timestamp with time zone;

DO $$ BEGIN
    ALTER TABLE agent_decisions
        ADD CONSTRAINT agent_decisions_status_check
        CHECK (status = ANY (ARRAY['queued'::text, 'running'::text, 'ready'::text, 'failed'::text]));
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

-- At most one in-flight job per (portfolio, kind).
CREATE UNIQUE INDEX IF NOT EXISTS agent_decisions_one_inflight_per_portfolio_kind
    ON agent_decisions (portfolio_id, kind)
    WHERE status IN ('queued', 'running');

-- The decision list + reasoning feed only ever show terminal `ready` rows;
-- index that hot path (a filtered companion to idx_agent_decisions_portfolio_created).
CREATE INDEX IF NOT EXISTS idx_agent_decisions_ready
    ON agent_decisions (portfolio_id, created_at DESC)
    WHERE status = 'ready';
