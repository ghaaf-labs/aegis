-- Sprint 4 — decision-time snapshots, referrals, trustability leaderboard.
--
-- The snapshot column lets the outcome compressor + diary counterfactual
-- compute real deltas against captured-at-decision prices instead of the
-- `realized + 0.5` heuristic shipped in Sprint 3.

ALTER TABLE agent_decisions
    ADD COLUMN IF NOT EXISTS snapshot JSONB NOT NULL DEFAULT '{}'::jsonb;

CREATE INDEX IF NOT EXISTS idx_agent_decisions_portfolio_created
    ON agent_decisions(portfolio_id, created_at DESC);


-- Referral attribution + Nanopayment payout audit trail. One reward per
-- newly-referred user, ever — the UNIQUE(new_user_id) lock prevents double
-- payouts if the wallet-create handler fires twice.
CREATE TABLE IF NOT EXISTS referrals (
    id                UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    referrer_user_id  UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    new_user_id       UUID         NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    reward_usdc       DOUBLE PRECISION NOT NULL CHECK (reward_usdc >= 0),
    paid_at           TIMESTAMPTZ,
    tx_hash           TEXT,
    created_at        TIMESTAMPTZ  NOT NULL DEFAULT NOW(),

    UNIQUE (new_user_id)
);

CREATE INDEX IF NOT EXISTS idx_referrals_referrer
    ON referrals(referrer_user_id);

CREATE INDEX IF NOT EXISTS idx_referrals_pending
    ON referrals(paid_at)
    WHERE paid_at IS NULL;


-- Leaderboard view. Joins decisions with their 24h outcome (written by the
-- compressor) and aggregates per-user. Handle is an 8-char prefix of
-- SHA-256(user_id), so users are anonymous-but-stable across reloads.
CREATE OR REPLACE VIEW v_trustability_per_user AS
SELECT
    u.id                                                  AS user_id,
    md5(u.id::text)                                       AS handle_full,
    SUBSTRING(md5(u.id::text), 1, 8)                      AS handle,
    COUNT(d.id)                                           AS decisions_executed,
    COUNT(DISTINCT d.model_slug)                          AS distinct_models,
    COALESCE(AVG((m.outcome_24h->>'realizedPctChange')::float8), 0.0) AS avg_7d_return,
    COALESCE(
        AVG(
            (m.outcome_24h->>'realizedPctChange')::float8
            - (m.outcome_24h->>'counterfactualPctChange')::float8
        ),
        0.0
    )                                                     AS trustability_delta,
    MAX(d.created_at)                                     AS last_decision_at
FROM users u
JOIN portfolios p          ON p.user_id = u.id
JOIN agent_decisions d     ON d.portfolio_id = p.id
LEFT JOIN agent_memory m   ON m.decision_id = d.id
WHERE d.created_at > NOW() - INTERVAL '7 days'
  AND d.triggered_by != 'abstain'
GROUP BY u.id;
