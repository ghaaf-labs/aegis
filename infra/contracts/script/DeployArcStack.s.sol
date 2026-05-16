// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import { Script, console2 } from "forge-std/Script.sol";

import { NoSwapRouter } from "../src/NoSwapRouter.sol";
import { RebalanceExecutor } from "../src/RebalanceExecutor.sol";
import { IUniswapV3SwapRouter } from "../src/interfaces/IUniswapV3SwapRouter.sol";

/// @notice Deploy `NoSwapRouter` then `RebalanceExecutor` on a chain
/// without a Uniswap V3 deployment (Arc testnet).
///
/// Reads `MESSAGE_TRANSMITTER` and `USDC` from env; the swap router is
/// the freshly-deployed `NoSwapRouter` so swap calls always revert
/// loudly. Aegis's stablecoin-only rebalances have `tokenOut == USDC`
/// and never reach the swap path, so this is benign in production.
contract DeployArcStack is Script {
    function run() external returns (NoSwapRouter stub, RebalanceExecutor exec) {
        address mt = vm.envAddress("MESSAGE_TRANSMITTER");
        address usdc = vm.envAddress("USDC");

        require(mt != address(0), "MESSAGE_TRANSMITTER missing");
        require(usdc != address(0), "USDC missing");

        vm.startBroadcast();
        stub = new NoSwapRouter();
        exec = new RebalanceExecutor(mt, usdc, IUniswapV3SwapRouter(address(stub)));
        vm.stopBroadcast();

        console2.log("NoSwapRouter:     ", address(stub));
        console2.log("RebalanceExecutor:", address(exec));
    }
}
