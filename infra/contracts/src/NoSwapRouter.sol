// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import { IUniswapV3SwapRouter } from "./interfaces/IUniswapV3SwapRouter.sol";

/// @title NoSwapRouter — sentinel swap router for chains without an AMM.
///
/// Deployed on Arc testnet (and any future chain where Circle StableFX
/// hasn't shipped a Uniswap V3-compatible adapter yet). Satisfies the
/// `IUniswapV3SwapRouter` interface so `RebalanceExecutor`'s non-zero
/// constructor check passes, but reverts any actual swap with
/// `AmmNotAvailable()`.
///
/// Aegis's stablecoin-only rebalances always have `tokenOut == USDC`,
/// so the executor never reaches this contract's `exactInputSingle`.
/// If a misconfigured hook payload does invoke it, the revert is a
/// loud, descriptive signal — the CCTP V2 message stays unconsumed
/// on the destination chain and the user can re-attempt with a
/// corrected hook.
///
/// Self-documenting on-chain: an explorer reader sees `NoSwapRouter`
/// as the swap router and knows immediately that swaps are not wired
/// on this chain. Compare to passing a placeholder address from
/// another chain (which is the alternative we deliberately rejected).
contract NoSwapRouter is IUniswapV3SwapRouter {
    /// @notice Emitted before `AmmNotAvailable` so on-chain forensics
    /// can see what hook payload triggered the revert. The amount and
    /// recipient travel through the contract memory and are observable
    /// in the trace; storing them in an event makes them indexable.
    event AttemptedSwap(
        address indexed tokenIn,
        address indexed tokenOut,
        address indexed recipient,
        uint256 amountIn,
        uint256 amountOutMinimum,
        uint24 fee
    );

    /// @notice Thrown by every swap call. Always.
    error AmmNotAvailable();

    /// @inheritdoc IUniswapV3SwapRouter
    function exactInputSingle(ExactInputSingleParams calldata params)
        external
        payable
        override
        returns (uint256)
    {
        emit AttemptedSwap(
            params.tokenIn,
            params.tokenOut,
            params.recipient,
            params.amountIn,
            params.amountOutMinimum,
            params.fee
        );
        revert AmmNotAvailable();
    }
}
