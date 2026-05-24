-- Phase 1 — Price history for real correlation, volatility, backtests and better regime signals.
--
-- This table is the foundation for:
--   * Real 30d realized vol + 90d cross-asset correlation (instead of 24h proxy)
--   * Real fetch_correlation tool for the strategist
--   * High-quality outcome compressor ("edge vs hold")
--   * Confidence calibration
--   * Backtest preview with historical fidelity
--
-- We keep ~180 days of raw ticks (sufficient for 90d windows + buffer).
-- Older data can be aggregated or pruned later without changing the schema.

CREATE TABLE IF NOT EXISTS price_history (
    id          BIGSERIAL PRIMARY KEY,
    symbol      TEXT           NOT NULL,
    price_usd   NUMERIC(20, 8) NOT NULL,
    fetched_at  TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    source      TEXT           NOT NULL DEFAULT 'coingecko'
);

-- Primary access pattern: "give me the last N days of prices for these symbols"
CREATE INDEX IF NOT EXISTS idx_price_history_symbol_time
    ON price_history (symbol, fetched_at DESC);

-- Useful for range deletes / retention jobs
CREATE INDEX IF NOT EXISTS idx_price_history_fetched_at
    ON price_history (fetched_at);

-- Optional: partial index for recent data (can help query planner on hot path)
-- CREATE INDEX IF NOT EXISTS idx_price_history_recent
--     ON price_history (symbol, fetched_at DESC)
--     WHERE fetched_at > NOW() - INTERVAL '200 days';

COMMENT ON TABLE price_history IS 'Historical price ticks used for real statistical features, correlation tool, outcome analysis and backtests. Populated by the market_data ticker on every successful snapshot.';
COMMENT ON COLUMN price_history.symbol IS 'Uppercase symbol (BTC, ETH, SOL, ...). Matches the symbols used in allocations and COINGECKO_IDS.';
COMMENT ON COLUMN price_history.price_usd IS 'Price in USD at fetch time with high precision.';
COMMENT ON COLUMN price_history.source IS 'Provenance of the price (coingecko, binance, etc.). Currently always coingecko.';