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

The contract reverts with `InvalidHookPayload` if the payload is not exactly 160 bytes or if `recipient` / `tokenOut` is the zero address.
