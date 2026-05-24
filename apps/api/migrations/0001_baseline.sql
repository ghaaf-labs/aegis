-- ════════════════════════════════════════════════════════════════════════════
-- 0001 — Squashed baseline schema
-- ════════════════════════════════════════════════════════════════════════════
--
-- This single migration is the squash of the original 0001–0039 migration
-- history. It reproduces the exact live schema (verified byte-for-byte against
-- a `pg_dump --schema-only` of a DB built from all 39 prior migrations).
--
-- Why squashed: the prior history churned several invariants (single-portfolio
-- toggled add→drop→re-add→constraint across 0019/0023/0034/0036; auth reworked
-- across 0020/0025/0026/0027) so the file count no longer reflected the schema.
-- A baseline is the canonical current state; new changes go in 0002_*.sql onward.
--
-- NOTE for existing databases: a DB that already ran 0001–0039 has those
-- versions in `_sqlx_migrations`. Applying this baseline on top will fail the
-- checksum/version reconciliation — reset such a DB (or reconcile the ledger by
-- hand). Fresh DBs (CI, new clones) migrate cleanly from this file alone.

CREATE FUNCTION public.set_updated_at() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN NEW.updated_at = NOW(); RETURN NEW; END;
$$;

CREATE TABLE public.account_export_jobs (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    archive jsonb NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    delivered_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.agent_decisions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    portfolio_id uuid NOT NULL,
    reasoning text NOT NULL,
    recommendation jsonb DEFAULT '{}'::jsonb NOT NULL,
    confidence double precision DEFAULT 0 NOT NULL,
    triggered_by text DEFAULT 'scheduled'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    model_slug text,
    regime text,
    prompt_tokens integer,
    completion_tokens integer,
    latency_ms integer,
    critic_verdict jsonb,
    snapshot jsonb DEFAULT '{}'::jsonb NOT NULL,
    raw_confidence double precision,
    calibrated_confidence double precision,
    counterfactual text,
    kind text DEFAULT 'rebalance'::text NOT NULL,
    recommended_allocation jsonb,
    allocation_applied_at timestamp with time zone,
    CONSTRAINT agent_decisions_confidence_check CHECK (((confidence >= (0)::double precision) AND (confidence <= (1)::double precision))),
    CONSTRAINT agent_decisions_regime_check CHECK (((regime IS NULL) OR (regime = ANY (ARRAY['risk_on'::text, 'neutral'::text, 'risk_off'::text])))),
    CONSTRAINT agent_decisions_triggered_by_check CHECK ((triggered_by = ANY (ARRAY['market_movement'::text, 'drift_threshold'::text, 'risk_breach'::text, 'scheduled'::text, 'user_request'::text, 'regime_flip'::text, 'abstain'::text, 'peg_alert'::text])))
);

COMMENT ON COLUMN public.agent_decisions.raw_confidence IS 'Strategist''s self-reported confidence pre-calibration. Equal to `confidence` for backfill.';

COMMENT ON COLUMN public.agent_decisions.calibrated_confidence IS 'Confidence after the A8 calibrator is applied. Equal to `raw_confidence` when no calibration exists yet.';

COMMENT ON COLUMN public.agent_decisions.counterfactual IS 'One-sentence critic counterfactual gated by CALIBRATED_CONF_ENABLED.';

