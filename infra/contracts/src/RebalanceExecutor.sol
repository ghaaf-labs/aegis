// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import { IERC20 } from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import { SafeERC20 } from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import { Ownable } from "@openzeppelin/contracts/access/Ownable.sol";
import { ReentrancyGuard } from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

import { IMessageHandlerV2 } from "./interfaces/ICCTPV2.sol";
import { IUniswapV3SwapRouter } from "./interfaces/IUniswapV3SwapRouter.sol";

/// @title Aegis cross-chain rebalance executor
/// @notice Destination-chain hook target for Circle CCTP V2. After the
///         transmitter mints fresh USDC into this contract, it calls
///         `handleReceiveMessage` with the user's swap intent encoded as the
///         hook payload. We swap USDC into the target asset via Uniswap V3 and
///         forward the output to the user's wallet.
///
/// @dev Integration pattern: this is a CCTP V2 *MessageHandler* (the
///      hook-recipient pattern), NOT a self-relaying `CCTPReceiverV2`. The
///      caller (Aegis backend or any relayer) submits
///      `MessageTransmitter.receiveMessage(message, attestation)`; the
///      transmitter mints USDC to this contract and then calls back into
///      `handleReceiveMessage`. We deliberately keep this interface rather than
///      a `relay(message, attestation)` entrypoint so `destinationCaller` can
///      stay `bytes32(0)` (permissionless relay) — see cross_chain.rs F-CCTP-5.
///
///      Safety invariant: minted USDC is NEVER trapped. Any failure after mint
///      (swap revert, slippage miss, non-allowlisted token/router) routes the
///      full minted USDC to the user `recipient` and emits `HookRefunded`. The
///      swap is wrapped in `try/catch` so a reverting router can never bubble
///      up and leave the message consumed with funds stuck here. `nonReentrant`
///      guards the hook entrypoint, and `rescue` is an owner escape hatch for
///      any residual dust.
contract RebalanceExecutor is IMessageHandlerV2, Ownable, ReentrancyGuard {
    using SafeERC20 for IERC20;

    /// @notice Trusted CCTP V2 MessageTransmitter on this chain.
    address public immutable messageTransmitter;
    /// @notice USDC token on this chain.
    address public immutable usdc;
    /// @notice Uniswap V3 swap router. Mutable to allow upgrade.
    IUniswapV3SwapRouter public swapRouter;

    /// @notice Owner-managed allowlist of permitted swap output tokens. A hook
    ///         whose `tokenOut` is not allowlisted is refunded as USDC.
    mapping(address => bool) public allowedTokenOut;
    /// @notice Owner-managed allowlist of permitted swap routers. The active
    ///         `swapRouter` is implicitly trusted; this lets the owner pre-bless
    ///         a replacement before rotating to it.
    mapping(address => bool) public allowedRouter;

    event SwapRouterUpdated(address indexed previous, address indexed next);
    event TokenOutAllowed(address indexed token, bool allowed);
    event RouterAllowed(address indexed router, bool allowed);

    event HookSwapSettled(address indexed recipient, address indexed tokenOut, uint256 amountOut);
    event HookRefunded(address indexed recipient, uint256 usdcAmount, string reason);
    event Rescued(address indexed token, address indexed to, uint256 amount);

    /// @dev Retained for backward-compatible analytics; emitted on the USDC
    ///      passthrough fast path and on a successful swap.
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
        // The deploy-time router is trusted; the owner can bless others later.
        allowedRouter[address(_swapRouter)] = true;
    }

    function setSwapRouter(IUniswapV3SwapRouter next) external onlyOwner {
        if (address(next) == address(0)) revert ZeroAddress();
        emit SwapRouterUpdated(address(swapRouter), address(next));
        swapRouter = next;
        // Rotating in a router implicitly trusts it for swaps.
        allowedRouter[address(next)] = true;
        emit RouterAllowed(address(next), true);
    }

    /// @notice Allow / disallow a swap output token. USDC is always implicitly
    ///         allowed (it is the passthrough asset), so it need not be set.
    function setAllowedTokenOut(address token, bool allowed) external onlyOwner {
        if (token == address(0)) revert ZeroAddress();
        allowedTokenOut[token] = allowed;
        emit TokenOutAllowed(token, allowed);
    }

    /// @notice Allow / disallow a swap router. The active `swapRouter` is the
    ///         one actually used; this controls which routers are considered
    ///         trusted if rotated in.
    function setAllowedRouter(address router, bool allowed) external onlyOwner {
        if (router == address(0)) revert ZeroAddress();
        allowedRouter[router] = allowed;
        emit RouterAllowed(router, allowed);
    }

    /// @notice Owner escape hatch for any token stuck in this contract (residual
    ///         dust, an unexpected direct transfer, or a token a refund could
    ///         not reach). Cannot front-run user funds: refunds happen
    ///         atomically inside `handleReceiveMessage`, so under normal flow
    ///         this contract holds no user balance between messages.
    function rescue(address token, address to, uint256 amount) external onlyOwner {
        if (to == address(0)) revert ZeroAddress();
        IERC20(token).safeTransfer(to, amount);
        emit Rescued(token, to, amount);
    }

    /// @inheritdoc IMessageHandlerV2
    /// @dev `nonReentrant`: the only external call is into the swap router; a
    ///      malicious router cannot re-enter to replay the refund/settle logic.
    function handleReceiveMessage(uint32 sourceDomain, bytes32, bytes calldata messageBody)
        external
        override
        nonReentrant
        returns (bool)
    {
        if (msg.sender != messageTransmitter) revert OnlyMessageTransmitter();
        if (messageBody.length != 32 * 5) revert InvalidHookPayload();

        (address recipient, address tokenOut, uint24 poolFee, uint256 minOut, uint256 deadline) =
            abi.decode(messageBody, (address, address, uint24, uint256, uint256));

        // A zero recipient/tokenOut is the one case we still revert on: there is
        // no safe destination for a refund, so we must not consume the message.
        if (recipient == address(0) || tokenOut == address(0)) revert InvalidHookPayload();

        uint256 amountIn = IERC20(usdc).balanceOf(address(this));

        // Fast path: native USDC at the destination (no swap). Forward directly,
        // saving the Uniswap fee + slippage roundtrip.
        if (tokenOut == usdc) {
            IERC20(usdc).safeTransfer(recipient, amountIn);
            emit HookExecuted(sourceDomain, recipient, tokenOut, amountIn, amountIn);
            return true;
        }

        // Allowlist gate: an un-blessed tokenOut or router can never trap funds —
        // we refund the minted USDC to the user instead of attempting the swap.
        if (!allowedTokenOut[tokenOut]) {
            _refund(recipient, amountIn, "tokenOut not allowlisted");
            return true;
        }
        address router = address(swapRouter);
        if (!allowedRouter[router]) {
            _refund(recipient, amountIn, "router not allowlisted");
            return true;
        }

        IERC20(usdc).forceApprove(router, amountIn);

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

        // try/catch: any swap revert (slippage miss, bad pool, expired deadline,
        // out-of-gas surfaced as a revert) is caught and the minted USDC is
        // refunded to the user. The CCTP message is consumed exactly once and
        // funds are never trapped in this contract.
        try swapRouter.exactInputSingle(params) returns (uint256 amountOut) {
            emit HookExecuted(sourceDomain, recipient, tokenOut, amountIn, amountOut);
            emit HookSwapSettled(recipient, tokenOut, amountOut);
        } catch {
            // Drop the approval we granted to the failed router, then refund.
            IERC20(usdc).forceApprove(router, 0);
            _refund(recipient, amountIn, "swap failed");
        }
        return true;
    }

    function _refund(address recipient, uint256 amount, string memory reason) internal {
        IERC20(usdc).safeTransfer(recipient, amount);
        emit HookRefunded(recipient, amount, reason);
    }
}
