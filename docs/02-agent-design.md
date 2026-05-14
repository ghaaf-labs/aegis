# 02 — Agent design

> **Different brains for different jobs, and a strict rule about what gets into the prompt.** A regime classifier runs before every rebalance. A critic runs after. The strategist sees a map of the portfolio, not a manual.

## Multi-model routing

We use **OpenRouter** as the single AI gateway. Each named task resolves to a model:

| Route | Default model | Why this model |
|---|---|---|
| `RegimeClassify` | `anthropic/claude-haiku-4-5` | Cheap, fast, reliable JSON output for a 1-of-3 label |
| `RebalanceReason` | `anthropic/claude-opus-4-7` | Highest reasoning quality on the user-facing decision |
| `TaxExplain` | `anthropic/claude-sonnet-4-6` | Good prose, lower cost than Opus, plenty for explanations |
| `MarketCommentary` | `google/gemini-2.5-flash` | Cheap daily-digest writer, long context for many assets |
| `CritiqueAgent` | `openai/gpt-5` | Different family from strategist → genuine adversarial diversity |

Every persisted decision records `model_slug`, `prompt_tokens`, `completion_tokens`, and `latency_ms`. The slug is rendered next to the decision in the UI.

## The strategist prompt

A strict system prompt with five sections, in this order:

1. **Role and constraints** — what the agent can propose and what it cannot (no leverage, no shorting, USDC-denominated).
2. **Goal** — the user's horizon, risk tolerance, target allocation, and any constraints from the goal wizard.
3. **Regime + signals** — output of the classifier, plus the structured risk report (concentration, vol, drift).
4. **Portfolio snapshot** — current allocations, prices with provenance, harvestable losses, USYC rate, EURC basis.
5. **Memory** — last 5 decisions for this portfolio, each compressed to `(date, action, outcome_24h)`.

The **user message** is just `"Propose a rebalance, or recommend hold."`. Everything else is in the system prompt.

## The "map, not a manual" rule

Lifted from OpenAI's harness-engineering writing. We picked it after the first prototype's prompt grew to 8k tokens of project lore and the model got worse, not better.

- **Map**: a small, structured reference of what exists and how to think about it.
- **Manual**: a long prose document trying to teach the model everything at once.

Concretely, this means:

- Memory retrieval is capped at 5 entries × ~120 tokens each.
- Asset-list context is the user's portfolio + the watchlist, not "all of CoinGecko."
- News/onchain metrics are pulled **on demand** via tool calls, not preloaded.
- Documentation about Aegis itself stays out of the prompt; the model doesn't need to know our architecture to choose allocations.

Total target: **system prompt under 3k tokens** at p95.

## Tool calls

The strategist has a small toolbox it can invoke during reasoning:

| Tool | When the model uses it |
|---|---|
| `fetch_news(symbol, hours)` | When proposing to *increase* an allocation; sanity check |
| `fetch_onchain_metric(asset, name)` | When considering large moves into less-liquid assets |
| `fetch_correlation(symbols)` | When proposing diversification moves |
| `simulate_trade(from, to, amount)` | Returns slippage and Hook fees before committing to a proposal |

Tool results are appended to the conversation. Latency budget per decision: **8 seconds p95**, **20 seconds p99** (Opus + 1 critic pass + up to 2 tool calls).

## The critic

Same context as the strategist, plus the proposal as a user message and a different system prompt:

> *You are a risk-averse counterparty. The strategist has proposed a rebalance. Find the strongest case against it: what regime change would invalidate it, what correlation it ignores, what tax cost it overlooks. If it survives your critique, say so. Be specific.*

The strategist gets one revision attempt with the critic's notes appended. Both versions are persisted; the UI shows the critic verdict.

## Confidence and abstain

Every proposal includes `confidence: 0..1`. Below a configurable threshold (default `0.5`), the agent emits an `AbstainDecision` instead of a rebalance. Abstaining is shown to the user with the reason — and counts as a decision in the diary, not a non-event.

---

> **What this enables:** explainable decisions, drift toward the right model for each task, and a prompt that stays cheap as the codebase grows.
>
> **What it doesn't:** memory across users (each portfolio's memory is private), or guarantees about model availability — OpenRouter's failover handles outages but model behavior can shift between routings.
