-- Add settlement_tx_hash for Nanopayments (x402) 25bps protocol fee settlements.
-- Populated when settle_protocol_fee_via_nanopayments succeeds.
ALTER TABLE rebalance_fees
  ADD COLUMN IF NOT EXISTS settlement_tx_hash TEXT;

CREATE INDEX IF NOT EXISTS idx_rebalance_fees_settlement ON rebalance_fees(settlement_tx_hash) WHERE settlement_tx_hash IS NOT NULL;