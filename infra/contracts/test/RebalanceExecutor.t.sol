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

    function test_slippage_too_tight_reverts() public {
        usdc.mint(address(executor), 1_000_000);

        vm.prank(messageTransmitter);
        vm.expectRevert(bytes("slippage"));
        executor.handleReceiveMessage(
            6, bytes32(0), _hookPayload(user, address(weth), 3000, type(uint256).max)
        );
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
    }

    function test_non_owner_cannot_rotate() public {
        MockSwapRouter newRouter = new MockSwapRouter(weth, 1e18);
        vm.prank(user);
        vm.expectRevert();
        executor.setSwapRouter(newRouter);
    }
}
