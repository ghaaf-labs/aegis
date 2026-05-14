# 03 — Circle stack

> **Every Circle product earns its place by removing a piece of friction the user would otherwise have to learn.** Wallets remove keys. Gateway removes per-chain accounting. CCTP removes bridges. Paymaster removes native gas. USYC removes "where do I park stablecoins?" StableFX removes FX rails.

## Product → file map

| Circle product | Used for | Module |
|---|---|---|
| **Circle Wallets** (modular MSCA) | One wallet per user, no seed phrase | `apps/api/src/modules/wallet/` |
| **Gateway** | Single USDC balance across Arc + Base | `apps/api/src/modules/gateway/` |
| **CCTP V2** | Cross-chain rebalances (Fast Transfer + Hooks) | `apps/api/src/modules/rebalance/cross_chain.rs` |
| **USYC** | Risk-off allocation; tokenized US T-bills | `apps/api/src/modules/yield/` |
| **Paymaster** | USDC-denominated gas on Arc + Base | configured per chain in `apps/api/src/modules/wallet/` |
| **StableFX** (Arc-native) | USDC↔EURC for the EUR sleeve | `apps/api/src/modules/fx/` |
| **Nanopayments** | Protocol fee per executed rebalance + referral payouts | `apps/api/src/modules/billing/` |

Chain config and contract addresses live in `packages/shared/src/constants.ts`.

## Wallets

A new user gets a Circle modular smart-contract account (MSCA) on Arc and Base in the same call. We persist `wallet_id`, `arc_address`, `base_address` on the `users` row. There is no seed phrase. The user authenticates with email + WebAuthn passkey; Circle's WaaS handles key custody and recovery.

## Gateway — the unified balance

Gateway is the most useful piece of plumbing in the stack. Instead of "you have 200 USDC on Arc and 50 on Base," the user sees **250 USDC**. When the agent proposes a rebalance that requires liquidity on Base, the executor mints from the unified balance directly on Base — no manual bridging, no manual approval per chain.

The UI follows: every USDC number is the Gateway balance. The per-chain breakdown is one click away in a "details" pane, never the default view.

## CCTP V2 — cross-chain rebalance

When a proposal moves capital across chains, the executor builds a CCTP V2 Fast Transfer with a **Hook** that calls `RebalanceExecutor.swap(...)` on the destination chain in the same atomic operation. The user sees one approval, one tx hash on the source, and one Hook execution on the destination — both surfaced in the rebalance event.

Fast Transfer keeps the cross-chain leg under ~15 seconds in practice; Hooks remove the "now I have to remember to swap on the other side" step.

## USYC — the risk-off sleeve

USYC is a synthetic asset in the agent's allocation universe. When the regime classifier flags `RiskOff`, the agent typically proposes shifting 30–60% of the portfolio into USYC and parks it there. When regime returns to `Neutral` or `RiskOn`, it redeems back to USDC and re-deploys.

The atomic USDC↔USYC API means there is no settlement window the user needs to think about; the UI just shows USYC as the yield-bearing component of the dollar sleeve.

## Paymaster — USDC gas

Configured on Arc (USDC-native) and Base (ERC-4337 paymaster). Every transaction the user authorizes pays gas in **USDC** from the same Gateway balance. The approval modal shows a real-time fee preview in USDC, no native token math required.

## StableFX — the EURC sleeve

Portfolios can hold a EUR sleeve. The agent treats EURC as a first-class asset with its own regime/correlation signal (`usdc_eurc_basis`) and rebalances USDC↔EURC through Arc's native StableFX engine. This is the cleanest "currency diversification" demo on the stack — no off-chain FX provider, no settlement risk.

## Nanopayments — agent-economy fees

Aegis charges $0.10 USDC per executed rebalance, settled through Nanopayments. The same rail powers referral payouts ($0.50 per referred user who completes a rebalance) and, if the strategy marketplace ships, royalty payments to strategy authors.

## Why this matters for judging

Circle Tool Usage is 20% of the score. By the time the demo plays through one rebalance, the user has touched **Wallets · Gateway · CCTP · Paymaster** in a single approval, plus **USYC** if regime flipped and **StableFX** if EURC is in the portfolio. **Nanopayments** sits behind the protocol-fee deduction. That's the entire stack in one user action.

---

> **What this enables:** a single approval that moves USDC across two chains, swaps, and pays its own gas — without the user knowing what a chain is.
>
> **What it doesn't:** Arc mainnet (testnet only during the event), and any product Circle hasn't launched yet (e.g., CCTP V2 isn't on every chain we'd like).
