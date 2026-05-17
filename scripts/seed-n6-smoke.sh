#!/usr/bin/env bash
# HS-4 / N6 — seed the rows needed to fire one real CCTP V2 rebalance against
# Base Sepolia → Arc testnet.
#
# Usage:
#   DATABASE_URL=postgres://aegis:aegis@localhost:5432/aegis \
#   ARC_EOA=0xf22C6d6047eC75c21f5845CEA7F83D740e78aa24 \
#   BASE_EOA=0x0043D379B27fa9367E02cF90F7A17a37Dc2c7a76 \
#     ./scripts/seed-n6-smoke.sh
#
# Prereqs:
#   - Postgres reachable at DATABASE_URL
#   - Migrations up-to-date: `cargo sqlx migrate run` from apps/api/
#   - The two EOAs already funded with testnet USDC (per the prior plan's
#     credential provisioning chronicle).
#
# Output: a stable user id and portfolio id you'll pass to forge_test_jwt
# and the rebalance plan/execute curl calls.
set -euo pipefail

USER_ID="aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"
PORT_ID="bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"
: "${DATABASE_URL:?DATABASE_URL must be set}"
: "${ARC_EOA:?ARC_EOA must be set (Arc testnet EOA address)}"
: "${BASE_EOA:?BASE_EOA must be set (Base Sepolia EOA address)}"

psql "$DATABASE_URL" <<SQL
INSERT INTO users (id, email, arc_address, base_address, risk_tolerance, investment_horizon_months)
VALUES (
  '$USER_ID',
  'n6-smoke@aegis.local',
  '$ARC_EOA',
  '$BASE_EOA',
  'moderate',
  12
)
ON CONFLICT (id) DO UPDATE
  SET arc_address = EXCLUDED.arc_address,
      base_address = EXCLUDED.base_address;

INSERT INTO portfolios (id, user_id, name, total_value_usd, goal)
VALUES (
  '$PORT_ID',
  '$USER_ID',
  'n6-smoke',
  20.00,
  jsonb_build_object(
    'name', 'N6 smoke',
    'horizon', '1y',
    'riskTolerance', 'moderate',
    'targetAllocation', jsonb_build_object('USDC', 100),
    'includeUsyc', false,
    'includeEurc', false,
    'createdAt', NOW()
  )
)
ON CONFLICT (id) DO UPDATE
  SET goal = EXCLUDED.goal,
      total_value_usd = EXCLUDED.total_value_usd;

INSERT INTO allocations (portfolio_id, asset_symbol, quantity, target_weight, current_weight)
VALUES ('$PORT_ID', 'USDC', 20.0, 100.0, 100.0)
ON CONFLICT (portfolio_id, asset_symbol) DO UPDATE
  SET quantity = EXCLUDED.quantity,
      target_weight = EXCLUDED.target_weight,
      current_weight = EXCLUDED.current_weight;
SQL

echo "Seeded:"
echo "  USER_ID=$USER_ID"
echo "  PORT_ID=$PORT_ID"
echo ""
echo "Next: mint a JWT with"
echo "  cargo run --bin forge_test_jwt -- $USER_ID > /tmp/jwt"
echo ""
echo "Then plan + execute:"
echo "  curl -X POST http://127.0.0.1:8080/portfolios/$PORT_ID/rebalance/plan \\"
echo "    -H \"Authorization: Bearer \$(cat /tmp/jwt)\""
echo "  curl -X POST http://127.0.0.1:8080/rebalance/<rebalance_id>/execute \\"
echo "    -H \"Authorization: Bearer \$(cat /tmp/jwt)\""
