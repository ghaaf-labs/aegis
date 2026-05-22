# Contributing to Aegis

Read [`CLAUDE.md`](./CLAUDE.md) and [`docs/`](./docs/) before opening a PR — they explain the locked-in decisions, the agent loop, and the design system.

## Branches

Branches off `main` use a **type/slug** format. Allowed types: `feat`, `fix`, `docs`, `chore`, `refactor`, `ci`, `test`, `perf`, `build`. Slug is lowercase kebab-case, 2–60 chars.

```
feat/sprint-2-circle-wallets
fix/sse-broadcaster-leak
docs/architecture-diagram
```

Enforced locally by Lefthook's `pre-push` hook (script: `scripts/check-branch-name.sh`) and in CI by the `branch-name` job. `main` and `dev` are exempt.

## Commits — Conventional Commits

```
type(scope): imperative subject

optional body — explain the why, not the what.
```

- **Types:** `feat`, `fix`, `docs`, `refactor`, `chore`, `ci`, `test`, `perf`, `build`, `revert`.
- **Scopes (optional but encouraged):** `web`, `api`, `shared`, `ui`, `agent`, `portfolio`, `auth`, `wallet`, `gateway`, `cctp`, `yield`, `fx`, `tax`, `sse`, `ai`, `risk`, `docs`, `contracts`, `infra`, `deps`.
- Subject in lowercase, no trailing period.
- Header ≤ 100 chars, body lines ≤ 100 chars.
- **Do not** add `Co-authored-by:` trailers for AI tools or `Made-with:` footers anywhere.

Enforced locally by Lefthook's `commit-msg` hook (runs `commitlint`) and in CI on PRs. Config: [`commitlint.config.cjs`](./commitlint.config.cjs).

## Git hooks — Lefthook

We use [Lefthook](https://lefthook.dev) instead of husky + lint-staged: single Go binary, runs hooks in parallel, faster on large repos, native staged-file filtering. Config: [`lefthook.yml`](./lefthook.yml).

After `pnpm install`, hooks are wired automatically via the `postinstall` script. To force a re-install:

```bash
pnpm exec lefthook install
```

| Hook         | What it runs                                                                     |
| ------------ | -------------------------------------------------------------------------------- |
| `commit-msg` | `commitlint --edit {1}`                                                          |
| `pre-commit` | `prettier --write` on staged `*.{ts,tsx,js,jsx,md,json,yml,yaml,css}` (parallel) |
| `pre-push`   | `scripts/check-branch-name.sh`                                                   |

Bypass for one command: `git commit --no-verify` or `git push --no-verify`. Use sparingly.

## Quality gates

CI on every PR:

| Job            | Gate                                                                                           | Blocking?         |
| -------------- | ---------------------------------------------------------------------------------------------- | ----------------- |
| `web`          | `lint` · `type-check` · `vitest run` · `next build`                                            | yes               |
| `web-coverage` | `vitest run --coverage` (artifact uploaded)                                                    | **no** (advisory) |
| `api`          | `cargo fmt --check` · `cargo clippy --all-targets -- -D warnings` · `cargo test --all-targets` | yes               |
| `api-coverage` | `cargo llvm-cov --workspace --lcov` (artifact uploaded)                                        | **no** (advisory) |
| `format`       | `prettier --check` across the whole tree                                                       | yes               |
| `commitlint`   | every commit on PR passes commitlint                                                           | yes               |
| `branch-name`  | PR head branch matches the conventional format                                                 | yes               |
| `typos`        | `crate-ci/typos@v1` — spell-check source + docs                                                | yes               |
| `knip`         | unused TS exports / files / deps                                                               | **no** (advisory) |
| `audit`        | `cargo-audit` (RUSTSEC) + `cargo-deny check` + `cargo-machete` (unused deps)                   | yes               |
| `docker`       | API image builds (on push to `main` only)                                                      | yes               |

## Local pre-flight

To run every CI gate locally before pushing:

```bash
# Frontend
pnpm format:check
pnpm --filter @aegis/web lint
pnpm --filter @aegis/web type-check
pnpm --filter @aegis/web test
pnpm --filter @aegis/web test:coverage   # optional, generates coverage/

# Backend
cd apps/api
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo llvm-cov --all-targets --workspace --summary-only  # optional, requires cargo-llvm-cov

# Spelling
typos                                     # requires `cargo install typos-cli`

# Dependency hygiene (requires cargo-audit, cargo-deny, cargo-machete installed)
cd apps/api
cargo audit --ignore RUSTSEC-2023-0071
cargo deny check
cargo machete

# Unused TS code (advisory)
pnpm dlx knip
```

## Code style

### General principles

- **No comments unless the WHY is non-obvious.** Names + types do the work. Comments should capture invariants, surprising decisions, links to issues, or workarounds — never restate what the code does. See `CLAUDE.md` § Conventions.
- **No premature abstraction.** A bug fix doesn't need a refactor; three similar lines beat a wrapper.
- **Trust internal code.** Validate at boundaries (user input, external APIs), nowhere else.
- **No backwards-compat shims.** Delete the old code, don't dual-path it.

### Rust (`apps/api/`)

- Format with `cargo fmt --all`. CI rejects unformatted code.
- `cargo clippy --all-targets -- -D warnings` must pass — no exemptions in CI.
- New unused fields/variants that will be used in a future sprint get an `#[allow(dead_code)]` with a one-line comment explaining when they'll be used. Use sparingly.
- Errors flow through `AppError` in `src/error.rs`. Don't `panic!` outside of `main` or test code.
- Every public function in `service.rs` files gets a doc comment if its behavior isn't obvious from the signature.
- SQL via SQLx with bind parameters (`$1`, `$2`, …). No string interpolation into queries.

### TypeScript (`apps/web/`, `packages/`)

- `pnpm type-check` must pass (no `tsc` errors).
- `pnpm lint` must pass (Next.js default ruleset; warnings allowed for now).
- Component + hook tests live next to the source as `*.test.ts(x)` (Vitest, jsdom).
- Prefer `@/types` (project alias) over deep imports for shared types.
- No `any` in component props. Use `unknown` + narrowing if a real type is unavailable.

### Comments policy — examples

**Bad** (restates the obvious):

```ts
// Increment counter
counter += 1;
```

**Bad** (references temporary context that rots):

```ts
// Added for the rebalance approval flow on 2026-05-12
```

**Good** (captures a non-obvious invariant):

```ts
// EventSource keeps the connection in CLOSED state after some proxies
// close idle streams; force a manual reopen instead of letting it
// reconnect on its own.
```

## Reporting

If a CI gate fails for reasons unrelated to your change, mention it in the PR description rather than disabling the gate.
