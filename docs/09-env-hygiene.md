# 09 — Environment-variable hygiene

> **Why this exists**: we audited the live `.env` on 2026-05-16 and found four real defects (un-quoted tildes that crash `source .env`, wrapping quotes that confuse non-dotenvy parsers, a `dev-…-change-me` `DIGEST_SECRET` that was never rotated, and a `.env.bak` file committed via `sed -i.bak` that leaked `JWT_SECRET` + `OPENROUTER_API_KEY` to `origin/main`). This doc encodes the hygiene contract so the same shapes don't bite the next contributor.

## 1. File precedence

API binaries call `aegis_api::env::load_env()`, which walks upward to the
workspace root (the directory containing `pnpm-workspace.yaml`) and then loads:

1. `<workspace>/.env.local`
2. `<workspace>/.env`

Because dotenvy's `from_path` never overrides an already-set variable, the
effective precedence is:

```
shell env  >  .env.local  >  .env  >  built-in default
```

- **shell env** wins for anything CI / k8s / `export FOO=bar` already set.
- **`.env.local`** is the maintainer's personal overrides (real secrets, real-exec flags). **Gitignored.**
- **`.env`** is the committed-as-`.env.example` hermetic baseline: mocks on, placeholders for secrets, public addresses real.

The contract is pinned by `apps/api/tests/env_local_precedence.rs` (three tests: `env_local_wins_over_env_for_same_key`, `env_fills_when_env_local_missing_key`, `shell_env_beats_both_files`).

## 2. What goes where

| Kind                                                                            | Where it lives            | Examples                                                                                                       |
| ------------------------------------------------------------------------------- | ------------------------- | -------------------------------------------------------------------------------------------------------------- |
| Public chain addresses (USDC, CCTP V2, executors, Nanopayments seller/treasury) | `.env` (= `.env.example`) | `USDC_ARC`, `CCTP_TOKEN_MESSENGER_BASE`, `REBALANCE_EXECUTOR_BASE`                                             |
| Public RPC URLs                                                                 | `.env`                    | `ARC_RPC_URL=https://rpc.testnet.arc.network`                                                                  |
| Hermetic defaults (mocks ON, flags OFF)                                         | `.env`                    | `EXECUTION_MOCK=true`, `MOCK_CIRCLE=true`, `BILLING_V2_ENABLED=false`                                          |
| Model slugs (with single-quoted tilde prefix)                                   | `.env`                    | `MODEL_CRITIC='~openai/gpt-mini-latest'`                                                                       |
| **Secrets** (any value that, if leaked, opens the account)                      | `.env.local` only         | `JWT_SECRET`, `DIGEST_SECRET`, `CIRCLE_API_KEY`, `OPENROUTER_API_KEY`, `CHAIN_PRIVATE_KEY_*`, `RESEND_API_KEY` |
| **Real-exec overrides** that flip the hermetic defaults                         | `.env.local` only         | `EXECUTION_MOCK=false`, `MOCK_CIRCLE=false`, `BILLING_V2_ENABLED=true`                                         |
| **Real Circle wallet binding**                                                  | `.env.local` only         | `CIRCLE_ENTITY_SECRET`, `CIRCLE_WALLET_SET_ID`                                                                 |

A fresh clone (`git clone … && cp .env.example .env && cargo run --bin aegis-api`
from `apps/api/`) MUST boot cleanly without touching `.env.local`. If a
contributor needs real execution they create workspace-root `.env.local` on top.

When `MOCK_CIRCLE=false`, the API intentionally fails fast unless all three
developer-controlled wallet values are present: `CIRCLE_API_KEY`,
`CIRCLE_ENTITY_SECRET`, and `CIRCLE_WALLET_SET_ID`. Use the API-side setup tool
to check or create the wallet set without printing secrets:

```bash
cd apps/api
cargo run --bin circle_wallet_setup -- check
cargo run --bin circle_wallet_setup -- entity-ciphertext --generate --write-env-local
cargo run --bin circle_wallet_setup -- list
cargo run --bin circle_wallet_setup -- create --name Aegis --write-env-local
```

Paste the `entity-ciphertext` output into Circle's **Entity Secret Ciphertext**
registration field. The command saves the raw 32-byte `CIRCLE_ENTITY_SECRET` to
`.env.local` and does not print it. After Circle accepts the ciphertext, store
Circle's recovery file somewhere offline. `create` writes only
`CIRCLE_WALLET_SET_ID` to `.env.local`; it never logs the API key or entity
secret.

## 3. The four pitfalls we hit live

### 3.1 OpenRouter tilde-prefix slugs crash `source .env`

OpenRouter's "latest pointer" alias uses the `~` prefix (e.g. `~openai/gpt-mini-latest`). zsh treats `~user/` as a home-directory expansion, and bombs when `user` doesn't exist on the system:

```
.env:23: no such user or named directory: openai
```

**Fix**: single-quote the value in `.env`. dotenvy parses single quotes correctly; the shell stops trying to expand the tilde.

```
# wrong (dotenvy works, source .env crashes):
MODEL_CRITIC=~openai/gpt-mini-latest
# right (both parse it):
MODEL_CRITIC='~openai/gpt-mini-latest'
```

### 3.2 Wrapping double-quotes confuse non-dotenvy parsers

dotenvy strips wrapping `"` from values. Most other tools (raw shell, some Python libs, `direnv` in some configs) treat them literally — producing phantom "empty value" errors when the value is actually `"…"` minus the literal quotes.

**Fix**: store values without wrapping quotes in `.env` and `.env.local`. Only quote when the value contains spaces, `#`, or shell metacharacters that need escaping.

```
# wrong:
CIRCLE_API_KEY="TEST_API_KEY:…"
# right:
CIRCLE_API_KEY=TEST_API_KEY:…
```

### 3.3 `source .env` is unsafe — don't use it

Both of the above bite shell `source`. Use the binary's own dotenvy or [`direnv`](https://direnv.net/) instead:

```
# unsafe:
source .env && cargo run
# safe:
cargo run                    # main.rs runs dotenvy itself
direnv allow .               # if you want shell-level access
```

### 3.4 `sed -i.bak` leaves `.env.bak` — gitignore it

`sed -i.bak …` is the macOS-portable way to edit-in-place, but it writes the pre-edit content to `.env.bak`. That file is the same shape as `.env` and contains every secret in plaintext. **`.env.bak` MUST be in `.gitignore`** (it is, after F-ENV-1).

Better: use `sed -i ''` on macOS (in-place, no backup) once you're confident; or use the `scripts/secure-env.sh` helper which never produces a backup.

## 4. Rotation cadence

| Secret                          | Rotation trigger                                               | Effect of rotation                                  |
| ------------------------------- | -------------------------------------------------------------- | --------------------------------------------------- |
| `JWT_SECRET`                    | Suspected leak; quarterly if no incident                       | All existing session JWTs become invalid → re-login |
| `DIGEST_SECRET`                 | Suspected leak; before enabling `RESEND_API_KEY` for real mail | Old unsubscribe-link tokens stop working            |
| `CIRCLE_API_KEY`                | Suspected leak; project-end                                    | API blocks until new key in `.env.local`            |
| `OPENROUTER_API_KEY`            | Suspected leak; monthly hygiene                                | Agent endpoints return 401 until new key            |
| `CHAIN_PRIVATE_KEY_*` (mainnet) | Suspected leak; **rotate via wallet sweep** to a fresh address | Old address is dead; funds must be moved first      |
| `CHAIN_PRIVATE_KEY_*` (testnet) | When you regenerate the EOAs (cheap; tx history is throwaway)  | None — testnet keys carry no real value             |

There is one open pre-deploy blocker in [`docs/05-open-questions.md`](./05-open-questions.md): `PRE-DEPLOY-ROTATE-1` (the OpenRouter API key that was in the committed `.env.bak` history before `git filter-repo` scrubbed `origin/main` on 2026-05-16).

## 5. Verification

Run all of these from the repo root. They should all pass.

```bash
# No unquoted tildes in .env
! grep -qE '^[A-Z_]+=~' .env

# No wrapping double-quotes in .env
! grep -qE '^[A-Z_]+="' .env

# .env.bak gitignored
grep -qxF '.env.bak' .gitignore

# .env.example matches .env keys (drift check; also runs at pre-push)
./scripts/env-key-diff.sh

# Both env files are 600
stat -f '%Sp' .env       # → -rw-------
stat -f '%Sp' .env.local # → -rw------- (if present)

# Clean-shell smoke: dotenvy loads workspace-root files, validate() passes
env -i HOME=$HOME PATH=$PATH ./apps/api/target/debug/aegis-api &
sleep 3 && curl -sS http://127.0.0.1:8080/health  # → {"status":"ok",…}
kill %1
```

## 6. When in doubt

- **"Should this go in `.env` or `.env.local`?"** — If a stranger seeing the value in `git blame` would be a problem, it goes in `.env.local`. Otherwise `.env`.
- **"Should this be required at boot?"** — Required-at-boot vars live in `Config::from_env()` as `required()`; optional ones use `parse_or(…, default)`. Add new required vars to `Config::validate()` so misconfig fails fast at startup, not at first use.
- **"I just leaked a secret to git."** — Rotate first, then `git filter-repo --invert-paths --path <file>` + force-push. Rewriting history does NOT unleak; rotation is the only fix.