CREATE TABLE public.agent_memory (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    portfolio_id uuid NOT NULL,
    decision_id uuid NOT NULL,
    outcome_24h jsonb,
    recorded_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.allocations (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    portfolio_id uuid NOT NULL,
    asset_symbol text NOT NULL,
    quantity double precision DEFAULT 0 NOT NULL,
    target_weight double precision DEFAULT 0 NOT NULL,
    current_weight double precision DEFAULT 0 NOT NULL,
    value_usd double precision DEFAULT 0 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT allocations_target_weight_check CHECK (((target_weight >= (0)::double precision) AND (target_weight <= (100)::double precision)))
);

CREATE TABLE public.analytics_events (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid,
    event_name text NOT NULL,
    properties jsonb DEFAULT '{}'::jsonb NOT NULL,
    occurred_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.assets (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    symbol text NOT NULL,
    name text NOT NULL,
    coingecko_id text NOT NULL,
    logo_url text,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.aum_accruals (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    subscription_id uuid NOT NULL,
    invoice_id uuid,
    period_start timestamp with time zone NOT NULL,
    period_end timestamp with time zone NOT NULL,
    aum_snapshot_usd numeric NOT NULL,
    bps integer NOT NULL,
    accrued_usdc numeric NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.auth_rate_limits (
    id text NOT NULL,
    hits integer DEFAULT 0 NOT NULL,
    reset_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.auth_sessions (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    revoked_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    last_seen_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.calibrated_predictions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    decision_id uuid,
    raw_confidence double precision,
    calibrated_confidence double precision,
    calibration_id uuid,
    counterfactual text,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

COMMENT ON TABLE public.calibrated_predictions IS 'Per-decision audit trail: raw → calibrated confidence + critic counterfactual.';

CREATE TABLE public.calibrations (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    model_slug text NOT NULL,
    task text NOT NULL,
    source_eval_run_id uuid,
    method text NOT NULL,
    params_jsonb jsonb NOT NULL,
    fit_samples_count integer DEFAULT 0 NOT NULL,
    brier_before double precision,
    brier_after double precision,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT calibrations_method_check CHECK ((method = ANY (ARRAY['platt'::text, 'isotonic'::text, 'brier_bin'::text])))
);

COMMENT ON TABLE public.calibrations IS 'Trained probability calibrators (isotonic or histogram-bin). One row per fit.';

COMMENT ON COLUMN public.calibrations.task IS 'Free-form task identifier — ''regime_classifier'' or ''strategist_confidence''.';

COMMENT ON COLUMN public.calibrations.method IS 'Calibration family. Histogram-bin (''brier_bin'') is what A8 currently fits; isotonic/platt reserved.';

COMMENT ON COLUMN public.calibrations.params_jsonb IS 'Method-specific fitted params. For brier_bin: { "classes": ["risk_on","neutral","risk_off"], "bins": [{ "lo": 0.0, "hi": 0.1, "empirical": { "risk_on": 0.12, ... }, "n": 14 }, ...] }.';

CREATE TABLE public.cost_basis_lots (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    allocation_id uuid NOT NULL,
    acquired_at timestamp with time zone DEFAULT now() NOT NULL,
    quantity double precision NOT NULL,
    basis_usd double precision NOT NULL,
    disposed_at timestamp with time zone,
    CONSTRAINT cost_basis_lots_basis_usd_check CHECK ((basis_usd >= (0)::double precision)),
    CONSTRAINT cost_basis_lots_quantity_check CHECK ((quantity > (0)::double precision))
);

CREATE TABLE public.digest_subscriptions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    email text NOT NULL,
    unsubscribe_token text NOT NULL,
    last_sent_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.invoices (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    subscription_id uuid,
    period_start timestamp with time zone NOT NULL,
    period_end timestamp with time zone NOT NULL,
    line_items jsonb NOT NULL,
    subtotal_usdc numeric NOT NULL,
    total_usdc numeric NOT NULL,
    status text NOT NULL,
    paid_at timestamp with time zone,
    paid_tx_hash text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT invoices_status_check CHECK ((status = ANY (ARRAY['open'::text, 'paid'::text, 'void'::text, 'past_due'::text])))
);

CREATE TABLE public.market_snapshots (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    assets jsonb DEFAULT '[]'::jsonb NOT NULL,
    fear_greed_index smallint DEFAULT 50 NOT NULL,
    total_market_cap_usd double precision DEFAULT 0 NOT NULL,
    btc_dominance double precision DEFAULT 0 NOT NULL,
    captured_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.model_evaluation_samples (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    eval_run_id uuid NOT NULL,
    observed_at timestamp with time zone NOT NULL,
    predicted_label text NOT NULL,
    predicted_proba jsonb NOT NULL,
    realized_label text NOT NULL,
    features_jsonb jsonb NOT NULL
);

COMMENT ON TABLE public.model_evaluation_samples IS 'Per-sample (predicted, realized) pair. Consumed by A8 calibrated-confidence to fit a Brier calibrator.';

COMMENT ON COLUMN public.model_evaluation_samples.predicted_proba IS 'JSON object: { "risk_on": 0.7, "neutral": 0.2, "risk_off": 0.1 } — sums to ~1.0.';

CREATE TABLE public.model_evaluations (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    model_slug text NOT NULL,
    eval_run_id uuid NOT NULL,
    task text NOT NULL,
    period_start date NOT NULL,
    period_end date NOT NULL,
    samples_count integer NOT NULL,
    accuracy numeric,
    precision_macro numeric,
    recall_macro numeric,
    f1_macro numeric,
    brier_score numeric,
    confusion_jsonb jsonb NOT NULL,
    per_regime_jsonb jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

COMMENT ON TABLE public.model_evaluations IS 'One row per backtest run. Aggregate metrics for the regime classifier and any future model evaluation. Surfaced on the public /about/regime model card.';

COMMENT ON COLUMN public.model_evaluations.task IS 'Free-form task identifier — e.g. ''regime_classifier''.';

COMMENT ON COLUMN public.model_evaluations.confusion_jsonb IS 'JSON object: { "rows": [ [tp_risk_on, fp_risk_on_as_neutral, fp_risk_on_as_risk_off], ... ] } indexed in label-order risk_on, neutral, risk_off.';

COMMENT ON COLUMN public.model_evaluations.per_regime_jsonb IS 'JSON object keyed by regime: { "risk_on": { "precision": 0.6, "recall": 0.5, "f1": 0.54, "support": 120 }, ... }.';

CREATE TABLE public.peg_events (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    rule_id uuid NOT NULL,
    asset text NOT NULL,
    observed_price double precision NOT NULL,
    observed_at timestamp with time zone NOT NULL,
    action_taken text NOT NULL,
    rebalance_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.peg_rules (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    portfolio_id uuid,
    asset text NOT NULL,
    threshold_price double precision NOT NULL,
    window_seconds integer DEFAULT 300 NOT NULL,
    action_kind text NOT NULL,
    target_asset text,
    enabled boolean DEFAULT true NOT NULL,
    paused_at timestamp with time zone,
    last_fired_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT peg_rules_action_kind_check CHECK ((action_kind = ANY (ARRAY['alert'::text, 'propose_rebalance'::text, 'auto_execute'::text]))),
    CONSTRAINT peg_rules_asset_allowed_check CHECK ((upper(asset) = ANY (ARRAY['USDC'::text, 'EURC'::text, 'USYC'::text]))),
    CONSTRAINT peg_rules_target_asset_allowed_check CHECK (((target_asset IS NULL) OR ((upper(target_asset) = ANY (ARRAY['USDC'::text, 'EURC'::text, 'USYC'::text])) AND (upper(target_asset) <> upper(asset))))),
    CONSTRAINT peg_rules_threshold_price_check CHECK ((threshold_price > (0)::double precision)),
    CONSTRAINT peg_rules_threshold_price_depeg_check CHECK (((threshold_price > (0)::double precision) AND (threshold_price <= (1.0)::double precision))),
    CONSTRAINT peg_rules_window_seconds_check CHECK ((window_seconds >= 0))
);

CREATE TABLE public.performance_fees (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    decision_id uuid,
    period text NOT NULL,
    benchmark text NOT NULL,
    realized_gain_usd numeric NOT NULL,
    accrued_bps integer NOT NULL,
    accrued_usdc numeric NOT NULL,
    settled_at timestamp with time zone,
    settlement_tx_hash text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT performance_fees_benchmark_check CHECK ((benchmark = ANY (ARRAY['tbill_3m'::text, 'susds'::text]))),
    CONSTRAINT performance_fees_period_check CHECK ((period = 'monthly'::text))
);

CREATE TABLE public.plan_tiers (
    code text NOT NULL,
    monthly_usd numeric NOT NULL,
    aum_cap_usd numeric,
    portfolios_cap integer,
    decisions_cap_monthly integer,
    per_rebalance_bps integer NOT NULL,
    aum_annual_bps integer NOT NULL
);

CREATE TABLE public.portfolios (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    name text DEFAULT 'My Portfolio'::text NOT NULL,
    total_value_usd double precision DEFAULT 0 NOT NULL,
    total_pnl_usd double precision DEFAULT 0 NOT NULL,
    total_pnl_pct double precision DEFAULT 0 NOT NULL,
    risk_score integer DEFAULT 50 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    goal jsonb DEFAULT '{}'::jsonb NOT NULL,
    diary_public boolean DEFAULT false NOT NULL,
    CONSTRAINT portfolios_risk_score_check CHECK (((risk_score >= 0) AND (risk_score <= 100)))
);

CREATE TABLE public.price_history (
    id bigint NOT NULL,
    symbol text NOT NULL,
    price_usd numeric(20,8) NOT NULL,
    fetched_at timestamp with time zone DEFAULT now() NOT NULL,
    source text DEFAULT 'coingecko'::text NOT NULL
);

COMMENT ON TABLE public.price_history IS 'Historical price ticks used for real statistical features, correlation tool, outcome analysis and backtests. Populated by the market_data ticker on every successful snapshot.';

COMMENT ON COLUMN public.price_history.symbol IS 'Uppercase symbol (BTC, ETH, SOL, ...). Matches the symbols used in allocations and COINGECKO_IDS.';

COMMENT ON COLUMN public.price_history.price_usd IS 'Price in USD at fetch time with high precision.';

COMMENT ON COLUMN public.price_history.source IS 'Provenance of the price (coingecko, binance, etc.). Currently always coingecko.';

CREATE SEQUENCE public.price_history_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.price_history_id_seq OWNED BY public.price_history.id;

CREATE TABLE public.rebalance_events (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    portfolio_id uuid NOT NULL,
    agent_decision_id uuid,
    status text DEFAULT 'pending'::text NOT NULL,
    trades jsonb DEFAULT '[]'::jsonb NOT NULL,
    executed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT rebalance_events_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'approved'::text, 'executing'::text, 'completed'::text, 'failed'::text, 'cancelled'::text])))
);

CREATE TABLE public.rebalance_fees (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    rebalance_id uuid NOT NULL,
    fee_type text NOT NULL,
    amount_usdc double precision NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    settlement_tx_hash text,
    refunded_at timestamp with time zone,
    refund_tx_hash text,
    status text DEFAULT 'settled'::text NOT NULL,
    CONSTRAINT rebalance_fees_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'settled'::text, 'refunded'::text, 'failed'::text])))
);

CREATE TABLE public.rebalance_legs (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    rebalance_id uuid NOT NULL,
    leg_index integer NOT NULL,
    kind text NOT NULL,
    src_chain text,
    dest_chain text,
    src_symbol text,
    dest_symbol text,
    amount_usdc double precision NOT NULL,
    min_out double precision,
    status text NOT NULL,
    tx_hash text,
    cctp_message_hash text,
    failure_reason text,
    submitted_at timestamp with time zone,
    confirmed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    idempotency_key text,
    attempt_count integer DEFAULT 0 NOT NULL,
    stranded_asset boolean DEFAULT false NOT NULL,
    CONSTRAINT rebalance_legs_amount_usdc_check CHECK ((amount_usdc >= (0)::double precision)),
    CONSTRAINT rebalance_legs_attempt_count_check CHECK ((attempt_count >= 0)),
    CONSTRAINT rebalance_legs_kind_check CHECK ((kind = ANY (ARRAY['local_swap'::text, 'cross_chain_burn'::text, 'cross_chain_mint'::text, 'park_usyc'::text, 'redeem_usyc'::text, 'fx_stablefx'::text]))),
    CONSTRAINT rebalance_legs_leg_index_check CHECK ((leg_index >= 0)),
    CONSTRAINT rebalance_legs_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'submitted'::text, 'confirmed'::text, 'failed'::text])))
);

