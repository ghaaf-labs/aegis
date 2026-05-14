# 06 — Harness engineering for this repo

> **The harness is everything that surrounds the agent loop. Enforce quality with mechanisms, not prompts.** A bad harness leaks failures into context; a good harness makes recurring problems impossible. This doc codifies how we run Claude Code (and how a contributor should expect the repo to behave) on the Aegis build.

The principle comes from OpenAI's "harness engineering" article and is the dominant theme across recent Claude Code best-practice writing in 2026 ([HumanLayer](https://www.humanlayer.dev/blog/skill-issue-harness-engineering-for-coding-agents), [Sakasegawa](https://nyosegawa.com/en/posts/harness-engineering-best-practices-2026/), [Anthropic's own docs](https://code.claude.com/docs/en/skills)). We adapt it to a 2-week hackathon timebox.

## The mechanisms we use

Claude Code exposes five harness primitives. We use each one with intent, not by default.

| Primitive | What it is | When we reach for it |
|---|---|---|
| **`CLAUDE.md`** | Project context loaded into every session | Locked decisions + commands + pointers to `docs/`. **Pointer-style, never a manual.** |
| **`.claude/skills/<name>/SKILL.md`** | Reusable instruction packs with `/slash-command` interfaces, loaded on demand | Repeating workflows that have a checklist (migrations, prompt iteration, PR shipping) |
| **`.claude/agents/<name>.md`** | Isolated context subagents that return a summary | Long exploration tasks that would bloat the main conversation |
| **Hooks** (`settings.json`) | Deterministic pre/post tool intercepts | Quality gates that should never be bypassed (clippy, fmt, type-check) |
| **`docs/`** | Human-readable engineering essays | Decisions that need to outlive a single session — the *why*, not the *how* |

> **Rule of thumb:** if the agent failed this way once, fix the agent (skill or prompt). If it fails this way three times in a week, fix the harness (hook or subagent).

## What we keep out of `CLAUDE.md`

Current best practice ([Anthropic docs](https://code.claude.com/docs/en/best-practices), HumanLayer): keep `CLAUDE.md` under ~50–60 lines. Every line in `CLAUDE.md` is a recurring token cost on every turn. Our `CLAUDE.md` is intentionally a **pointer file** with:

- The locked-in decisions table (with links into `docs/`)
- Workspace layout
- Common commands
- Conventions

Everything else — architecture, agent design, Circle stack details, design system, open questions — lives in `docs/` and loads only when relevant.

## Project skills we should create

A skill earns its place when a procedure has been repeated 3+ times. Candidates for the Aegis build:

| Skill | Trigger | What it does |
|---|---|---|
| `/migrate` | New SQL migration needed | Stages an `apps/api/migrations/NNNN_*.sql`, runs `cargo sqlx migrate run`, regenerates sqlx offline metadata, reminds about rollback. |
| `/new-prompt` | New AI prompt template | Scaffolds `apps/api/prompts/<name>.md` with frontmatter listing required placeholders, adds the variant to `PromptKey`, wires through `PromptRegistry`. |
| `/wire-sse` | New SSE event type | Adds the variant to `SseEvent` in shared types, broadcaster call site in API, handler in `useEventSource` hook, demo path through the UI. |
| `/ship` | Ready to commit | Runs `cargo fmt --check && cargo clippy -- -D warnings && cargo test` then `pnpm lint && pnpm type-check`, drafts a Conventional Commit message, stages specific files only, never `git add -A`. |
| `/circle-call` | Stubbing a Circle SDK call | Pulls the current Circle docs page for that endpoint, scaffolds a typed wrapper in the right module, adds an env var to `Config` if needed. |

`SKILL.md` body stays concise — best practice is "state what to do, not why" — and any procedure long enough to need supporting files moves them next to the skill. Project skills are checked into `.claude/skills/` so they apply for every contributor.

## Subagents we should create

The bundled `Explore` and `Plan` agents cover most needs. For Aegis-specific recurring work, two custom subagents are worth defining:

| Subagent | Use case |
|---|---|
| `rust-axum-module` | Add a new `apps/api/src/modules/<name>/` module: pattern-match existing modules, scaffold `mod.rs`, `models.rs`, `service.rs`, `handlers.rs`, wire into `router.rs`. |
| `neo-brutalist-sweep` | Convert a `components/` subtree from shadcn defaults to neo-brutalism tokens, verifying the two-accent rule from `docs/04-design-system.md`. |

Both have narrow scope — "specificity buys you better tool selection and tighter context" ([HumanLayer](https://www.humanlayer.dev/blog/skill-issue-harness-engineering-for-coding-agents)). General-purpose subagents are weaker than focused ones.

## Hooks we should install

Two deterministic guardrails worth wiring into `.claude/settings.json`:

1. **Pre-commit** — block `git commit` unless `cargo fmt --check && cargo clippy -- -D warnings` and `pnpm lint && pnpm type-check` both pass. Catches the "I forgot to format" loop.
2. **Post-`Edit` on `apps/api/prompts/*.md`** — emit a reminder to bump a prompt version comment, so prompt churn shows up in `git log -- apps/api/prompts/`.

Hooks enforce architecturally. A prompt-based reminder ("remember to run clippy") can be reasoned around; a hook cannot.

## The build process (Sprint 1)

The current build plan is tracked as tasks (use `TaskList` to see them). Sprint 1 is 11 ordered tasks targeting the **agent foundation** — see `/Users/malivix/.claude/plans/check-this-website-and-moonlit-matsumoto.md` for the full plan with file paths and verification steps.

Sprint output: every agent decision produced by OpenRouter with regime classification, an adversarial critic pass, model + token telemetry, pushed to the UI over SSE. No Circle integration yet (Sprint 2), no on-chain execution yet (Sprint 3).

## What this enables / doesn't

> **Enables:** repeatable workflows that don't burn context on every invocation, deterministic quality gates that a tired engineer can't bypass, and a `CLAUDE.md` that fits in one screen.
>
> **Doesn't:** replace good prompts or good code review. The harness reduces the cost of mistakes; it doesn't make the agent smarter.
