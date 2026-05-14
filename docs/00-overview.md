# 00 — Overview

> **Aegis is an adaptive portfolio harness for stablecoin-native finance.** The user steers (sets a goal, approves moves); a multi-model AI agent executes on Arc and Base through Circle's stack.

## What it is

A goal-based crypto portfolio manager that reads market regime, proposes rebalances, and settles them across chains in USDC. Submitted to **RFB 04: Adaptive Portfolio Manager** at Canteen × Circle's Agora Agents Hackathon (May 11–25, 2026).

## The constraint

Humans steer. Agents execute. The agent never moves money on its own — every rebalance is a proposal that lands in a one-screen approval flow. The agent's autonomy lives in *what to consider, when to act, and how confident to be* — not in custody.

## The rails

| Layer | Choice | Why |
|---|---|---|
| Settlement | **Arc** (primary) + **Base** (cross-chain) | USDC-native gas + sub-second finality; Base for CCTP V2 Fast Transfer + Hooks |
| Wallets | **Circle Wallets** (modular MSCA) | One API, paymaster-aware, no seed phrases |
| Cross-chain | **Gateway** + **CCTP V2** | Unified USDC balance; atomic burn-mint with destination-chain Hooks |
| Yield | **USYC** | Tokenized US T-bills as the risk-off sleeve |
| FX | **Arc StableFX** | Native USDC↔EURC for multi-currency portfolios |
| Fees | **Circle Paymaster** + **Nanopayments** | Users pay in USDC; protocol fees are sub-cent |
| AI | **OpenRouter** with per-task model routing | Right brain for each job; not locked to one provider |
| Realtime | **SSE** (`/sse`) | Server→client only; native `EventSource`; trivial proxying |

## What we are *not* building

- A custodian. Circle Wallets hold keys; we hold preferences.
- A trading desk. There is no leverage, no perps, no shorting.
- A black box. Every decision shows the model that produced it, the regime it saw, and the prompt context size.

## Reading order

1. [01 — Architecture](./01-architecture.md)
2. [02 — Agent design](./02-agent-design.md)
3. [03 — Circle stack](./03-circle-stack.md)
4. [04 — Design system](./04-design-system.md)
5. [05 — Open questions](./05-open-questions.md)

---

> **What this enables:** a portfolio that reacts to regime changes in seconds, holds yield-bearing dollars when markets turn, and executes across chains without the user thinking about chains.
>
> **What it doesn't:** discretionary trading, leverage, or any action without a human in the loop.