CREATE TABLE public.rebalances (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    portfolio_id uuid NOT NULL,
    decision_id uuid NOT NULL,
    status text NOT NULL,
    total_legs integer NOT NULL,
    completed_legs integer DEFAULT 0 NOT NULL,
    total_gas_usdc double precision,
    failure_reason text,
    approved_at timestamp with time zone,
    completed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    execution_mode text DEFAULT 'mock'::text NOT NULL,
    CONSTRAINT rebalances_completed_legs_check CHECK ((completed_legs >= 0)),
    CONSTRAINT rebalances_execution_mode_check CHECK ((execution_mode = ANY (ARRAY['mock'::text, 'real'::text]))),
    CONSTRAINT rebalances_status_check CHECK ((status = ANY (ARRAY['planned'::text, 'approved'::text, 'executing'::text, 'completed'::text, 'failed'::text, 'cancelled'::text]))),
    CONSTRAINT rebalances_total_legs_check CHECK ((total_legs >= 0))
);

CREATE TABLE public.referrals (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    referrer_user_id uuid NOT NULL,
    new_user_id uuid NOT NULL,
    reward_usdc double precision NOT NULL,
    paid_at timestamp with time zone,
    tx_hash text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT referrals_reward_usdc_check CHECK ((reward_usdc >= (0)::double precision))
);

