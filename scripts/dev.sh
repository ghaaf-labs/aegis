#!/usr/bin/env bash
# scripts/dev.sh — multi-agent dev runtime for Aegis.
#
# Supervises the Rust API and the Next.js web server inside detached tmux
# sessions so several agents (Claude Code, Codex, a human) can start, restart,
# tail, and hand off the same dev servers without trampling each other or
# fighting over ports. See docs/14-dev-runtime.md for the protocol.
#
#   Main checkout      → canonical ports  api :8080  web :3000
#   A linked worktree  → deterministic offset ports derived from its path,
#                        so sibling worktrees never collide.
#
# Quick start:
#   scripts/dev.sh up            # ensure api + web are running (idempotent)
#   scripts/dev.sh status        # what's running, where, who owns it
#   scripts/dev.sh logs api      # tail recent API output
#   scripts/dev.sh restart api   # restart just the API in place
#   scripts/dev.sh claim "wiring SSE"   # advisory: tell other agents you're on it
#
# macOS-friendly: no flock (atomic mkdir locks), no bash-4 features, cksum hash.
set -euo pipefail

# ── identity: where are we, and which ports do we own? ───────────────────────
ROOT="$(git rev-parse --show-toplevel)"
if [ "$(git rev-parse --git-dir)" = "$(git rev-parse --git-common-dir)" ]; then
  SLUG="main"; OFFSET=0                       # primary checkout → canonical ports
else
  base="$(basename "$ROOT")"; base="${base#aegis-}"
  SLUG="$(printf '%s' "$base" | tr -c 'a-zA-Z0-9_.' '-' | sed -E 's/^-+|-+$//g')"
  [ -n "$SLUG" ] || SLUG="wt"
  h="$(printf '%s' "$ROOT" | cksum | cut -d' ' -f1)"
  OFFSET=$(( h % 200 + 1 ))                    # stable per-worktree offset 1..200
fi
API_PORT=$(( 8080 + OFFSET ))
WEB_PORT=$(( 3000 + OFFSET ))
SESSION="aegis-${SLUG}"
OWNER="${DEV_AGENT:-${USER:-agent}}"

STATE_DIR="$ROOT/.dev"
LOG_DIR="$STATE_DIR/logs"
LOCK_DIR="$STATE_DIR/locks"
STATUS_FILE="$STATE_DIR/status"
mkdir -p "$LOG_DIR" "$LOCK_DIR"

# The exact shell command each service's tmux pane runs. It carries its own PATH
# (dev.sh runs with the user's PATH, so cargo/pnpm/node resolve) and execs the
# server directly — no send-keys into an interactive shell, which a heavy zsh
# (p10k / autosuggestions / bracketed-paste) mangles. Override the inner command
# with DEV_API_CMD / DEV_WEB_CMD (e.g. DEV_API_CMD="cargo run --features real-cctp").
svc_inner() {
  local body
  if [ "$1" = api ]; then
    body="cd $(printf %q "$ROOT/apps/api") && exec env API_HOST=127.0.0.1 API_PORT=$API_PORT ${DEV_API_CMD:-cargo run --bin aegis-api}"
  else
    body="cd $(printf %q "$ROOT/apps/web") && exec env PORT=$WEB_PORT NEXT_PUBLIC_API_URL=http://localhost:$API_PORT ${DEV_WEB_CMD:-pnpm dev}"
  fi
  printf 'export PATH=%s; %s' "$(printf %q "$PATH")" "$body"
}

die() { printf '\033[31m✗ %s\033[0m\n' "$*" >&2; exit 1; }
ok()  { printf '\033[32m✓ %s\033[0m\n' "$*"; }
info(){ printf '  %s\n' "$*"; }

# ── probes (port truth beats lock files; the lock here is advisory only) ─────
port_pid()    { lsof -ti "tcp:$1" -sTCP:LISTEN 2>/dev/null | head -1 || true; }
api_healthy() { curl -fsS -m 2 "http://127.0.0.1:$API_PORT/health" >/dev/null 2>&1; }
web_healthy() { curl -fsS -m 2 -o /dev/null "http://127.0.0.1:$WEB_PORT" 2>/dev/null; }
svc_port()    { [ "$1" = api ] && echo "$API_PORT" || echo "$WEB_PORT"; }
# Real per-service branch: a failing `api_healthy` must NOT fall through to
# `web_healthy` (the old `&& ||` form reported the API up whenever the web port
# was), which masked a crashed API pane as healthy.
svc_healthy() { if [ "$1" = api ]; then api_healthy; else web_healthy; fi; }
have_session(){ tmux has-session -t "$SESSION" 2>/dev/null; }
have_window() { tmux list-windows -t "$SESSION" -F '#{window_name}' 2>/dev/null | grep -qx "$1"; }
mtime()       { stat -c %Y "$1" 2>/dev/null || stat -f %m "$1" 2>/dev/null || echo 0; }  # GNU first, then BSD

