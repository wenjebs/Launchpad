// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "./interfaces/IUniswapV2Pair.sol";

contract ArbitrageValidator {
    /// @notice Validate whether an arbitrage cycle is profitable using current on-chain reserves.
    /// @param tokens Ordered array of token addresses in the cycle (first == last).
    /// @param pools Ordered array of Uniswap V2 pair addresses for each hop.
    /// @param amountIn Starting amount of tokens[0] in raw units.
    /// @param minOut Minimum acceptable output (set to amountIn for breakeven check).
    /// @return actualOut The simulated output amount from the full cycle.
    function validateCycle(
        address[] calldata tokens,
        address[] calldata pools,
        uint256 amountIn,
        uint256 minOut
    ) external returns (uint256 actualOut) {
        require(tokens.length >= 3, "Min 2 hops");
        require(tokens.length == pools.length + 1, "Length mismatch");
        require(tokens[0] == tokens[tokens.length - 1], "Not a cycle");

        uint256 currentAmount = amountIn;

        for (uint256 i = 0; i < pools.length; i++) {
            address tokenIn = tokens[i];
            address pool = pools[i];

            // Fetch current on-chain reserves
            (uint112 reserve0, uint112 reserve1, ) = IUniswapV2Pair(pool).getReserves();

            // Determine swap direction
            address token0 = IUniswapV2Pair(pool).token0();
            (uint256 reserveIn, uint256 reserveOut) = tokenIn == token0
                ? (uint256(reserve0), uint256(reserve1))
                : (uint256(reserve1), uint256(reserve0));

            // Uniswap V2 getAmountOut with 0.3% fee
            uint256 amountInWithFee = currentAmount * 997;
            uint256 numerator = amountInWithFee * reserveOut;
            uint256 denominator = reserveIn * 1000 + amountInWithFee;
            currentAmount = numerator / denominator;
        }

        require(currentAmount >= minOut, "Below minimum output");
        actualOut = currentAmount;
    }
}
