#!/usr/bin/env bash
# Seed a smoke user + portfolio + USDC allocation for the cctp_rebalance_smoke
# binary. Idempotent: drops any prior smoke rows before inserting.
#
# The smoke binary bypasses signup + the Gateway-balance read; this script
# writes the user/portfolio rows it expects, using the locally-generated
# EOA addresses from .env.local.
#
# Usage: ./scripts/seed-rebalance-smoke.sh

set -euo pipefail

DATABASE_URL="${DATABASE_URL:-postgres://aegis:aegis@localhost:5432/aegis}"

USER_ID="aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"
PORTFOLIO_ID="bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"
ARC_ADDR="0xf22C6d6047eC75c21f5845CEA7F83D740e78aa24"
BASE_ADDR="0x0043D379B27fa9367E02cF90F7A17a37Dc2c7a76"

echo "Seeding rebalance-smoke user+portfolio against $DATABASE_URL..."

psql "$DATABASE_URL" <<SQL
BEGIN;

DELETE FROM users WHERE id = '${USER_ID}'::uuid;

INSERT INTO users (id, email, risk_tolerance, investment_horizon_months,
                   arc_address, base_address)
VALUES (
  '${USER_ID}'::uuid,
  'rebalance-smoke@aegis.local',
  'moderate', 12,
  '${ARC_ADDR}',
  '${BASE_ADDR}'
);

INSERT INTO portfolios (id, user_id, name, total_value_usd, goal)
VALUES (
  '${PORTFOLIO_ID}'::uuid,
  '${USER_ID}'::uuid,
  'rebalance-smoke',
  20.00,
  jsonb_build_object(
    'name', 'rebalance smoke goal',
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
echo "Stable UUIDs:"
echo "  USER_ID=${USER_ID}"
echo "  PORTFOLIO_ID=${PORTFOLIO_ID}"
