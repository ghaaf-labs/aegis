// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import { IERC20 } from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import { SafeERC20 } from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import { Ownable } from "@openzeppelin/contracts/access/Ownable.sol";

import { IMessageHandlerV2 } from "./interfaces/ICCTPV2.sol";
import { IUniswapV3SwapRouter } from "./interfaces/IUniswapV3SwapRouter.sol";

/// @title Aegis cross-chain rebalance executor
/// @notice Destination-chain hook target for Circle CCTP V2. After the
///         transmitter mints fresh USDC into this contract, it calls
///         `handleReceiveMessage` with the user's swap intent encoded as the
///         hook payload. We swap USDC into the target asset via Uniswap V3 and
///         forward the output to the user's wallet.
/// @dev   Only the canonical CCTP V2 MessageTransmitter on the deploy chain
///        may invoke `handleReceiveMessage`. The owner can rotate the swap
///        router if Uniswap migrates, but cannot drain user funds.
contract RebalanceExecutor is IMessageHandlerV2, Ownable {
    using SafeERC20 for IERC20;

    /// @notice Trusted CCTP V2 MessageTransmitter on this chain.
    address public immutable messageTransmitter;
    /// @notice USDC token on this chain.
    address public immutable usdc;
    /// @notice Uniswap V3 swap router. Mutable to allow upgrade.
    IUniswapV3SwapRouter public swapRouter;

    event SwapRouterUpdated(address indexed previous, address indexed next);

    event HookExecuted(
        uint32 indexed sourceDomain,
        address indexed recipient,
        address tokenOut,
        uint256 amountIn,
        uint256 amountOut
    );

    error OnlyMessageTransmitter();
    error InvalidHookPayload();
    error ZeroAddress();

    constructor(address _messageTransmitter, address _usdc, IUniswapV3SwapRouter _swapRouter)
        Ownable(msg.sender)
    {
        if (
            _messageTransmitter == address(0) || _usdc == address(0)
                || address(_swapRouter) == address(0)
        ) {
            revert ZeroAddress();
        }
        messageTransmitter = _messageTransmitter;
        usdc = _usdc;
        swapRouter = _swapRouter;
    }

    function setSwapRouter(IUniswapV3SwapRouter next) external onlyOwner {
        if (address(next) == address(0)) revert ZeroAddress();
        emit SwapRouterUpdated(address(swapRouter), address(next));
        swapRouter = next;
    }

    /// @inheritdoc IMessageHandlerV2
    function handleReceiveMessage(uint32 sourceDomain, bytes32, bytes calldata messageBody)
        external
        override
        returns (bool)
    {
        if (msg.sender != messageTransmitter) revert OnlyMessageTransmitter();
        if (messageBody.length != 32 * 5) revert InvalidHookPayload();

        (address recipient, address tokenOut, uint24 poolFee, uint256 minOut, uint256 deadline) =
            abi.decode(messageBody, (address, address, uint24, uint256, uint256));

        if (recipient == address(0) || tokenOut == address(0)) revert InvalidHookPayload();

        uint256 amountIn = IERC20(usdc).balanceOf(address(this));

        // If the user asked for native USDC at the destination (no swap),
        // forward it directly. Saves the Uniswap fee + slippage roundtrip.
        if (tokenOut == usdc) {
            IERC20(usdc).safeTransfer(recipient, amountIn);
            emit HookExecuted(sourceDomain, recipient, tokenOut, amountIn, amountIn);
            return true;
        }

        IERC20(usdc).forceApprove(address(swapRouter), amountIn);

        IUniswapV3SwapRouter.ExactInputSingleParams memory params =
            IUniswapV3SwapRouter.ExactInputSingleParams({
                tokenIn: usdc,
                tokenOut: tokenOut,
                fee: poolFee,
                recipient: recipient,
                deadline: deadline,
                amountIn: amountIn,
                amountOutMinimum: minOut,
                sqrtPriceLimitX96: 0
            });

        uint256 amountOut = swapRouter.exactInputSingle(params);

        emit HookExecuted(sourceDomain, recipient, tokenOut, amountIn, amountOut);
        return true;
    }
}
