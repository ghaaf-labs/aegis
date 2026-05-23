# 03 — Circle stack

> **Every Circle product earns its place by removing a piece of friction the user would otherwise have to learn.** Wallets remove keys. Gateway removes per-chain accounting. CCTP removes bridges. Paymaster removes native gas. USYC removes "where do I park stablecoins?" StableFX removes FX rails.

## Product → file map

| Circle product                                | Used for                                               | Module                                          |
| --------------------------------------------- | ------------------------------------------------------ | ----------------------------------------------- |
| **Circle Wallets** (developer-controlled SCA) | One wallet route per supported testnet, no seed phrase | `apps/api/src/modules/wallet/`                  |
| **Gateway**                                   | Single USDC balance across supported wallet routes     | `apps/api/src/modules/gateway/`                 |
| **CCTP V2**                                   | Arc↔Base rebalance execution (Fast Transfer + Hooks)   | `apps/api/src/modules/rebalance/cross_chain.rs` |
| **USYC**                                      | Risk-off allocation; tokenized US T-bills              | `apps/api/src/modules/yield/`                   |
| **Paymaster**                                 | USDC-denominated gas on execution rails                | `apps/api/src/modules/paymaster/`               |
| **StableFX** (Arc-native)                     | USDC↔EURC for the EUR sleeve                           | `apps/api/src/modules/fx/`                      |
| **Nanopayments**                              | Protocol fee per executed rebalance + referral payouts | `apps/api/src/modules/billing/`                 |

Chain config and contract addresses live in `packages/shared/src/constants.ts`.

## Wallets

A new user gets Circle developer-controlled SCA wallet routes on `ARC-TESTNET`, `BASE-SEPOLIA`, `ETH-SEPOLIA`, `ARB-SEPOLIA`, and `AVAX-FUJI`. The source of truth is `user_wallet_networks`; legacy `users.arc_address` / `users.base_address` are projections for older UI surfaces. There is no seed phrase or browser SDK ceremony. The user authenticates with email code, while Aegis signs server-side through Circle's entity-secret flow.

## Gateway — the unified balance

Gateway is the most useful piece of plumbing in the stack. Instead of "you have 200 USDC on Arc, 50 on Base, and dust elsewhere," the user sees one cash number plus a per-route detail view. When the agent proposes a rebalance that requires liquidity on Base, the current executor mints through the Arc/Base rails — no manual bridging, no manual approval per chain.

The UI follows: every USDC number is the Gateway balance. The per-chain breakdown is one click away in a "details" pane, never the default view.

## CCTP V2 — cross-chain rebalance

When a proposal moves capital across the currently deployed execution rails, the executor builds a CCTP V2 Fast Transfer with a **Hook** that calls `RebalanceExecutor.swap(...)` on the destination chain in the same atomic operation. Today that execution rail is Arc testnet ↔ Base Sepolia; Ethereum Sepolia, Arbitrum Sepolia, and Avalanche Fuji are wallet/balance-ready until their executors and env addresses are deployed.

Fast Transfer keeps the cross-chain leg under ~15 seconds in practice; Hooks remove the "now I have to remember to swap on the other side" step.

## USYC — the risk-off sleeve

USYC is a synthetic asset in the agent's allocation universe. When the regime classifier flags `RiskOff`, the agent typically proposes shifting 30–60% of the portfolio into USYC and parks it there. When regime returns to `Neutral` or `RiskOn`, it redeems back to USDC and re-deploys.

The atomic USDC↔USYC API means there is no settlement window the user needs to think about; the UI just shows USYC as the yield-bearing component of the dollar sleeve.

## Paymaster — USDC gas

Configured for the deployed execution rails first. Every transaction the user authorizes pays gas in **USDC** from the same Gateway balance. The approval modal shows an indicative fee preview in USDC, no native token math required.

## StableFX — the EURC sleeve

Portfolios can hold a EUR sleeve. The agent treats EURC as a first-class asset with its own regime/correlation signal (`usdc_eurc_basis`) and rebalances USDC↔EURC through Arc's native StableFX engine. This is the cleanest "currency diversification" demo on the stack — no off-chain FX provider, no settlement risk.

## Nanopayments — agent-economy fees

Aegis charges $0.10 USDC per executed rebalance, settled through Nanopayments. The same rail powers referral payouts ($0.50 per referred user who completes a rebalance) and, if the strategy marketplace ships, royalty payments to strategy authors.

## Why this matters for judging

Circle Tool Usage is 20% of the score. By the time the demo plays through one rebalance, the user has touched **Wallets · Gateway · CCTP · Paymaster** in a single approval, plus **USYC** if regime flipped and **StableFX** if EURC is in the portfolio. **Nanopayments** sits behind the protocol-fee deduction. That's the entire stack in one user action.

---

> **What this enables:** five wallet/balance routes for the account, and a single approval that moves USDC across the deployed Arc/Base execution rails, swaps, and pays its own gas — without the user knowing what a chain is.
>
> **What it doesn't:** execute rebalances on Ethereum Sepolia, Arbitrum Sepolia, or Avalanche Fuji until the matching RebalanceExecutor contracts, RPC/signing config, token addresses, and adapter tests are live.