CREATE TABLE public.strategies (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name text NOT NULL,
    description text NOT NULL,
    risk_band text NOT NULL,
    min_horizon_months integer NOT NULL,
    target_allocation jsonb NOT NULL,
    is_curated boolean DEFAULT false NOT NULL,
    author_user_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT strategies_min_horizon_months_check CHECK ((min_horizon_months >= 1)),
    CONSTRAINT strategies_risk_band_check CHECK ((risk_band = ANY (ARRAY['low'::text, 'medium'::text, 'high'::text])))
);

CREATE TABLE public.subscriptions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    tier text NOT NULL,
    status text NOT NULL,
    started_at timestamp with time zone DEFAULT now() NOT NULL,
    current_period_start timestamp with time zone NOT NULL,
    current_period_end timestamp with time zone NOT NULL,
    cancel_at timestamp with time zone,
    billing_anchor_day integer NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT subscriptions_billing_anchor_day_check CHECK (((billing_anchor_day >= 1) AND (billing_anchor_day <= 28))),
    CONSTRAINT subscriptions_status_check CHECK ((status = ANY (ARRAY['trialing'::text, 'active'::text, 'past_due'::text, 'canceled'::text])))
);

CREATE TABLE public.tax_share_tokens (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    portfolio_id uuid NOT NULL,
    token text NOT NULL,
    year integer NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    revoked_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT tax_share_tokens_year_check CHECK (((year >= 2020) AND (year <= 2100)))
);

CREATE TABLE public.usage_meters (
    user_id uuid NOT NULL,
    period_start date NOT NULL,
    decisions_count integer DEFAULT 0 NOT NULL,
    aum_usd_avg numeric DEFAULT 0 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.user_wallet_networks (
    user_id uuid NOT NULL,
    blockchain text NOT NULL,
    circle_wallet_id text NOT NULL,
    address text NOT NULL,
    account_type text DEFAULT 'SCA'::text NOT NULL,
    wallet_set_id text,
    state text DEFAULT 'LIVE'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.users (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    email text NOT NULL,
    risk_tolerance text DEFAULT 'moderate'::text NOT NULL,
    investment_horizon_months integer DEFAULT 12 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    wallet_id text,
    arc_address text,
    base_address text,
    agent_paused_at timestamp with time zone,
    account_status text DEFAULT 'active'::text NOT NULL,
    custody_model text DEFAULT 'circle_developer'::text NOT NULL,
    wallet_set_id text,
    tos_version text,
    privacy_version text,
    consented_at timestamp with time zone,
    marketing_opt_in boolean DEFAULT false NOT NULL,
    deletion_requested_at timestamp with time zone,
    anonymized_at timestamp with time zone,
    wallet_provision_attempts integer DEFAULT 0 NOT NULL,
    wallet_provision_next_retry_at timestamp with time zone,
    wallet_provision_last_error text,
    auto_pilot_enabled boolean DEFAULT false NOT NULL,
    CONSTRAINT users_account_status_check CHECK ((account_status = ANY (ARRAY['pending_wallet'::text, 'active'::text]))),
    CONSTRAINT users_custody_model_check CHECK ((custody_model = ANY (ARRAY['circle_developer'::text, 'circle_user'::text, 'external'::text]))),
    CONSTRAINT users_risk_tolerance_check CHECK ((risk_tolerance = ANY (ARRAY['conservative'::text, 'moderate'::text, 'aggressive'::text])))
);

CREATE VIEW public.v_trustability_per_user AS
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
           FROM public.portfolios p2
          WHERE (p2.user_id = u.id)), (0.0)::double precision) AS aum_usd
   FROM (((public.users u
     JOIN public.portfolios p ON ((p.user_id = u.id)))
     JOIN public.agent_decisions d ON ((d.portfolio_id = p.id)))
     LEFT JOIN public.agent_memory m ON ((m.decision_id = d.id)))
  WHERE ((d.created_at > (now() - '7 days'::interval)) AND (d.triggered_by <> 'abstain'::text) AND (EXISTS ( SELECT 1
           FROM public.rebalances rb
          WHERE ((rb.decision_id = d.id) AND (rb.status = 'completed'::text) AND (rb.execution_mode = 'real'::text)))))
  GROUP BY u.id;

