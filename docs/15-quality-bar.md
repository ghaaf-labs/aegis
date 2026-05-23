# 15 — Quality bar

> **What "good code" means here, and which of it is enforced by a machine vs. left to judgement.** Metrics are lint config, not prose you have to remember. This page is the rationale + the knobs.

## Metrics & where they're enforced

| Metric                      | Tool / rule                         | Threshold            | Level                   |
| --------------------------- | ----------------------------------- | -------------------- | ----------------------- |
| Cyclomatic complexity (TS)  | ESLint `complexity`                 | 12 / fn              | warn                    |
| Cognitive complexity (Rust) | clippy `cognitive_complexity`       | 25                   | deny in CI              |
| Function length (TS)        | `max-lines-per-function`            | 80                   | warn                    |
| Function length (Rust)      | clippy `too_many_lines`             | 100                  | _allowed back_ (legacy) |
| File length (TS)            | `max-lines`                         | 400                  | warn                    |
| Nesting depth (TS)          | `max-depth`                         | 4                    | warn                    |
| Params (TS / Rust)          | `max-params` / `too_many_arguments` | 4 / 8                | warn / deny             |
| Statements / fn (TS)        | `max-statements`                    | 25                   | warn                    |
| Type complexity (Rust)      | clippy `type_complexity`            | 250                  | deny                    |
| Duplicate code              | `jscpd` (advisory)                  | ≥8 lines / 50 tokens | report                  |
| Unused TS exports/files     | `knip`                              | —                    | advisory CI             |
| Unused Rust deps            | `cargo-machete`                     | —                    | CI                      |

Rust config: `apps/api/Cargo.toml [lints]` + `apps/api/clippy.toml`. TS config: `apps/web/eslint.config.mjs`. Both wired into existing CI (`api`, `web` jobs).

## Two enforcement styles, on purpose

**Rust is a deny-gate; TypeScript metrics are advisory warnings.** Why the asymmetry:

- CI runs `cargo clippy --all-targets -- -D warnings`, so every Rust lint is effectively a hard error. We use the **ratchet pattern**: `clippy::pedantic` + `clippy::nursery` are enabled as guard-rails for _new_ code, and the ~55 lints that already fire across the 34k-LOC tree are allowed back in `Cargo.toml` (measured, listed there). Net: the build stays green, new code is held to a higher bar. **To raise the bar, delete an allow-back and fix the sites it flags** — don't add new `#[allow]` sprinkles.
- `next lint` / `next build` fail only on _errors_, so the TS size/complexity rules are `warn`. They surface the worst offenders (run `pnpm --filter @aegis/web lint`) without blocking a hackathon merge. Promote one to `error` once its backlog is paid down.

This is the honest move on an existing codebase: you can't retroactively force complexity thresholds without a refactor you didn't ask for, so you **hold the current line and ratchet**.

## Running it

```bash
make api-check           # cargo fmt --check && cargo clippy -- -D warnings
make web-check           # next lint + tsc --noEmit
pnpm --filter @aegis/web lint            # see the complexity/size warnings
pnpm dlx jscpd apps/web/src apps/api/src --min-lines 8 --min-tokens 50   # duplication report
```

`jscpd` needs no install (`pnpm dlx`) and isn't a blocking gate — it's a periodic "where's the copy-paste" scan.

## Beyond what a linter sees

Linters catch size and shape; they don't catch _wrong abstraction_ or _tangled data flow_. These are review judgement, codified from this repo's conventions:

**Abstraction**

- Three similar lines beat a premature wrapper. Don't abstract on the second occurrence; consider it on the third (rule of three).
- No "flexibility" or config that wasn't asked for. A single-caller helper that just renames its argument is noise.
- A `mod`/file should have one reason to change. If `service.rs` mixes HTTP shaping, business logic, and SQL, split by concern — not by line count.

**Data flow**

- Validate at boundaries (user input, external APIs — CoinGecko, Circle, OpenRouter), and trust internal code after that. Don't re-validate the same value at every layer.
- Money is `rust_decimal::Decimal`, never `f64` (rounding drift). Carried as typed domain values, not bare strings, past the boundary.
- One source of truth per fact. Server state in the API; UI server-state via React Query; UI domain state in Zustand. Don't shadow-copy server state into the store.
- SSE is server→client only. Client never infers state a server event should carry.

**Surgical change discipline** (see CLAUDE.md §3)

- Every changed line traces to the task. Don't reformat or "improve" adjacent code.
- Remove only the imports/symbols _your_ change orphaned; flag pre-existing dead code, don't delete it unasked.

## Language checklists

**Rust** — prefer `?` over `unwrap`/`expect` outside tests; `thiserror` for typed errors at boundaries, `anyhow` internally; no `unsafe` (`forbid`-ed in `apps/api`); derive `Debug`; keep handler signatures under the arg threshold (bundle into a request struct); spawn long work on the proactive scheduler, not inline in a handler.

**TypeScript / Next 15** — `strict` is on plus `noUncheckedIndexedAccess` (`packages/config/tsconfig.base.json`); no `any` (use `unknown` + narrow); Server Components by default, `"use client"` only when needed; never leak a secret into a `NEXT_PUBLIC_*` var (see `docs/09-env-hygiene.md`); every external value renders its provenance and every agent decision its `model_slug` (see `docs/04-design-system.md`).

## Tightening later

The thresholds are starting points tuned to _today's_ tree. As areas get cleaned up: drop the relevant clippy allow-back (Rust) or flip a `warn` to `error` (TS), run the gate, fix the fallout in that one area, commit. Ratchet — never loosen a gate to make a red build green.
