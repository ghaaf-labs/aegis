-- Sprint 3 — cross-chain rebalance execution, public diary, daily digest.
--
-- `rebalances` is the top-level plan; `rebalance_legs` is the per-step state
-- machine that drives CCTP burns, mints, hook swaps, USYC park/redeem, and
-- StableFX FX swaps. The agent emits one rebalance per decision; the user
-- approves once, the executor walks the legs and broadcasts per-leg SSE.

CREATE TABLE IF NOT EXISTS rebalances (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    portfolio_id    UUID        NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
    decision_id     UUID        NOT NULL REFERENCES agent_decisions(id) ON DELETE CASCADE,
    status          TEXT        NOT NULL CHECK (status IN (
                        'planned','approved','executing','completed','failed','cancelled')),
    total_legs      INTEGER     NOT NULL CHECK (total_legs >= 0),
    completed_legs  INTEGER     NOT NULL DEFAULT 0 CHECK (completed_legs >= 0),
    total_gas_usdc  DOUBLE PRECISION,
    failure_reason  TEXT,
    approved_at     TIMESTAMPTZ,
    completed_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TRIGGER set_updated_at_rebalances
    BEFORE UPDATE ON rebalances FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE INDEX IF NOT EXISTS idx_rebalances_portfolio_at
    ON rebalances(portfolio_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_rebalances_status
    ON rebalances(status)
    WHERE status IN ('planned','approved','executing');


CREATE TABLE IF NOT EXISTS rebalance_legs (
    id                 UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    rebalance_id       UUID        NOT NULL REFERENCES rebalances(id) ON DELETE CASCADE,
    leg_index          INTEGER     NOT NULL CHECK (leg_index >= 0),
    kind               TEXT        NOT NULL CHECK (kind IN (
                            'local_swap','cross_chain_burn','cross_chain_mint',
                            'park_usyc','redeem_usyc','fx_stablefx')),
    src_chain          TEXT,
    dest_chain         TEXT,
    src_symbol         TEXT,
    dest_symbol        TEXT,
    amount_usdc        DOUBLE PRECISION NOT NULL CHECK (amount_usdc >= 0),
    min_out            DOUBLE PRECISION,
    status             TEXT        NOT NULL CHECK (status IN (
                            'pending','submitted','confirmed','failed')),
    tx_hash            TEXT,
    cctp_message_hash  TEXT,
    failure_reason     TEXT,
    submitted_at       TIMESTAMPTZ,
    confirmed_at       TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE (rebalance_id, leg_index)
);

CREATE INDEX IF NOT EXISTS idx_rebalance_legs_plan
    ON rebalance_legs(rebalance_id, leg_index);

CREATE INDEX IF NOT EXISTS idx_rebalance_legs_open
    ON rebalance_legs(status)
    WHERE status IN ('pending','submitted');


-- Diary visibility is opt-in at the portfolio level; default OFF so privacy
-- is the default and share cards require explicit consent.
ALTER TABLE portfolios
    ADD COLUMN IF NOT EXISTS diary_public BOOLEAN NOT NULL DEFAULT FALSE;

CREATE INDEX IF NOT EXISTS idx_portfolios_diary_public
    ON portfolios(user_id)
    WHERE diary_public = TRUE;


-- One subscription per user. Token is signed (HMAC-SHA256 of user_id +
-- DIGEST_SECRET) so unsubscribe does not need auth.
CREATE TABLE IF NOT EXISTS digest_subscriptions (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID        NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    email               TEXT        NOT NULL,
    unsubscribe_token   TEXT        NOT NULL UNIQUE,
    last_sent_at        TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_digest_subscriptions_token
    ON digest_subscriptions(unsubscribe_token);
