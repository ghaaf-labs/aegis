#!/usr/bin/env bash
# Fail if .env and .env.example have a different set of keys (names only, values irrelevant).
# Used by lefthook pre-push and CI to catch drift early.
set -euo pipefail

if [ ! -f .env ]; then
  echo "skip: .env missing (fresh clone — copy .env.example to .env first)"
  exit 0
fi
if [ ! -f .env.example ]; then
  echo "fail: .env.example missing"
  exit 1
fi

ENV_KEYS=$(grep -oE '^[A-Z_]+' .env | sort -u)
EXAMPLE_KEYS=$(grep -oE '^[A-Z_]+' .env.example | sort -u)

if [ "$ENV_KEYS" = "$EXAMPLE_KEYS" ]; then
  exit 0
fi

echo "❌ .env vs .env.example drift detected:"
diff <(echo "$ENV_KEYS") <(echo "$EXAMPLE_KEYS") || true
echo ""
echo "Fix: regenerate .env.example to match .env (placeholder values for secrets)."
exit 1
