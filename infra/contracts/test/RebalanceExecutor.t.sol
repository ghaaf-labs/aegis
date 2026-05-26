// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import { Test } from "forge-std/Test.sol";
import { IERC20 } from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import { SafeERC20 } from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import { ERC20 } from "@openzeppelin/contracts/token/ERC20/ERC20.sol";

import { RebalanceExecutor } from "../src/RebalanceExecutor.sol";
import { IUniswapV3SwapRouter } from "../src/interfaces/IUniswapV3SwapRouter.sol";

contract MockUSDC is ERC20 {
    constructor() ERC20("Mock USDC", "USDC") { }

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

contract MockTokenOut is ERC20 {
    constructor() ERC20("Mock WETH", "WETH") { }

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

/// @dev Deterministic mock router: pays `amountIn * rate / 1e18` of tokenOut.
contract MockSwapRouter is IUniswapV3SwapRouter {
    using SafeERC20 for IERC20;

    uint256 public rate;
    MockTokenOut public tokenOut;
    bool public shouldRevert;

    constructor(MockTokenOut _tokenOut, uint256 _rate) {
        tokenOut = _tokenOut;
        rate = _rate;
    }

    function setShouldRevert(bool v) external {
        shouldRevert = v;
    }

    function exactInputSingle(ExactInputSingleParams calldata params)
        external
        payable
        returns (uint256 amountOut)
    {
        require(!shouldRevert, "router-revert");
        IERC20(params.tokenIn).safeTransferFrom(msg.sender, address(this), params.amountIn);
        amountOut = (params.amountIn * rate) / 1e18;
        require(amountOut >= params.amountOutMinimum, "slippage");
        tokenOut.mint(params.recipient, amountOut);
    }
}

/// @dev Router that tries to re-enter the executor's hook entrypoint mid-swap.
///      `nonReentrant` is the first modifier on `handleReceiveMessage`, so the
///      re-entry reverts with `ReentrancyGuardReentrantCall` before any
///      transmitter check — proving the guard fires. The executor's `try/catch`
///      turns that revert into a refund.
contract ReentrantRouter is IUniswapV3SwapRouter {
    RebalanceExecutor public target;
    bytes public replayBody;

    function arm(RebalanceExecutor _target, bytes calldata _body) external {
        target = _target;
        replayBody = _body;
    }

    function exactInputSingle(ExactInputSingleParams calldata) external payable returns (uint256) {
        // Re-enter the guarded hook. `nonReentrant` reverts here.
        target.handleReceiveMessage(6, bytes32(0), replayBody);
        return 0;
    }
}

contract RebalanceExecutorTest is Test {
    RebalanceExecutor executor;
    MockUSDC usdc;
    MockTokenOut weth;
    MockSwapRouter router;

    address messageTransmitter = address(0xCCCC);
    address user = address(0xBEEF);

    function setUp() public {
        usdc = new MockUSDC();
        weth = new MockTokenOut();
        // 1 USDC = 0.00033 WETH (ETH at ~$3000).
        router = new MockSwapRouter(weth, 333_000_000_000_000); // 3.33e14
        executor = new RebalanceExecutor(messageTransmitter, address(usdc), router);
        // Bless WETH as a swap output; the deploy router is allowlisted in ctor.
        executor.setAllowedTokenOut(address(weth), true);
    }

    function _hookPayload(address recipient, address tokenOut, uint24 fee, uint256 minOut)
        internal
        view
        returns (bytes memory)
    {
        return abi.encode(recipient, tokenOut, fee, minOut, block.timestamp + 600);
    }

    function test_swap_pays_user() public {
        usdc.mint(address(executor), 1_000_000); // 1 USDC

        vm.prank(messageTransmitter);
        executor.handleReceiveMessage(6, bytes32(0), _hookPayload(user, address(weth), 3000, 0));

        assertEq(weth.balanceOf(user), (1_000_000 * 333_000_000_000_000) / 1e18);
        assertEq(usdc.balanceOf(address(executor)), 0);
    }

    function test_unauthorized_caller_reverts() public {
        usdc.mint(address(executor), 1_000_000);

        vm.expectRevert(RebalanceExecutor.OnlyMessageTransmitter.selector);
        executor.handleReceiveMessage(6, bytes32(0), _hookPayload(user, address(weth), 3000, 0));
    }

    function test_invalid_payload_length_reverts() public {
        usdc.mint(address(executor), 1_000_000);

        vm.prank(messageTransmitter);
        vm.expectRevert(RebalanceExecutor.InvalidHookPayload.selector);
        executor.handleReceiveMessage(6, bytes32(0), abi.encodePacked(uint256(1)));
    }

    function test_usdc_passthrough_skips_swap() public {
        usdc.mint(address(executor), 5_000_000); // 5 USDC

        // shouldRevert=true ensures we never touch the router on the passthrough path
        router.setShouldRevert(true);

        vm.prank(messageTransmitter);
        executor.handleReceiveMessage(6, bytes32(0), _hookPayload(user, address(usdc), 0, 0));

        assertEq(usdc.balanceOf(user), 5_000_000);
        assertEq(usdc.balanceOf(address(executor)), 0);
    }

    function test_zero_recipient_reverts() public {
        usdc.mint(address(executor), 1_000_000);

        vm.prank(messageTransmitter);
        vm.expectRevert(RebalanceExecutor.InvalidHookPayload.selector);
        executor.handleReceiveMessage(
            6, bytes32(0), _hookPayload(address(0), address(weth), 3000, 0)
        );
    }

    function test_owner_can_rotate_swap_router() public {
        MockSwapRouter newRouter = new MockSwapRouter(weth, 1e18);
        executor.setSwapRouter(newRouter);
        assertEq(address(executor.swapRouter()), address(newRouter));
        // Rotating in a router implicitly allowlists it.
        assertTrue(executor.allowedRouter(address(newRouter)));
    }

    function test_non_owner_cannot_rotate() public {
        MockSwapRouter newRouter = new MockSwapRouter(weth, 1e18);
        vm.prank(user);
        vm.expectRevert();
        executor.setSwapRouter(newRouter);
    }

    // ── Refund-on-failure ──────────────────────────────────────────────────

    function test_swap_revert_refunds_usdc_to_user() public {
        usdc.mint(address(executor), 1_000_000);
        router.setShouldRevert(true);

        vm.expectEmit(true, false, false, true, address(executor));
        emit RebalanceExecutor.HookRefunded(user, 1_000_000, "swap failed");

        vm.prank(messageTransmitter);
        bool ok = executor.handleReceiveMessage(
            6, bytes32(0), _hookPayload(user, address(weth), 3000, 0)
        );

        // Message consumed (no revert), funds at the user, none trapped.
        assertTrue(ok);
        assertEq(usdc.balanceOf(user), 1_000_000);
        assertEq(weth.balanceOf(user), 0);
        assertEq(usdc.balanceOf(address(executor)), 0);
    }

    /// @dev Fund-Safety Theorem (spec §17), encoded on-chain: across *every*
    ///      destination-side failure mode, the recipient ends with the full
    ///      minted USDC and the executor holds nothing — funds are never
    ///      stranded in the contract or an intermediate token.
    function test_fund_safety_theorem_failure_modes_always_refund_full_usdc() public {
        uint256 amountIn = 1_000_000; // 1 USDC per mode

        // Mode 1 — the swap router reverts.
        router.setShouldRevert(true);
        _assertRefundsFull(amountIn, _hookPayload(user, address(weth), 3000, 0));
        router.setShouldRevert(false);

        // Mode 2 — min-out cannot be met (slippage).
        _assertRefundsFull(amountIn, _hookPayload(user, address(weth), 3000, type(uint256).max));

        // Mode 3 — tokenOut is not on the allowlist.
        MockTokenOut other = new MockTokenOut();
        _assertRefundsFull(amountIn, _hookPayload(user, address(other), 3000, 0));
    }

    /// Mint `amountIn` to the executor, run the hook, and assert the full amount
    /// landed back with the user and nothing is stranded in the executor.
    function _assertRefundsFull(uint256 amountIn, bytes memory payload) internal {
        uint256 userBefore = usdc.balanceOf(user);
        usdc.mint(address(executor), amountIn);

        vm.prank(messageTransmitter);
        executor.handleReceiveMessage(6, bytes32(0), payload);

        assertEq(usdc.balanceOf(user) - userBefore, amountIn, "full USDC refunded (no strand)");
        assertEq(usdc.balanceOf(address(executor)), 0, "executor holds no stranded USDC");
    }

    function test_minout_miss_refunds_usdc_to_user() public {
        usdc.mint(address(executor), 1_000_000);

        // Demand an impossibly high minOut so the router's slippage check trips.
        vm.expectEmit(true, false, false, true, address(executor));
        emit RebalanceExecutor.HookRefunded(user, 1_000_000, "swap failed");

        vm.prank(messageTransmitter);
        executor.handleReceiveMessage(
            6, bytes32(0), _hookPayload(user, address(weth), 3000, type(uint256).max)
        );

        assertEq(usdc.balanceOf(user), 1_000_000);
        assertEq(usdc.balanceOf(address(executor)), 0);
    }

    function test_successful_swap_emits_settled() public {
        usdc.mint(address(executor), 1_000_000);
        uint256 expectedOut = (1_000_000 * 333_000_000_000_000) / 1e18;

        vm.expectEmit(true, true, false, true, address(executor));
        emit RebalanceExecutor.HookSwapSettled(user, address(weth), expectedOut);

        vm.prank(messageTransmitter);
        executor.handleReceiveMessage(6, bytes32(0), _hookPayload(user, address(weth), 3000, 0));
    }

    // ── Allowlist rejection + refund ───────────────────────────────────────

    function test_non_allowlisted_tokenout_refunds() public {
        MockTokenOut other = new MockTokenOut();
        usdc.mint(address(executor), 2_000_000);
        // `other` is never allowlisted; the router must not be touched even if
        // it would revert.
        router.setShouldRevert(true);

        vm.expectEmit(true, false, false, true, address(executor));
        emit RebalanceExecutor.HookRefunded(user, 2_000_000, "tokenOut not allowlisted");

        vm.prank(messageTransmitter);
        executor.handleReceiveMessage(6, bytes32(0), _hookPayload(user, address(other), 3000, 0));

        assertEq(usdc.balanceOf(user), 2_000_000);
        assertEq(usdc.balanceOf(address(executor)), 0);
    }

    function test_non_allowlisted_router_refunds() public {
        // Owner allowlists a fresh tokenOut, then deauthorizes the active router.
        usdc.mint(address(executor), 1_500_000);
        executor.setAllowedRouter(address(router), false);

        vm.expectEmit(true, false, false, true, address(executor));
        emit RebalanceExecutor.HookRefunded(user, 1_500_000, "router not allowlisted");

        vm.prank(messageTransmitter);
        executor.handleReceiveMessage(6, bytes32(0), _hookPayload(user, address(weth), 3000, 0));

        assertEq(usdc.balanceOf(user), 1_500_000);
        assertEq(usdc.balanceOf(address(executor)), 0);
    }

    function test_only_owner_can_set_allowlists() public {
        vm.startPrank(user);
        vm.expectRevert();
        executor.setAllowedTokenOut(address(weth), true);
        vm.expectRevert();
        executor.setAllowedRouter(address(router), true);
        vm.stopPrank();
    }

    // ── Reentrancy ─────────────────────────────────────────────────────────

    function test_reentrant_router_is_rejected_and_refunds() public {
        ReentrantRouter evil = new ReentrantRouter();
        executor.setSwapRouter(evil); // also allowlists it

        usdc.mint(address(executor), 1_000_000);
        bytes memory body = _hookPayload(user, address(weth), 3000, 0);
        evil.arm(executor, body);

        // The re-entry hits nonReentrant and reverts; the outer call's try/catch
        // turns that into a refund. The message is consumed exactly once.
        vm.expectEmit(true, false, false, true, address(executor));
        emit RebalanceExecutor.HookRefunded(user, 1_000_000, "swap failed");

        vm.prank(messageTransmitter);
        bool ok = executor.handleReceiveMessage(6, bytes32(0), body);
        assertTrue(ok);
        assertEq(usdc.balanceOf(user), 1_000_000);
        assertEq(usdc.balanceOf(address(executor)), 0);
    }

    // ── Rescue ─────────────────────────────────────────────────────────────

    function test_owner_can_rescue_stuck_tokens() public {
        // Simulate dust stuck in the contract (e.g. a direct transfer).
        usdc.mint(address(executor), 750_000);

        vm.expectEmit(true, true, false, true, address(executor));
        emit RebalanceExecutor.Rescued(address(usdc), user, 750_000);

        executor.rescue(address(usdc), user, 750_000);
        assertEq(usdc.balanceOf(user), 750_000);
        assertEq(usdc.balanceOf(address(executor)), 0);
    }

    function test_non_owner_cannot_rescue() public {
        usdc.mint(address(executor), 750_000);
        vm.prank(user);
        vm.expectRevert();
        executor.rescue(address(usdc), user, 750_000);
    }
}
