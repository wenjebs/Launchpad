// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "../src/ArbitrageValidator.sol";
import "./mocks/MockUniswapV2Pair.sol";

// Minimal test harness (no Foundry dependency — can be tested via solc + script or Foundry)
// If using Foundry: forge test
// If not: compile and run via the Node.js test script

contract ArbitrageValidatorTest {
    ArbitrageValidator public validator;

    // Token addresses (arbitrary, just need to be distinct and ordered)
    address constant TOKEN_A = address(0x1);
    address constant TOKEN_B = address(0x2);
    address constant TOKEN_C = address(0x3);

    constructor() {
        validator = new ArbitrageValidator();
    }

    /// @notice Test a profitable 2-hop cycle: A -> B -> A
    /// Pool1: A/B with reserves 100e18 / 200e18 (A is cheap relative to B)
    /// Pool2: B/A with reserves 150e18 / 400e18 (A is even cheaper — arb exists)
    function testProfitable2Hop() external returns (bool) {
        MockUniswapV2Pair pool1 = new MockUniswapV2Pair(TOKEN_A, TOKEN_B, 100e18, 200e18);
        MockUniswapV2Pair pool2 = new MockUniswapV2Pair(TOKEN_B, TOKEN_A, 150e18, 400e18);

        address[] memory tokens = new address[](3);
        tokens[0] = TOKEN_A;
        tokens[1] = TOKEN_B;
        tokens[2] = TOKEN_A;

        address[] memory pools = new address[](2);
        pools[0] = address(pool1);
        pools[1] = address(pool2);

        uint256 amountIn = 1e18;
        // Hop 1: 1e18 A -> B via pool1 (100e18 A / 200e18 B)
        // out = (1e18 * 997 * 200e18) / (100e18 * 1000 + 1e18 * 997)
        //     = 199400e36 / 100997e18 = 1.974...e18 B
        // Hop 2: ~1.974e18 B -> A via pool2 (150e18 B / 400e18 A)
        // out = (1.974e18 * 997 * 400e18) / (150e18 * 1000 + 1.974e18 * 997)
        //     should be > 1e18 A (profitable)

        uint256 actualOut = validator.validateCycle(tokens, pools, amountIn, amountIn);
        require(actualOut > amountIn, "Should be profitable");
        return true;
    }

    /// @notice Test an unprofitable cycle (same reserves both directions)
    function testUnprofitable2Hop() external returns (bool) {
        MockUniswapV2Pair pool1 = new MockUniswapV2Pair(TOKEN_A, TOKEN_B, 100e18, 100e18);
        MockUniswapV2Pair pool2 = new MockUniswapV2Pair(TOKEN_B, TOKEN_A, 100e18, 100e18);

        address[] memory tokens = new address[](3);
        tokens[0] = TOKEN_A;
        tokens[1] = TOKEN_B;
        tokens[2] = TOKEN_A;

        address[] memory pools = new address[](2);
        pools[0] = address(pool1);
        pools[1] = address(pool2);

        // With equal reserves and 0.3% fee each hop, output < input
        // Should revert with "Below minimum output"
        try validator.validateCycle(tokens, pools, 1e18, 1e18) returns (uint256) {
            revert("Should have reverted");
        } catch Error(string memory reason) {
            require(
                keccak256(bytes(reason)) == keccak256("Below minimum output"),
                "Wrong revert reason"
            );
        }
        return true;
    }

    /// @notice Test: tokens.length < 3 should revert
    function testMinHopsRevert() external returns (bool) {
        address[] memory tokens = new address[](2);
        tokens[0] = TOKEN_A;
        tokens[1] = TOKEN_A;

        address[] memory pools = new address[](1);
        pools[0] = address(0xdead);

        try validator.validateCycle(tokens, pools, 1e18, 1e18) returns (uint256) {
            revert("Should have reverted");
        } catch Error(string memory reason) {
            require(
                keccak256(bytes(reason)) == keccak256("Min 2 hops"),
                "Wrong revert reason"
            );
        }
        return true;
    }

    /// @notice Test: length mismatch should revert
    function testLengthMismatchRevert() external returns (bool) {
        address[] memory tokens = new address[](3);
        tokens[0] = TOKEN_A;
        tokens[1] = TOKEN_B;
        tokens[2] = TOKEN_A;

        address[] memory pools = new address[](3); // should be 2

        try validator.validateCycle(tokens, pools, 1e18, 1e18) returns (uint256) {
            revert("Should have reverted");
        } catch Error(string memory reason) {
            require(
                keccak256(bytes(reason)) == keccak256("Length mismatch"),
                "Wrong revert reason"
            );
        }
        return true;
    }

    /// @notice Test: non-cyclic path should revert
    function testNotCycleRevert() external returns (bool) {
        address[] memory tokens = new address[](3);
        tokens[0] = TOKEN_A;
        tokens[1] = TOKEN_B;
        tokens[2] = TOKEN_C; // not equal to tokens[0]

        address[] memory pools = new address[](2);

        try validator.validateCycle(tokens, pools, 1e18, 1e18) returns (uint256) {
            revert("Should have reverted");
        } catch Error(string memory reason) {
            require(
                keccak256(bytes(reason)) == keccak256("Not a cycle"),
                "Wrong revert reason"
            );
        }
        return true;
    }

    /// @notice Test a profitable 3-hop cycle
    function testProfitable3Hop() external returns (bool) {
        // A -> B -> C -> A with price discrepancies
        MockUniswapV2Pair pool1 = new MockUniswapV2Pair(TOKEN_A, TOKEN_B, 100e18, 300e18);
        MockUniswapV2Pair pool2 = new MockUniswapV2Pair(TOKEN_B, TOKEN_C, 200e18, 100e18);
        MockUniswapV2Pair pool3 = new MockUniswapV2Pair(TOKEN_C, TOKEN_A, 50e18, 200e18);

        address[] memory tokens = new address[](4);
        tokens[0] = TOKEN_A;
        tokens[1] = TOKEN_B;
        tokens[2] = TOKEN_C;
        tokens[3] = TOKEN_A;

        address[] memory pools = new address[](3);
        pools[0] = address(pool1);
        pools[1] = address(pool2);
        pools[2] = address(pool3);

        uint256 actualOut = validator.validateCycle(tokens, pools, 1e18, 1e18);
        require(actualOut > 1e18, "Should be profitable");
        return true;
    }

    /// @notice Run all tests, revert if any fail
    function runAllTests() external returns (string memory) {
        this.testProfitable2Hop();
        this.testUnprofitable2Hop();
        this.testMinHopsRevert();
        this.testLengthMismatchRevert();
        this.testNotCycleRevert();
        this.testProfitable3Hop();
        return "All 6 tests passed";
    }
}
