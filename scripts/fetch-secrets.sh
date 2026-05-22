#!/usr/bin/env bash
# Pulls secrets from Infisical (EU) and writes /opt/aegis/.env.
# Requires /opt/aegis/.infisical-creds with:
#   INFISICAL_MACHINE_IDENTITY_CLIENT_ID
#   INFISICAL_MACHINE_IDENTITY_CLIENT_SECRET
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CREDS_FILE="/opt/aegis/.infisical-creds"
OUT_FILE="/opt/aegis/.env"
INFISICAL_URL="https://eu.infisical.com"
WORKSPACE_ID="8b2bce62-4e5f-41a0-aec6-0e19ec4d90d2"
ENVIRONMENT="prod"

if [[ ! -f "$CREDS_FILE" ]]; then
  echo "ERROR: $CREDS_FILE not found" >&2
  exit 1
fi
# shellcheck source=/dev/null
source "$CREDS_FILE"

TOKEN=$(curl -sf -X POST "$INFISICAL_URL/api/v1/auth/universal-auth/login" \
  -H 'Content-Type: application/json' \
  -d "{\"clientId\":\"$INFISICAL_MACHINE_IDENTITY_CLIENT_ID\",\"clientSecret\":\"$INFISICAL_MACHINE_IDENTITY_CLIENT_SECRET\"}" \
  | jq -r '.accessToken')

if [[ -z "$TOKEN" || "$TOKEN" == "null" ]]; then
  echo "ERROR: Failed to obtain Infisical token" >&2
  exit 1
fi

curl -sf "$INFISICAL_URL/api/v3/secrets/raw?workspaceId=$WORKSPACE_ID&environment=$ENVIRONMENT&secretPath=/" \
  -H "Authorization: Bearer $TOKEN" \
  | jq -r '.secrets[] | "\(.secretKey)=\(.secretValue)"' \
  > "$OUT_FILE"

chmod 600 "$OUT_FILE"
echo "Secrets written to $OUT_FILE ($(wc -l < "$OUT_FILE") vars)"
