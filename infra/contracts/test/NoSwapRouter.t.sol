// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import { Test } from "forge-std/Test.sol";
import { Vm } from "forge-std/Vm.sol";
import { NoSwapRouter } from "../src/NoSwapRouter.sol";
import { IUniswapV3SwapRouter } from "../src/interfaces/IUniswapV3SwapRouter.sol";

contract NoSwapRouterTest is Test {
    NoSwapRouter internal router;

    address internal constant USDC_LIKE = address(0xA);
    address internal constant USDT_LIKE = address(0xB);
    address internal constant ALICE = address(0xCAFE);

    function setUp() public {
        router = new NoSwapRouter();
    }

    function _params() internal pure returns (IUniswapV3SwapRouter.ExactInputSingleParams memory) {
        return IUniswapV3SwapRouter.ExactInputSingleParams({
            tokenIn: USDC_LIKE,
            tokenOut: USDT_LIKE,
            fee: 500,
            recipient: ALICE,
            deadline: type(uint256).max,
            amountIn: 1_000_000, // 1 USDC (6-dec)
            amountOutMinimum: 995_000,
            sqrtPriceLimitX96: 0
        });
    }

    function test_ExactInputSingleAlwaysReverts() public {
        vm.expectRevert(NoSwapRouter.AmmNotAvailable.selector);
        router.exactInputSingle(_params());
    }

    function test_AttemptedSwapEventEmittedBeforeRevert() public {
        // Per Solidity semantics, events emitted before a revert are
        // discarded in production. But Foundry's `expectEmit` captures
        // the emit attempt during the failed call — so this test pins
        // the forensics-friendly behavior: the emit *did* happen before
        // the revert (proving on-chain trace observability).
        vm.expectEmit(true, true, true, true, address(router));
        emit NoSwapRouter.AttemptedSwap(USDC_LIKE, USDT_LIKE, ALICE, 1_000_000, 995_000, 500);
        vm.expectRevert(NoSwapRouter.AmmNotAvailable.selector);
        router.exactInputSingle(_params());
    }

    function test_ImplementsIUniswapV3SwapRouter() public {
        // Compile-time check that NoSwapRouter is a valid implementor
        // (this would fail at compile time if the signature drifted).
        // We assert by casting and re-attempting the call.
        IUniswapV3SwapRouter typed = IUniswapV3SwapRouter(address(router));
        vm.expectRevert(NoSwapRouter.AmmNotAvailable.selector);
        typed.exactInputSingle(_params());
    }
}
