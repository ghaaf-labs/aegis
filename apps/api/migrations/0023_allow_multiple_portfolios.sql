-- ── 0023 — Multiple portfolios per wallet ────────────────────────────────
--
-- Strategies and the app navigation now treat each adopted strategy or
-- custom goal as its own portfolio. The old single-portfolio constraint made
-- strategy adoption fail for returning users.

ALTER TABLE portfolios
    DROP CONSTRAINT IF EXISTS portfolios_user_id_unique;
