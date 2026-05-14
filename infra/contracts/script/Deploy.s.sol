// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import { Script, console2 } from "forge-std/Script.sol";

import { RebalanceExecutor } from "../src/RebalanceExecutor.sol";
import { IUniswapV3SwapRouter } from "../src/interfaces/IUniswapV3SwapRouter.sol";

/// @notice Deploy RebalanceExecutor on a CCTP V2 destination chain.
/// @dev Reads `MESSAGE_TRANSMITTER`, `USDC`, `SWAP_ROUTER` from env.
contract DeployRebalanceExecutor is Script {
    function run() external returns (RebalanceExecutor exec) {
        address mt = vm.envAddress("MESSAGE_TRANSMITTER");
        address usdc = vm.envAddress("USDC");
        address router = vm.envAddress("SWAP_ROUTER");

        require(mt != address(0), "MESSAGE_TRANSMITTER missing");
        require(usdc != address(0), "USDC missing");
        require(router != address(0), "SWAP_ROUTER missing");

        vm.startBroadcast();
        exec = new RebalanceExecutor(mt, usdc, IUniswapV3SwapRouter(router));
        vm.stopBroadcast();

        console2.log("RebalanceExecutor:", address(exec));
    }
}