CREATE TABLE public.wallet_auth_codes (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    email text NOT NULL,
    code_hash text NOT NULL,
    referrer_handle text,
    attempts integer DEFAULT 0 NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    consumed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.price_history ALTER COLUMN id SET DEFAULT nextval('public.price_history_id_seq'::regclass);

ALTER TABLE ONLY public.account_export_jobs
    ADD CONSTRAINT account_export_jobs_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.agent_decisions
    ADD CONSTRAINT agent_decisions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.agent_memory
    ADD CONSTRAINT agent_memory_decision_id_key UNIQUE (decision_id);

ALTER TABLE ONLY public.agent_memory
    ADD CONSTRAINT agent_memory_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.allocations
    ADD CONSTRAINT allocations_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.allocations
    ADD CONSTRAINT allocations_portfolio_id_asset_symbol_key UNIQUE (portfolio_id, asset_symbol);

ALTER TABLE ONLY public.analytics_events
    ADD CONSTRAINT analytics_events_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.assets
    ADD CONSTRAINT assets_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.assets
    ADD CONSTRAINT assets_symbol_key UNIQUE (symbol);

ALTER TABLE ONLY public.aum_accruals
    ADD CONSTRAINT aum_accruals_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.aum_accruals
    ADD CONSTRAINT aum_accruals_subscription_id_period_start_period_end_key UNIQUE (subscription_id, period_start, period_end);

ALTER TABLE ONLY public.auth_rate_limits
    ADD CONSTRAINT auth_rate_limits_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.auth_sessions
    ADD CONSTRAINT auth_sessions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.calibrated_predictions
    ADD CONSTRAINT calibrated_predictions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.calibrations
    ADD CONSTRAINT calibrations_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.cost_basis_lots
    ADD CONSTRAINT cost_basis_lots_allocation_id_acquired_at_basis_usd_key UNIQUE (allocation_id, acquired_at, basis_usd);

ALTER TABLE ONLY public.cost_basis_lots
    ADD CONSTRAINT cost_basis_lots_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.digest_subscriptions
    ADD CONSTRAINT digest_subscriptions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.digest_subscriptions
    ADD CONSTRAINT digest_subscriptions_unsubscribe_token_key UNIQUE (unsubscribe_token);

ALTER TABLE ONLY public.digest_subscriptions
    ADD CONSTRAINT digest_subscriptions_user_id_key UNIQUE (user_id);

ALTER TABLE ONLY public.invoices
    ADD CONSTRAINT invoices_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.market_snapshots
    ADD CONSTRAINT market_snapshots_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.model_evaluation_samples
    ADD CONSTRAINT model_evaluation_samples_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.model_evaluations
    ADD CONSTRAINT model_evaluations_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.peg_events
    ADD CONSTRAINT peg_events_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.peg_rules
    ADD CONSTRAINT peg_rules_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.performance_fees
    ADD CONSTRAINT performance_fees_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.plan_tiers
    ADD CONSTRAINT plan_tiers_pkey PRIMARY KEY (code);

ALTER TABLE ONLY public.portfolios
    ADD CONSTRAINT portfolios_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.portfolios
    ADD CONSTRAINT portfolios_user_id_unique UNIQUE (user_id);

ALTER TABLE ONLY public.price_history
    ADD CONSTRAINT price_history_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.rebalance_events
    ADD CONSTRAINT rebalance_events_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.rebalance_fees
    ADD CONSTRAINT rebalance_fees_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.rebalance_fees
    ADD CONSTRAINT rebalance_fees_rebalance_id_fee_type_key UNIQUE (rebalance_id, fee_type);

ALTER TABLE ONLY public.rebalance_legs
    ADD CONSTRAINT rebalance_legs_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.rebalance_legs
    ADD CONSTRAINT rebalance_legs_rebalance_id_leg_index_key UNIQUE (rebalance_id, leg_index);

ALTER TABLE ONLY public.rebalances
    ADD CONSTRAINT rebalances_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.referrals
    ADD CONSTRAINT referrals_new_user_id_key UNIQUE (new_user_id);

ALTER TABLE ONLY public.referrals
    ADD CONSTRAINT referrals_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.strategies
    ADD CONSTRAINT strategies_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.subscriptions
    ADD CONSTRAINT subscriptions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.tax_share_tokens
    ADD CONSTRAINT tax_share_tokens_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.tax_share_tokens
    ADD CONSTRAINT tax_share_tokens_token_key UNIQUE (token);

ALTER TABLE ONLY public.usage_meters
    ADD CONSTRAINT usage_meters_pkey PRIMARY KEY (user_id, period_start);

ALTER TABLE ONLY public.user_wallet_networks
    ADD CONSTRAINT user_wallet_networks_pkey PRIMARY KEY (user_id, blockchain);

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_email_key UNIQUE (email);

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.wallet_auth_codes
    ADD CONSTRAINT wallet_auth_codes_pkey PRIMARY KEY (id);

CREATE INDEX account_export_jobs_expires_idx ON public.account_export_jobs USING btree (expires_at);

CREATE INDEX account_export_jobs_user_created_idx ON public.account_export_jobs USING btree (user_id, created_at DESC);

CREATE INDEX agent_decisions_portfolio_kind_idx ON public.agent_decisions USING btree (portfolio_id, kind, created_at DESC);

CREATE INDEX idx_agent_decisions_created_at ON public.agent_decisions USING btree (created_at DESC);

CREATE INDEX idx_agent_decisions_portfolio_created ON public.agent_decisions USING btree (portfolio_id, created_at DESC);

CREATE INDEX idx_agent_decisions_portfolio_id ON public.agent_decisions USING btree (portfolio_id);

CREATE INDEX idx_agent_decisions_regime ON public.agent_decisions USING btree (regime) WHERE (regime IS NOT NULL);

CREATE INDEX idx_agent_memory_portfolio ON public.agent_memory USING btree (portfolio_id, recorded_at DESC);

CREATE INDEX idx_allocations_portfolio_id ON public.allocations USING btree (portfolio_id);

CREATE INDEX idx_analytics_events_name_at ON public.analytics_events USING btree (event_name, occurred_at DESC);

CREATE INDEX idx_analytics_events_user_at ON public.analytics_events USING btree (user_id, occurred_at DESC);

CREATE INDEX idx_aum_accruals_subscription ON public.aum_accruals USING btree (subscription_id);

CREATE INDEX idx_aum_accruals_user_invoice ON public.aum_accruals USING btree (user_id, invoice_id);

CREATE INDEX idx_auth_rate_limits_reset ON public.auth_rate_limits USING btree (reset_at);

CREATE INDEX idx_auth_sessions_user_active ON public.auth_sessions USING btree (user_id, expires_at DESC) WHERE (revoked_at IS NULL);

CREATE INDEX idx_calibrated_predictions_decision ON public.calibrated_predictions USING btree (decision_id);

CREATE INDEX idx_calibrations_task_model_created ON public.calibrations USING btree (task, model_slug, created_at DESC);

CREATE INDEX idx_cost_basis_lots_allocation ON public.cost_basis_lots USING btree (allocation_id, acquired_at);

CREATE INDEX idx_cost_basis_lots_open ON public.cost_basis_lots USING btree (allocation_id) WHERE (disposed_at IS NULL);

CREATE INDEX idx_digest_subscriptions_token ON public.digest_subscriptions USING btree (unsubscribe_token);

CREATE INDEX idx_invoices_user_period ON public.invoices USING btree (user_id, period_end DESC);

CREATE INDEX idx_market_snapshots_captured_at ON public.market_snapshots USING btree (captured_at DESC);

CREATE INDEX idx_model_evaluation_samples_run_observed ON public.model_evaluation_samples USING btree (eval_run_id, observed_at);

CREATE INDEX idx_model_evaluations_task_model_created ON public.model_evaluations USING btree (task, model_slug, created_at DESC);

CREATE INDEX idx_peg_events_rule_observed ON public.peg_events USING btree (rule_id, observed_at DESC);

CREATE INDEX idx_peg_rules_user_enabled ON public.peg_rules USING btree (user_id) WHERE ((enabled = true) AND (paused_at IS NULL));

CREATE INDEX idx_performance_fees_user_unsettled ON public.performance_fees USING btree (user_id) WHERE (settled_at IS NULL);

CREATE INDEX idx_portfolios_diary_public ON public.portfolios USING btree (user_id) WHERE (diary_public = true);

CREATE INDEX idx_portfolios_user_id ON public.portfolios USING btree (user_id);

CREATE INDEX idx_price_history_fetched_at ON public.price_history USING btree (fetched_at);

CREATE INDEX idx_price_history_symbol_time ON public.price_history USING btree (symbol, fetched_at DESC);

CREATE INDEX idx_rebalance_fees_rebalance ON public.rebalance_fees USING btree (rebalance_id);

CREATE INDEX idx_rebalance_fees_settlement ON public.rebalance_fees USING btree (settlement_tx_hash) WHERE (settlement_tx_hash IS NOT NULL);

CREATE INDEX idx_rebalance_fees_status ON public.rebalance_fees USING btree (rebalance_id, status);

CREATE INDEX idx_rebalance_legs_open ON public.rebalance_legs USING btree (status) WHERE (status = ANY (ARRAY['pending'::text, 'submitted'::text]));

CREATE INDEX idx_rebalance_legs_plan ON public.rebalance_legs USING btree (rebalance_id, leg_index);

CREATE INDEX idx_rebalance_legs_stranded ON public.rebalance_legs USING btree (rebalance_id) WHERE (stranded_asset = true);

CREATE INDEX idx_rebalances_portfolio_at ON public.rebalances USING btree (portfolio_id, created_at DESC);

CREATE INDEX idx_rebalances_status ON public.rebalances USING btree (status) WHERE (status = ANY (ARRAY['planned'::text, 'approved'::text, 'executing'::text]));

CREATE INDEX idx_referrals_pending ON public.referrals USING btree (paid_at) WHERE (paid_at IS NULL);

CREATE INDEX idx_referrals_referrer ON public.referrals USING btree (referrer_user_id);

CREATE INDEX idx_strategies_curated_risk ON public.strategies USING btree (is_curated, risk_band);

CREATE INDEX idx_subscriptions_user_status ON public.subscriptions USING btree (user_id, status);

CREATE INDEX idx_tax_share_tokens_token ON public.tax_share_tokens USING btree (token);

CREATE INDEX idx_tax_share_tokens_user_at ON public.tax_share_tokens USING btree (user_id, created_at DESC);

CREATE UNIQUE INDEX idx_users_wallet_id ON public.users USING btree (wallet_id) WHERE (wallet_id IS NOT NULL);

CREATE INDEX idx_wallet_auth_codes_email_created ON public.wallet_auth_codes USING btree (email, created_at DESC);

CREATE INDEX idx_wallet_auth_codes_live ON public.wallet_auth_codes USING btree (email, expires_at) WHERE (consumed_at IS NULL);

CREATE UNIQUE INDEX uq_rebalance_legs_idempotency ON public.rebalance_legs USING btree (rebalance_id, idempotency_key) WHERE (idempotency_key IS NOT NULL);

CREATE UNIQUE INDEX uq_subscriptions_user_live ON public.subscriptions USING btree (user_id) WHERE (status = ANY (ARRAY['trialing'::text, 'active'::text, 'past_due'::text]));

CREATE INDEX user_wallet_networks_address_idx ON public.user_wallet_networks USING btree (lower(address));

CREATE INDEX user_wallet_networks_circle_wallet_id_idx ON public.user_wallet_networks USING btree (circle_wallet_id);

CREATE INDEX users_deletion_pending_idx ON public.users USING btree (deletion_requested_at) WHERE ((deletion_requested_at IS NOT NULL) AND (anonymized_at IS NULL));

CREATE INDEX users_wallet_provision_retry_idx ON public.users USING btree (wallet_provision_next_retry_at, updated_at) WHERE ((account_status = 'pending_wallet'::text) AND (deletion_requested_at IS NULL) AND (anonymized_at IS NULL));

CREATE TRIGGER allocations_updated_at BEFORE UPDATE ON public.allocations FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();

CREATE TRIGGER peg_rules_updated_at BEFORE UPDATE ON public.peg_rules FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();

CREATE TRIGGER portfolios_updated_at BEFORE UPDATE ON public.portfolios FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();

CREATE TRIGGER set_updated_at_rebalances BEFORE UPDATE ON public.rebalances FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();

CREATE TRIGGER strategies_updated_at BEFORE UPDATE ON public.strategies FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();

CREATE TRIGGER subscriptions_updated_at BEFORE UPDATE ON public.subscriptions FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();

CREATE TRIGGER usage_meters_updated_at BEFORE UPDATE ON public.usage_meters FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();

CREATE TRIGGER user_wallet_networks_updated_at BEFORE UPDATE ON public.user_wallet_networks FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();

CREATE TRIGGER users_updated_at BEFORE UPDATE ON public.users FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();

ALTER TABLE ONLY public.account_export_jobs
    ADD CONSTRAINT account_export_jobs_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.agent_decisions
    ADD CONSTRAINT agent_decisions_portfolio_id_fkey FOREIGN KEY (portfolio_id) REFERENCES public.portfolios(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.agent_memory
    ADD CONSTRAINT agent_memory_decision_id_fkey FOREIGN KEY (decision_id) REFERENCES public.agent_decisions(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.agent_memory
    ADD CONSTRAINT agent_memory_portfolio_id_fkey FOREIGN KEY (portfolio_id) REFERENCES public.portfolios(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.allocations
    ADD CONSTRAINT allocations_portfolio_id_fkey FOREIGN KEY (portfolio_id) REFERENCES public.portfolios(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.analytics_events
    ADD CONSTRAINT analytics_events_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.aum_accruals
    ADD CONSTRAINT aum_accruals_invoice_id_fkey FOREIGN KEY (invoice_id) REFERENCES public.invoices(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.aum_accruals
    ADD CONSTRAINT aum_accruals_subscription_id_fkey FOREIGN KEY (subscription_id) REFERENCES public.subscriptions(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.aum_accruals
    ADD CONSTRAINT aum_accruals_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.auth_sessions
    ADD CONSTRAINT auth_sessions_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.calibrated_predictions
    ADD CONSTRAINT calibrated_predictions_calibration_id_fkey FOREIGN KEY (calibration_id) REFERENCES public.calibrations(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.calibrated_predictions
    ADD CONSTRAINT calibrated_predictions_decision_id_fkey FOREIGN KEY (decision_id) REFERENCES public.agent_decisions(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.cost_basis_lots
    ADD CONSTRAINT cost_basis_lots_allocation_id_fkey FOREIGN KEY (allocation_id) REFERENCES public.allocations(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.digest_subscriptions
    ADD CONSTRAINT digest_subscriptions_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.invoices
    ADD CONSTRAINT invoices_subscription_id_fkey FOREIGN KEY (subscription_id) REFERENCES public.subscriptions(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.invoices
    ADD CONSTRAINT invoices_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.peg_events
    ADD CONSTRAINT peg_events_rebalance_id_fkey FOREIGN KEY (rebalance_id) REFERENCES public.rebalances(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.peg_events
    ADD CONSTRAINT peg_events_rule_id_fkey FOREIGN KEY (rule_id) REFERENCES public.peg_rules(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.peg_rules
    ADD CONSTRAINT peg_rules_portfolio_id_fkey FOREIGN KEY (portfolio_id) REFERENCES public.portfolios(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.peg_rules
    ADD CONSTRAINT peg_rules_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.performance_fees
    ADD CONSTRAINT performance_fees_decision_id_fkey FOREIGN KEY (decision_id) REFERENCES public.agent_decisions(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.performance_fees
    ADD CONSTRAINT performance_fees_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.portfolios
    ADD CONSTRAINT portfolios_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.rebalance_events
    ADD CONSTRAINT rebalance_events_agent_decision_id_fkey FOREIGN KEY (agent_decision_id) REFERENCES public.agent_decisions(id);

ALTER TABLE ONLY public.rebalance_events
    ADD CONSTRAINT rebalance_events_portfolio_id_fkey FOREIGN KEY (portfolio_id) REFERENCES public.portfolios(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.rebalance_fees
    ADD CONSTRAINT rebalance_fees_rebalance_id_fkey FOREIGN KEY (rebalance_id) REFERENCES public.rebalances(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.rebalance_legs
    ADD CONSTRAINT rebalance_legs_rebalance_id_fkey FOREIGN KEY (rebalance_id) REFERENCES public.rebalances(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.rebalances
    ADD CONSTRAINT rebalances_decision_id_fkey FOREIGN KEY (decision_id) REFERENCES public.agent_decisions(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.rebalances
    ADD CONSTRAINT rebalances_portfolio_id_fkey FOREIGN KEY (portfolio_id) REFERENCES public.portfolios(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.referrals
    ADD CONSTRAINT referrals_new_user_id_fkey FOREIGN KEY (new_user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.referrals
    ADD CONSTRAINT referrals_referrer_user_id_fkey FOREIGN KEY (referrer_user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.strategies
    ADD CONSTRAINT strategies_author_user_id_fkey FOREIGN KEY (author_user_id) REFERENCES public.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.subscriptions
    ADD CONSTRAINT subscriptions_tier_fkey FOREIGN KEY (tier) REFERENCES public.plan_tiers(code);

ALTER TABLE ONLY public.subscriptions
    ADD CONSTRAINT subscriptions_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.tax_share_tokens
    ADD CONSTRAINT tax_share_tokens_portfolio_id_fkey FOREIGN KEY (portfolio_id) REFERENCES public.portfolios(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.tax_share_tokens
    ADD CONSTRAINT tax_share_tokens_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.usage_meters
    ADD CONSTRAINT usage_meters_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.user_wallet_networks
    ADD CONSTRAINT user_wallet_networks_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


-- ── Seed data ────────────────────────────────────────────────────────────────
-- Tradable asset catalogue (was 0001) and the static pricing tiers (was 0010).
-- Idempotent so a re-run never clobbers edited rows.
INSERT INTO assets (symbol, name, coingecko_id) VALUES
    ('BTC',   'Bitcoin',    'bitcoin'),
    ('ETH',   'Ethereum',   'ethereum'),
    ('SOL',   'Solana',     'solana'),
    ('BNB',   'BNB',        'binancecoin'),
    ('AVAX',  'Avalanche',  'avalanche-2'),
    ('LINK',  'Chainlink',  'chainlink'),
    ('UNI',   'Uniswap',    'uniswap'),
    ('MATIC', 'Polygon',    'matic-network')
ON CONFLICT (symbol) DO NOTHING;

INSERT INTO plan_tiers (code, monthly_usd, aum_cap_usd, portfolios_cap, decisions_cap_monthly, per_rebalance_bps, aum_annual_bps)
VALUES
    ('free',     0,   5000, 1,    5,    25, 0),
    ('pro',      19,  NULL, 5,    240,  15, 25),
    ('business', 199, NULL, NULL, NULL, 10, 15)
ON CONFLICT (code) DO NOTHING;
