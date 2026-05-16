#!/usr/bin/env bash
# Lock .env* file perms to 600 (owner-only read/write).
# Run after editing .env or .env.local by hand.
set -euo pipefail
for f in .env .env.local; do
  if [ -f "$f" ]; then
    chmod 600 "$f"
    echo "chmod 600 $f"
  fi
done
