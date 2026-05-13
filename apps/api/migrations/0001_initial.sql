-- ── Users ──────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS users (
    id                        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email                     TEXT NOT NULL UNIQUE,
    password_hash             TEXT NOT NULL,
    risk_tolerance            TEXT NOT NULL DEFAULT 'moderate'
                              CHECK (risk_tolerance IN ('conservative', 'moderate', 'aggressive')),
    investment_horizon_months INTEGER NOT NULL DEFAULT 12,
    created_at                TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at                TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ── Portfolios ─────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS portfolios (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            TEXT NOT NULL DEFAULT 'My Portfolio',
    total_value_usd DOUBLE PRECISION NOT NULL DEFAULT 0,
    total_pnl_usd   DOUBLE PRECISION NOT NULL DEFAULT 0,
    total_pnl_pct   DOUBLE PRECISION NOT NULL DEFAULT 0,
    risk_score      INTEGER NOT NULL DEFAULT 50 CHECK (risk_score BETWEEN 0 AND 100),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_portfolios_user_id ON portfolios(user_id);

-- ── Assets ─────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS assets (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    symbol        TEXT NOT NULL UNIQUE,
    name          TEXT NOT NULL,
    coingecko_id  TEXT NOT NULL,
    logo_url      TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

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

-- ── Allocations ────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS allocations (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    portfolio_id    UUID NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
    asset_symbol    TEXT NOT NULL,
    quantity        DOUBLE PRECISION NOT NULL DEFAULT 0,
    target_weight   DOUBLE PRECISION NOT NULL DEFAULT 0 CHECK (target_weight BETWEEN 0 AND 100),
    current_weight  DOUBLE PRECISION NOT NULL DEFAULT 0,
    value_usd       DOUBLE PRECISION NOT NULL DEFAULT 0,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (portfolio_id, asset_symbol)
);

CREATE INDEX idx_allocations_portfolio_id ON allocations(portfolio_id);

-- ── Agent Decisions ────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS agent_decisions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    portfolio_id    UUID NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
    reasoning       TEXT NOT NULL,
    recommendation  JSONB NOT NULL DEFAULT '{}',
    confidence      DOUBLE PRECISION NOT NULL DEFAULT 0 CHECK (confidence BETWEEN 0 AND 1),
    triggered_by    TEXT NOT NULL DEFAULT 'scheduled'
                    CHECK (triggered_by IN ('market_movement', 'drift_threshold', 'risk_breach', 'scheduled', 'user_request')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agent_decisions_portfolio_id ON agent_decisions(portfolio_id);
CREATE INDEX idx_agent_decisions_created_at  ON agent_decisions(created_at DESC);

-- ── Rebalance Events ───────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS rebalance_events (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    portfolio_id      UUID NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
    agent_decision_id UUID REFERENCES agent_decisions(id),
    status            TEXT NOT NULL DEFAULT 'pending'
                      CHECK (status IN ('pending', 'approved', 'executing', 'completed', 'failed', 'cancelled')),
    trades            JSONB NOT NULL DEFAULT '[]',
    executed_at       TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ── Market Snapshots ───────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS market_snapshots (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    assets               JSONB NOT NULL DEFAULT '[]',
    fear_greed_index     SMALLINT NOT NULL DEFAULT 50,
    total_market_cap_usd DOUBLE PRECISION NOT NULL DEFAULT 0,
    btc_dominance        DOUBLE PRECISION NOT NULL DEFAULT 0,
    captured_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_market_snapshots_captured_at ON market_snapshots(captured_at DESC);

-- ── Triggers ───────────────────────────────────────────────────────────────
CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER AS $$
BEGIN NEW.updated_at = NOW(); RETURN NEW; END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER users_updated_at       BEFORE UPDATE ON users       FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER portfolios_updated_at  BEFORE UPDATE ON portfolios  FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER allocations_updated_at BEFORE UPDATE ON allocations FOR EACH ROW EXECUTE FUNCTION set_updated_at();
