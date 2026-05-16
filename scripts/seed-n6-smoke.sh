#!/usr/bin/env bash
# N6.1 — Seed a smoke user + portfolio + USDC allocation for the first real
# CCTP V2 rebalance. Idempotent: deletes any prior smoke rows before inserting.
#
# Bypasses the F-WALLET-1 broken signup path by writing directly to the DB
# with our locally-generated EOA addresses. Bypasses the broken Gateway
# balance read by `apps/api/src/bin/n6_smoke.rs` injecting a synthetic
# `usdc_per_chain` at planner-input time.
#
# Usage:
#   ./scripts/seed-n6-smoke.sh
#
# Outputs the stable UUIDs you can reference from the n6_smoke binary or
# psql queries.

set -euo pipefail

DATABASE_URL="${DATABASE_URL:-postgres://aegis:aegis@localhost:5432/aegis}"

USER_ID="aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"
PORTFOLIO_ID="bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"
ARC_ADDR="0xf22C6d6047eC75c21f5845CEA7F83D740e78aa24"
BASE_ADDR="0x0043D379B27fa9367E02cF90F7A17a37Dc2c7a76"

echo "Seeding N6 smoke user+portfolio against $DATABASE_URL..."

psql "$DATABASE_URL" <<SQL
BEGIN;

-- Idempotent: cascade-drop any prior smoke rows.
DELETE FROM users WHERE id = '${USER_ID}'::uuid;

-- User: real EOA addresses from .env.local. wallet_id stays NULL so the
-- Gateway-balance poller skips this user (the smoke binary injects pool
-- state synthetically anyway).
INSERT INTO users (id, email, risk_tolerance, investment_horizon_months,
                   arc_address, base_address)
VALUES (
  '${USER_ID}'::uuid,
  'n6-smoke@aegis.local',
  'moderate', 12,
  '${ARC_ADDR}',
  '${BASE_ADDR}'
);

-- Portfolio: \$20 of USDC, all on Base today. Goal wants 50/50 USDC/USYC,
-- which forces the planner to cross-chain-burn \$10 USDC from Base → mint
-- on Arc → park into USYC. The first two legs exercise real CCTP V2; the
-- park is still mocked (acceptable for N6, real-USYC lands in N7).
INSERT INTO portfolios (id, user_id, name, total_value_usd, goal)
VALUES (
  '${PORTFOLIO_ID}'::uuid,
  '${USER_ID}'::uuid,
  'n6-smoke',
  20.00,
  jsonb_build_object(
    'name', 'N6 smoke goal',
    'horizon', '1y',
    'riskTolerance', 'moderate',
    'targetAllocation', jsonb_build_object('USDC', 50, 'USYC', 50),
    'monthlyContributionUsd', 0,
    'includeUsyc', true,
    'includeEurc', false,
    'createdAt', NOW()::text
  )
);

INSERT INTO allocations (portfolio_id, asset_symbol, quantity, target_weight, current_weight)
VALUES
  ('${PORTFOLIO_ID}'::uuid, 'USDC', 20.0, 50.0, 100.0),
  ('${PORTFOLIO_ID}'::uuid, 'USYC',  0.0, 50.0,   0.0);

COMMIT;

\echo ''
\echo 'Seeded:'
SELECT id, email, arc_address, base_address FROM users WHERE id = '${USER_ID}'::uuid;
SELECT id, name, total_value_usd FROM portfolios WHERE id = '${PORTFOLIO_ID}'::uuid;
SELECT asset_symbol, quantity, target_weight, current_weight
  FROM allocations WHERE portfolio_id = '${PORTFOLIO_ID}'::uuid;
SQL

echo ""
echo "Stable UUIDs (referenced by n6_smoke binary + Day-2 verification):"
echo "  USER_ID=${USER_ID}"
echo "  PORTFOLIO_ID=${PORTFOLIO_ID}"
