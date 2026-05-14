# 04 — Design system

> **Dark neo-brutalism for a serious trading desk.** Two semantic accents (green = money, cyan = agent) with a strict separation rule. Hard shadows, no gradients, monospace numbers, decisive layouts.

## Tokens

```
--bg          #0A0A0A   /* near-black canvas */
--surface     #141414   /* default card */
--raised      #1C1C1C   /* hover / focus card */
--border      #2A2A2A   /* default 2px solid border */
--border-hi   #FFFFFF   /* focus / hover border */

--accent-pnl     #00FF88   /* electric green — money / PnL / approve */
--accent-agent   #00E0FF   /* cyan — agent activity / model / regime */
--risk           #FF2D7A   /* magenta — loss / risk */
--warn           #FFB800   /* amber — warning */

--text-hi   #FFFFFF
--text      #E5E5E5
--text-lo   #8A8A8A
--text-mut  #5A5A5A
```

Shadows are always hard, no blur:

```
--shadow-sm  2px 2px 0 0 #000
--shadow     4px 4px 0 0 #000
--shadow-lg  6px 6px 0 0 #000
```

Border radius: `2px` for buttons and pills, `4px` for cards. **Never** `rounded-full`.

## Typography

- **`Inter Tight`** — UI text, headings, body. Loaded via `next/font`.
- **`JetBrains Mono`** — every number, every address, every regime pill, every model slug. `font-variant-numeric: tabular-nums` set globally on `.mono`.

Scale: `12 · 14 · 16 · 20 · 28 · 40` px. Tracking `-0.01em` on display sizes; default tracking otherwise.

## The two-accent rule

This is the rule that keeps the dual-accent system from looking like a casino:

- **Green never appears in agent surfaces.** Model slug, regime pill, decision card chrome, critic verdict — all cyan. PnL number on the same card — green.
- **Cyan never appears in PnL numbers.** A change percentage is always green/red, never cyan even when the change came from an agent action.
- Risk uses magenta, not red — red is reserved for destructive system actions (delete, force-close).
- Amber for "you should look at this but it's not broken."

When in doubt: ask "is this number money or signal?" Money → green/magenta. Signal → cyan.

## Components (in `packages/ui/`)

| Primitive | Notes |
|---|---|
| `Card` | `2px` border, `--shadow` on hover, no gradient backgrounds |
| `Button` (primary) | Solid `--accent-pnl` for approvals; solid `--text-hi` for neutrals; `--shadow-sm` |
| `Button` (agent) | Solid `--accent-agent` for "ask agent" / "rerun analysis" |
| `Pill` (regime) | `RISK-ON` solid green, `NEUTRAL` solid white, `RISK-OFF` solid magenta — text always black, monospace |
| `DataTable` | Dense rows (28px), monospace numerics, right-aligned numbers |
| `ModelBadge` | `[opus-4-7]` style chip, cyan border, monospace |
| `ChainBadge` | `ARC` / `BASE` / `AVAX` chip with chain accent stripe |
| `FeePreview` | Always shows `~$0.0123 USDC` with `via Paymaster` provenance |
| `ProvenanceLine` | `via CoinGecko · 2.1s ago` muted text under any fetched value |

## Trust signals (mandatory on every screen)

1. **Data provenance** — every external value names its source and freshness.
2. **Chain badges** — every cross-chain action names both chains.
3. **USDC fee preview** — every approvable action shows the USDC fee before the approve button.
4. **Model slug** — every agent-written sentence shows the model that wrote it.
5. **Confidence bar** — every agent decision shows confidence as a thin cyan bar (1–100).

## Do / Don't

| Do | Don't |
|---|---|
| Hard 4px offset shadows | Soft blurred shadows |
| Solid colored regime pills with black text | Outline pills with colored text |
| Tabular monospace numbers | Proportional digits in numerics |
| Two-accent semantic separation | Mixing green and cyan in one component |
| Border radius 2–4px | `rounded-full`, `rounded-2xl` |
| Black canvas, white focus borders | Slate gray "dark mode" |
| One emphasis per card | Multiple competing accents |

## Responsive

Two breakpoints: `390px` (mobile) and `1440px` (desktop). The dashboard is information-dense; mobile collapses tables to stacked cards but never hides numbers.

---

> **What this enables:** a feeling of seriousness — the look of Bloomberg/Linear/Polymarket carried by neo-brutalism's confidence — without being decorative.
>
> **What it doesn't:** light mode (out of scope for the hackathon), or theme customization (one opinionated palette).
