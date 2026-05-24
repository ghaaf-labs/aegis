# Aegis CCTP V2 Hook ABI

`RebalanceExecutor.handleReceiveMessage(uint32 sourceDomain, bytes32 sender, bytes calldata messageBody)`

`messageBody` is a 160-byte payload built by `apps/api/src/modules/rebalance/cross_chain.rs::build_hook_payload`:

| Field       | Solidity type | Notes                                                                                                                              |
| ----------- | ------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `recipient` | `address`     | Final destination wallet (the user's MSCA on the destination chain)                                                                |
| `tokenOut`  | `address`     | ERC-20 to swap into. If equal to the chain's USDC address the contract skips the Uniswap leg and forwards the minted USDC directly |
| `poolFee`   | `uint24`      | Uniswap V3 pool fee tier (`500`, `3000`, `10000`)                                                                                  |
| `minOut`    | `uint256`     | Minimum `tokenOut` the swap must produce. Computed by the planner with 50bps slippage from the Uniswap V3 quoter                   |
| `deadline`  | `uint256`     | Unix seconds. Planner sets `now + 600`. CCTP-V2 attestation usually lands within 30s; this gives 9.5 minutes of headroom           |

The contract reverts with `InvalidHookPayload` if the payload is not exactly 160 bytes or if `recipient` / `tokenOut` is the zero address (the one revert it keeps — there is no safe refund destination for a zero recipient).

## Safety semantics (post-mint, funds never trapped)

`handleReceiveMessage` is `nonReentrant` and treats the minted USDC as the user's at all times:

| Outcome                                           | Behavior                                                  | Event                                            |
| ------------------------------------------------- | --------------------------------------------------------- | ------------------------------------------------ |
| `tokenOut == usdc`                                | Forward USDC directly (fast path, no swap)                | `HookExecuted`                                   |
| `tokenOut` not on owner allowlist                 | Refund full USDC to `recipient`                           | `HookRefunded(_, _, "tokenOut not allowlisted")` |
| active `swapRouter` not on owner allowlist        | Refund full USDC to `recipient`                           | `HookRefunded(_, _, "router not allowlisted")`   |
| swap reverts / `minOut` miss / deadline / reentry | Drop the router approval, refund full USDC to `recipient` | `HookRefunded(_, _, "swap failed")`              |
| swap succeeds                                     | `recipient` receives `tokenOut`                           | `HookSwapSettled` (+ `HookExecuted`)             |

Owner-only controls: `setAllowedTokenOut(token, allowed)`, `setAllowedRouter(router, allowed)`, `setSwapRouter(next)` (implicitly allowlists `next`), and `rescue(token, to, amount)` (escape hatch for residual dust; under normal flow the contract holds no balance between messages because every path settles or refunds atomically).

The deploy chain's USDC is always implicitly allowed (it is the passthrough asset). The constructor allowlists the deploy-time router. A non-allowlisted `tokenOut` is refunded **without** touching the router, so a misconfigured payload cannot reach a swap venue.