# ── advisory start-lock (atomic mkdir; auto-steals a >10-min-old lock) ───────
HELD_LOCKS=""
cleanup_locks() { local l; for l in $HELD_LOCKS; do rmdir "$LOCK_DIR/$l" 2>/dev/null || true; done; }
trap cleanup_locks EXIT
acquire() {
  local d="$LOCK_DIR/$1"
  if mkdir "$d" 2>/dev/null; then HELD_LOCKS="$HELD_LOCKS $1"; return 0; fi
  local age=$(( $(date +%s) - $(mtime "$d") ))
  if [ "$age" -gt 600 ]; then
    rmdir "$d" 2>/dev/null || true
    mkdir "$d" 2>/dev/null && { HELD_LOCKS="$HELD_LOCKS $1"; return 0; }
  fi
  die "another agent is (re)starting '$1' (lock ${age}s old). Retry shortly, or 'down' first."
}
release() { rmdir "$LOCK_DIR/$1" 2>/dev/null || true; }

write_status() { # state, note
  printf '{"slug":"%s","owner":"%s","state":"%s","note":"%s","session":"%s","api_port":%s,"web_port":%s,"ts":%s}\n' \
    "$SLUG" "$OWNER" "$1" "${2:-}" "$SESSION" "$API_PORT" "$WEB_PORT" "$(date +%s)" > "$STATUS_FILE"
}

# ── start / restart / stop ───────────────────────────────────────────────────
start_svc() { # svc, [restart]
  local svc="$1" restart="${2:-}" inner p
  if [ -z "$restart" ] && svc_healthy "$svc"; then
    ok "$svc already up on :$(svc_port "$svc")"; return 0
  fi
  acquire "$svc"
  # A plain `up` must not trample a service another agent is already booting.
  # The lock only spans launch (a Rust compile can outlast the 10-min steal),
  # so use pane liveness as the readiness signal: a LIVE pane is starting/running
  # — leave it. Only a dead pane (crashed) or an explicit `restart` respawns.
  if [ -z "$restart" ] && have_window "$svc" \
     && [ "$(tmux display -p -t "$SESSION:$svc" '#{pane_dead}' 2>/dev/null || echo 1)" != 1 ]; then
    release "$svc"
    ok "$svc already starting/running on :$(svc_port "$svc")"
    info "tail it:  scripts/dev.sh logs $svc"
    return 0
  fi
  inner="$(svc_inner "$svc")"
  if ! have_session; then
    tmux new-session -d -s "$SESSION" -n "$svc" -c "$ROOT" "$inner"
  elif have_window "$svc"; then                    # restart, or revive a crashed (dead) pane
    p="$(port_pid "$(svc_port "$svc")")"; [ -n "$p" ] && kill "$p" 2>/dev/null || true
    wait_port_free "$(svc_port "$svc")"            # kill can orphan workers; let the port actually release
    tmux respawn-pane -k -t "$SESSION:$svc" "$inner"
  else
    tmux new-window -t "$SESSION" -n "$svc" -c "$ROOT" "$inner"
  fi
  tmux setw -t "$SESSION:$svc" remain-on-exit on 2>/dev/null || true   # keep dead pane visible on crash
  tmux pipe-pane -o -t "$SESSION:$svc" "cat >> $(printf '%q' "$LOG_DIR/$svc.log")" 2>/dev/null || true
  write_status starting "$svc"
  release "$svc"
  ok "$svc launching on :$(svc_port "$svc")  (tmux $SESSION:$svc)"
  info "tail it:  scripts/dev.sh logs $svc"
}

stop_svc() { # svc
  local svc="$1" p
  if have_window "$svc"; then
    tmux send-keys -t "$SESSION:$svc" C-c 2>/dev/null || true; sleep 1
    tmux kill-window -t "$SESSION:$svc" 2>/dev/null || true
  fi
  p="$(port_pid "$(svc_port "$svc")")"; [ -n "$p" ] && kill "$p" 2>/dev/null || true
  ok "$svc stopped"
}

wait_port_free() { local port="$1" n=10; while [ "$n" -gt 0 ] && [ -n "$(port_pid "$port")" ]; do sleep 0.5; n=$(( n - 1 )); done; }

wait_healthy() { # svc, [secs]
  local svc="$1" n="${2:-45}"
  while [ "$n" -gt 0 ]; do
    svc_healthy "$svc" && { ok "$svc healthy on :$(svc_port "$svc")"; return 0; }
    sleep 1; n=$(( n - 1 ))
  done
  info "$svc not answering yet on :$(svc_port "$svc") — still compiling? check 'logs $svc'"
}

