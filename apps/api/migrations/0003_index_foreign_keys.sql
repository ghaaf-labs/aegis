-- ════════════════════════════════════════════════════════════════════════════
-- 0003 — Index unindexed foreign keys
-- ════════════════════════════════════════════════════════════════════════════
--
-- Postgres does NOT auto-create an index for a foreign-key column. Without one,
-- every delete of a parent row seq-scans the child table to enforce the FK, and
-- joins on the FK can't use an index. This app deletes parents in normal flows:
-- the single-portfolio model replaces (deletes) a portfolio, GDPR account
-- deletion cascades from users, and decision cleanup cascades to dependents — so
-- these scans are real, not hypothetical.
--
-- Covers every FK column that lacked a left-prefix index, EXCEPT
-- subscriptions.tier: it references plan_tiers (3 immutable rows, never deleted)
-- and has only 3 distinct values, so an index would never be selective.

CREATE INDEX IF NOT EXISTS idx_peg_rules_portfolio          ON peg_rules(portfolio_id);
CREATE INDEX IF NOT EXISTS idx_tax_share_tokens_portfolio   ON tax_share_tokens(portfolio_id);
CREATE INDEX IF NOT EXISTS idx_rebalances_decision          ON rebalances(decision_id);
CREATE INDEX IF NOT EXISTS idx_rebalance_events_portfolio   ON rebalance_events(portfolio_id);
CREATE INDEX IF NOT EXISTS idx_rebalance_events_decision    ON rebalance_events(agent_decision_id);
CREATE INDEX IF NOT EXISTS idx_peg_events_rebalance         ON peg_events(rebalance_id);
CREATE INDEX IF NOT EXISTS idx_performance_fees_decision    ON performance_fees(decision_id);
CREATE INDEX IF NOT EXISTS idx_strategies_author            ON strategies(author_user_id);
CREATE INDEX IF NOT EXISTS idx_invoices_subscription        ON invoices(subscription_id);
CREATE INDEX IF NOT EXISTS idx_aum_accruals_invoice         ON aum_accruals(invoice_id);
CREATE INDEX IF NOT EXISTS idx_calibrated_predictions_calibration ON calibrated_predictions(calibration_id);
