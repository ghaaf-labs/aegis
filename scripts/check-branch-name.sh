#!/usr/bin/env bash
# Enforces conventional branch names:
#   feat/<slug>, fix/<slug>, docs/<slug>, chore/<slug>,
#   refactor/<slug>, ci/<slug>, test/<slug>, perf/<slug>, build/<slug>
#
# `main` and `dev` are always allowed (long-lived branches).
# Slug must be kebab-case (lowercase letters, digits, hyphens), 2-60 chars.
#
# Bypass for a single push with `git push --no-verify` (use sparingly).

set -euo pipefail

# In CI on a PR the checked-out ref is the merge SHA. Allow callers to pass
# the real branch name via $GIT_BRANCH_OVERRIDE.
branch="${GIT_BRANCH_OVERRIDE:-$(git rev-parse --abbrev-ref HEAD)}"

case "$branch" in
  main|dev)
    exit 0
    ;;
esac

pattern='^(feat|fix|docs|chore|refactor|ci|test|perf|build)/[a-z0-9][a-z0-9-]{1,59}$'

if [[ "$branch" =~ $pattern ]]; then
  exit 0
fi

cat <<EOF
✗ Branch name "$branch" doesn't match the convention.

Expected: <type>/<kebab-case-slug>
Types:    feat, fix, docs, chore, refactor, ci, test, perf, build
Example:  feat/sprint-1-agent-foundation

Rename with:
    git branch -m <new-name>

Or bypass once (not recommended) with:
    git push --no-verify
EOF
exit 1
