-- ════════════════════════════════════════════════════════════════════════════
-- 0002 — Money & quantity columns: double precision → numeric
-- ════════════════════════════════════════════════════════════════════════════
--
-- The billing tables (invoices, performance_fees, aum_accruals, plan_tiers,
-- usage_meters) and price_history already store money/prices as `numeric`. The
-- older core tables stored USD values and token quantities as `double
-- precision`, which drifts under repeated arithmetic and violates the project's
-- "money is Decimal, never f64" rule. This migration makes the core columns
-- consistent with billing: exact decimal storage and exact SQL aggregation.
--
-- Scope is money + token-quantity + price columns only. Percentages
-- (target_weight, current_weight, total_pnl_pct, btc_dominance), probabilities
-- (confidence, raw/calibrated_confidence), and integer bps are left as
-- double precision — they are not money and feed float-only statistics.
--
-- The Rust side reads these columns into rust_decimal::Decimal (runtime decode;
-- no column↔field compile check), so this migration ships together with the
-- struct/arithmetic conversion in the same change.

-- The trustability view depends on portfolios.total_value_usd, so Postgres
-- blocks the column retype until the view is dropped. Recreate it afterwards
-- with the sum cast back to double precision: aum_usd stays an f64 boundary for
-- the tier-cap middleware and constitution, which compare against f64 caps.
DROP VIEW IF EXISTS v_trustability_per_user;

ALTER TABLE portfolios
    ALTER COLUMN total_value_usd TYPE numeric USING total_value_usd::numeric,
    ALTER COLUMN total_pnl_usd   TYPE numeric USING total_pnl_usd::numeric;

ALTER TABLE allocations
    ALTER COLUMN quantity  TYPE numeric USING quantity::numeric,
    ALTER COLUMN value_usd TYPE numeric USING value_usd::numeric;

ALTER TABLE market_snapshots
    ALTER COLUMN total_market_cap_usd TYPE numeric USING total_market_cap_usd::numeric;

ALTER TABLE cost_basis_lots
    ALTER COLUMN quantity  TYPE numeric USING quantity::numeric,
    ALTER COLUMN basis_usd TYPE numeric USING basis_usd::numeric;

ALTER TABLE peg_events
    ALTER COLUMN observed_price TYPE numeric USING observed_price::numeric;

ALTER TABLE peg_rules
    ALTER COLUMN threshold_price TYPE numeric USING threshold_price::numeric;

ALTER TABLE rebalance_fees
    ALTER COLUMN amount_usdc TYPE numeric USING amount_usdc::numeric;

ALTER TABLE rebalance_legs
    ALTER COLUMN amount_usdc TYPE numeric USING amount_usdc::numeric,
    ALTER COLUMN min_out     TYPE numeric USING min_out::numeric;

ALTER TABLE rebalances
    ALTER COLUMN total_gas_usdc TYPE numeric USING total_gas_usdc::numeric;

ALTER TABLE referrals
    ALTER COLUMN reward_usdc TYPE numeric USING reward_usdc::numeric;

CREATE VIEW v_trustability_per_user AS
 SELECT u.id AS user_id,
    md5((u.id)::text) AS handle_full,
    "substring"(md5((u.id)::text), 1, 8) AS handle,
    count(d.id) AS decisions_executed,
    count(d.id) AS decisions_per_week,
    count(DISTINCT d.model_slug) AS distinct_models,
    COALESCE(avg(((m.outcome_24h ->> 'realizedPctChange'::text))::double precision), (0.0)::double precision) AS avg_7d_return,
    COALESCE(avg((((m.outcome_24h ->> 'realizedPctChange'::text))::double precision - ((m.outcome_24h ->> 'counterfactualPctChange'::text))::double precision)), (0.0)::double precision) AS trustability_delta,
    max(d.created_at) AS last_decision_at,
    COALESCE(( SELECT sum(p2.total_value_usd) AS sum
           FROM portfolios p2
          WHERE (p2.user_id = u.id))::double precision, (0.0)::double precision) AS aum_usd
   FROM (((users u
     JOIN portfolios p ON ((p.user_id = u.id)))
     JOIN agent_decisions d ON ((d.portfolio_id = p.id)))
     LEFT JOIN agent_memory m ON ((m.decision_id = d.id)))
  WHERE ((d.created_at > (now() - '7 days'::interval)) AND (d.triggered_by <> 'abstain'::text) AND (EXISTS ( SELECT 1
           FROM rebalances rb
          WHERE ((rb.decision_id = d.id) AND (rb.status = 'completed'::text) AND (rb.execution_mode = 'real'::text)))))
  GROUP BY u.id;