# ── status / logs / misc ─────────────────────────────────────────────────────
line() { # svc
  local svc="$1" port pid health win
  port="$(svc_port "$svc")"; pid="$(port_pid "$port")"
  if svc_healthy "$svc"; then health='\033[32mUP\033[0m  '; else health='\033[33mdown\033[0m'; fi
  have_window "$svc" && win="tmux✓" || win="tmux✗"
  printf '  %-3s :%-5s  %b  %-5s  pid=%s\n' "$svc" "$port" "$health" "$win" "${pid:-–}"
}

cmd_status() {
  printf '\033[1maegis dev · %s\033[0m  (port offset %s)\n' "$SLUG" "$OFFSET"
  line api; line web
  if [ -f "$STATUS_FILE" ]; then
    local state owner note ts age flag=""
    state="$(sed -E 's/.*"state":"([^"]*)".*/\1/' "$STATUS_FILE")"
    owner="$(sed -E 's/.*"owner":"([^"]*)".*/\1/' "$STATUS_FILE")"
    note="$(sed -E 's/.*"note":"([^"]*)".*/\1/' "$STATUS_FILE")"
    ts="$(sed -E 's/.*"ts":([0-9]+).*/\1/' "$STATUS_FILE")"
    age=$(( $(date +%s) - ${ts:-0} ))
    [ "$age" -gt 7200 ] && flag=' \033[33m(stale — likely abandoned)\033[0m'
    printf '  lock  %s by %s%s%b\n' "$state" "$owner" "${note:+ — \"$note\"}" "$flag"
  fi
}

cmd_logs() { # svc, [n]
  local svc="${1:?usage: logs <api|web> [lines]}" n="${2:-80}"
  local f="$LOG_DIR/$svc.log"
  if [ -s "$f" ]; then tail -n "$n" "$f"
  elif have_window "$svc"; then tmux capture-pane -p -t "$SESSION:$svc" -S "-$n"
  else die "no logs for '$svc' (not started in this checkout)"; fi
}

cmd_doctor() {
  local b p
  for b in tmux curl lsof git; do command -v "$b" >/dev/null && ok "$b" || die "$b missing"; done
  printf 'checkout: %s\nslug: %s   offset: %s\napi: :%s    web: :%s\nsession: %s\n' \
    "$ROOT" "$SLUG" "$OFFSET" "$API_PORT" "$WEB_PORT" "$SESSION"
  for b in api web; do
    p="$(port_pid "$(svc_port "$b")")"
    [ -n "$p" ] && ! have_window "$b" && \
      printf '\033[33m! :%s held by foreign pid %s (another checkout?). It will block %s here — down it first.\033[0m\n' \
        "$(svc_port "$b")" "$p" "$b"
  done
  return 0
}

usage() {
  cat <<EOF
aegis dev runtime — tmux-supervised api+web, worktree-aware ports.

  up   [api|web|all]    ensure running (idempotent). default: all
  restart [api|web|all] restart in place — the multi-agent hand-off op
  down [api|web|all]    stop. default: all
  status                what's running, ports, owner, staleness
  logs <api|web> [n]    last n lines (default 80)
  attach                tmux attach to this checkout's session
  ports                 print API_PORT / WEB_PORT (eval-friendly)
  claim "<reason>"      advisory: mark "under work" so others back off
  release               clear the advisory lock
  broken "<why>"        advisory: mark the env broken
  doctor                check deps + flag foreign port holders

This checkout: $SLUG  →  api :$API_PORT   web :$WEB_PORT
EOF
}

main() {
  local cmd="${1:-status}"; shift || true
  case "$cmd" in
    up)      case "${1:-all}" in
               api) start_svc api ;; web) start_svc web ;;
               *) start_svc api; start_svc web; wait_healthy api; wait_healthy web ;; esac ;;
    restart) case "${1:-all}" in
               api) start_svc api 1; wait_healthy api ;; web) start_svc web 1; wait_healthy web ;;
               *) start_svc api 1; start_svc web 1; wait_healthy api; wait_healthy web ;; esac ;;
    down)    case "${1:-all}" in
               api) stop_svc api ;; web) stop_svc web ;;
               *) stop_svc api; stop_svc web
                  tmux kill-session -t "$SESSION" 2>/dev/null || true; write_status idle ;; esac ;;
    status)  cmd_status ;;
    logs)    cmd_logs "$@" ;;
    attach)  have_session || die "no session $SESSION — run 'up' first"
             [ -n "${TMUX:-}" ] && tmux switch-client -t "$SESSION" || tmux attach -t "$SESSION" ;;
    ports)   printf 'API_PORT=%s\nWEB_PORT=%s\n' "$API_PORT" "$WEB_PORT" ;;
    claim)   write_status under-work "${1:?usage: claim \"reason\"}"; ok "marked under-work by $OWNER" ;;
    release) write_status idle; ok "lock released" ;;
    broken)  write_status broken "${1:?usage: broken \"why\"}"; ok "marked broken: $1" ;;
    doctor)  cmd_doctor ;;
    -h|--help|help) usage ;;
    *)       usage; die "unknown command: $cmd" ;;
  esac
}
main "$@"
