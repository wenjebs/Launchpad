# On-Chain Profitability Validation Specification (Part 2 - Optional)

## Overview

A Solidity smart contract that validates whether a candidate arbitrage cycle is still profitable using current on-chain reserves. If not profitable, the transaction reverts.

## Contract Interface

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

interface IArbitrageValidator {
    /// @notice Validate and optionally execute an arbitrage cycle
    /// @param tokens Ordered array of token addresses in the cycle (first == last)
    /// @param pools Ordered array of Uniswap V2 pair addresses for each hop
    /// @param amountIn Starting amount of tokens[0]
    /// @param minOut Minimum acceptable output (must be > amountIn for profit)
    /// @return actualOut The actual output amount from the cycle
    function validateCycle(
        address[] calldata tokens,
        address[] calldata pools,
        uint256 amountIn,
        uint256 minOut
    ) external returns (uint256 actualOut);
}
```

## Data Encoding

### Calldata Layout

```
tokens[]:  [tokenA, tokenB, tokenC, tokenA]   // length = hops + 1
pools[]:   [poolAB, poolBC, poolCA]            // length = hops
amountIn:  uint256
minOut:    uint256
```

### ABI Encoding (for script submission)

```solidity
bytes memory data = abi.encodeWithSelector(
    IArbitrageValidator.validateCycle.selector,
    tokens,
    pools,
    amountIn,
    minOut
);
```

## Validation Logic

```solidity
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
        address tokenOut = tokens[i + 1];
        address pool = pools[i];

        // Fetch current reserves
        (uint112 reserve0, uint112 reserve1, ) = IUniswapV2Pair(pool).getReserves();

        // Determine direction
        address token0 = IUniswapV2Pair(pool).token0();
        (uint256 reserveIn, uint256 reserveOut) = tokenIn == token0
            ? (uint256(reserve0), uint256(reserve1))
            : (uint256(reserve1), uint256(reserve0));

        // Compute swap output (Uniswap V2 formula)
        uint256 amountInWithFee = currentAmount * 997;
        uint256 numerator = amountInWithFee * reserveOut;
        uint256 denominator = reserveIn * 1000 + amountInWithFee;
        currentAmount = numerator / denominator;
    }

    require(currentAmount >= minOut, "Below minimum output");
    actualOut = currentAmount;
}
```

## Revert Conditions

| Condition | Error |
|-----------|-------|
| `tokens.length < 3` | "Min 2 hops" |
| `tokens.length != pools.length + 1` | "Length mismatch" |
| `tokens[0] != tokens[last]` | "Not a cycle" |
| `currentAmount < minOut` | "Below minimum output" |
| Pool `getReserves()` fails | Revert from external call |
| Zero reserves in pool | Division by zero (inherent revert) |

## Execution vs. Dry-Run

The contract above is a **dry-run validator** (read-only simulation). For actual execution, you'd need:

1. Flash swap from the first pool
2. Execute each hop (transfer tokens + call `swap()`)
3. Repay the flash swap
4. Keep the profit

### Flash Swap Pattern (for actual execution)

```solidity
// 1. Initiate flash swap from pool[0]
IUniswapV2Pair(pools[0]).swap(amount0Out, amount1Out, address(this), callbackData);

// 2. In the callback (uniswapV2Call):
function uniswapV2Call(address sender, uint256 amount0, uint256 amount1, bytes calldata data) external {
    // Decode cycle data
    // Execute remaining hops
    // Repay first pool + fee
    // Keep profit
}
```

## Deployment & Testing

### Foundry Setup

```bash
forge init arbitrage-validator
cd arbitrage-validator
# Write contract in src/ArbitrageValidator.sol
# Write tests in test/ArbitrageValidator.t.sol
```

### Test Strategy

1. **Unit test**: Mock pool reserves, verify swap math matches off-chain calculation
2. **Fork test**: Fork mainnet, use real pool addresses, validate against real reserves
3. **Revert test**: Verify revert when cycle is not profitable

### Testnet Deployment (Sepolia)

```bash
forge create src/ArbitrageValidator.sol:ArbitrageValidator \
    --rpc-url $SEPOLIA_RPC_URL \
    --private-key $PRIVATE_KEY
```

Note: Sepolia doesn't have real Uniswap V2 pools with meaningful liquidity. Options:
- Deploy mock pairs with controlled reserves
- Use mainnet fork for realistic testing (`forge test --fork-url $MAINNET_RPC`)

## Submission Script

A script (Rust or TypeScript) that:

1. Reads top-10 cycles from Part 1 output
2. Encodes each cycle as calldata
3. Calls `validateCycle()` via `eth_call` (static call, no gas cost)
4. Reports which cycles are still profitable and the expected output
5. Optionally submits profitable cycles as transactions

```
For each cycle in top_10:
    calldata = encode(tokens, pools, amountIn, minOut=amountIn)
    result = eth_call(to=validator, data=calldata)
    if success:
        print("Cycle {i}: profitable, output = {result}")
    else:
        print("Cycle {i}: not profitable (reverted)")
```
