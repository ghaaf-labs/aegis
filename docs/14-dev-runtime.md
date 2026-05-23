# 14 — Dev runtime: many agents, two ports

> **Several agents (Claude Code, Codex, a human) work this repo at once. They must be able to start, restart, tail, and hand off the API and web servers without fighting over `:8080` / `:3000` or leaving orphaned `cargo run` / `pnpm dev` processes behind.** `scripts/dev.sh` is that runtime. Enforce coordination with a mechanism, not etiquette.

## The rule

**Never start a dev server with a bare `cargo run` or `pnpm dev`.** A background process launched from a tool call is invisible to the next agent, can't be restarted by anyone else, and orphans on the port. Always go through:

```bash
scripts/dev.sh up          # ensure api + web are running (idempotent)
scripts/dev.sh status      # what's running, where, who owns it
scripts/dev.sh logs api    # tail recent output (no attach needed)
scripts/dev.sh restart api # restart in place — the hand-off operation
scripts/dev.sh down        # stop both, free the ports
```

It supervises each server in a **detached tmux session**, so the process outlives the tool call that started it and any other agent can see, tail, or restart it.

## How it stays out of its own way

| Concern                             | Mechanism                                                                                                                                                                                                |
| ----------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Server survives the tool call**   | Runs inside a detached tmux session (`aegis-<slug>`), one window per service.                                                                                                                            |
| **Which ports?**                    | Main checkout → canonical `api :8080` / `web :3000`. A linked worktree → a deterministic offset (`cksum` of its path, 1–200) so siblings never collide. `scripts/dev.sh ports` prints them.              |
| **Heavy zsh mangles input**         | The pane runs the server command _directly_ (tmux's non-interactive shell) — no `send-keys` into an interactive shell with p10k / autosuggestions / bracketed-paste. The command carries its own `PATH`. |
| **Restart without orphans**         | `restart` frees the port (kills the listener, waits for release) then `respawn-pane -k` relaunches in the same pane.                                                                                     |
| **Crash stays visible**             | `remain-on-exit on` — a dead server leaves its last output + exit code on screen instead of vanishing.                                                                                                   |
| **Two agents start at once**        | An atomic `mkdir` lock per service (no `flock`, macOS-safe); a >10-min-old lock is auto-stolen. The loser is told to retry.                                                                              |
| **"Don't touch this, I'm working"** | Advisory status: `claim "<reason>"`, `release`, `broken "<why>"`. `status` shows the owner and flags a stale (>2h) claim. Advisory only — port truth always beats the lock file.                         |
| **Health, not hope**                | "Up" means the port answers (`GET /health` for api; a `200` for web), not "a process exists".                                                                                                            |

## Worktree ports

The same checkout always gets the same ports; different checkouts never collide.

```
main checkout (.git == git-common-dir)     → api :8080   web :3000
../aegis-ux            (offset 53)          → api :8133   web :3053
../aegis-hygiene       (offset 117)         → api :8197   web :3117
```

The web server is launched with `NEXT_PUBLIC_API_URL` pointed at _its own_ worktree's API port, so a worktree is fully self-contained. No code changes are needed — the API reads `API_PORT`, Next reads `PORT`.

## Command reference

| Command                                | Does                                                           |
| -------------------------------------- | -------------------------------------------------------------- |
| `up [api\|web\|all]`                   | Ensure running (idempotent). Already-healthy → no-op.          |
| `restart [api\|web\|all]`              | Free the port and relaunch in place.                           |
| `down [api\|web\|all]`                 | Stop; `down all` also kills the session.                       |
| `status`                               | Per-service port, health, tmux state, pid + the advisory lock. |
| `logs <api\|web> [n]`                  | Last `n` lines (default 80) from the persisted pane log.       |
| `attach`                               | `tmux attach` to this checkout's session (human use).          |
| `ports`                                | `API_PORT=… / WEB_PORT=…` (eval-friendly).                     |
| `claim "…"` / `release` / `broken "…"` | Set / clear the advisory status.                               |
| `doctor`                               | Check deps; flag a foreign process squatting on your port.     |

State lives in `.dev/` (gitignored): `logs/`, `locks/`, `status`. Override what a pane runs with `DEV_API_CMD` / `DEV_WEB_CMD`, e.g. `DEV_API_CMD="cargo run --features real-cctp" scripts/dev.sh restart api`.

## Worked example: two agents, one repo

1. Agent A: `scripts/dev.sh up` → tmux `aegis-main` with `api`/`web` windows; both healthy.
2. Agent A: `scripts/dev.sh claim "refactoring rebalance executor"`.
3. Agent B (same checkout): `scripts/dev.sh status` → sees `api UP / web UP`, `lock under-work by A`. It tails (`logs api`) instead of restarting.
4. Agent A edits the API, then `scripts/dev.sh restart api` → same pane, fresh binary, port reused.
5. Agent A: `scripts/dev.sh release`. Done.

If B were in a **worktree** instead, it would just `scripts/dev.sh up` and get its own offset ports — no contention, no lock needed.

## Doesn't

It doesn't replace Docker for Postgres/Redis (`make db-up`), and it isn't a production supervisor. It's a local, multi-agent-safe wrapper around the two dev servers — nothing more.
