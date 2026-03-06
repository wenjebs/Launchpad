# AMM Mechanics - Uniswap V2 Swap Formula

## Constant Product Market Maker (CPMM)

Uniswap V2 pools maintain the invariant:

```
x * y = k
```

Where:
- `x` = reserve of token0
- `y` = reserve of token1
- `k` = constant product (increases slightly per trade due to fees)

## Swap Output Formula (getAmountOut)

Given an input amount, compute how many output tokens you receive:

```
amountOut = (amountIn * 997 * reserveOut) / (reserveIn * 1000 + amountIn * 997)
```

Breakdown:
- `997/1000` = fee factor (0.3% fee deducted from input)
- `amountInWithFee = amountIn * 997`
- `numerator = amountInWithFee * reserveOut`
- `denominator = reserveIn * 1000 + amountInWithFee`

### In generalized form with fee parameter `r`:

```
r = (10000 - feeBps) / 10000     # e.g., r = 0.997 for 30bps fee

amountOut = (r * amountIn * reserveOut) / (reserveIn + r * amountIn)
```

## Swap Input Formula (getAmountIn)

Given a desired output, compute how many input tokens are needed:

```
amountIn = (reserveIn * amountOut * 1000) / ((reserveOut - amountOut) * 997) + 1
```

## Multi-Hop Swap Simulation

For a path `A -> B -> C -> A`:

```
Step 1: amountB = getAmountOut(amountA, reserveA_pool1, reserveB_pool1)
Step 2: amountC = getAmountOut(amountB, reserveB_pool2, reserveC_pool2)
Step 3: amountA' = getAmountOut(amountC, reserveC_pool3, reserveA_pool3)

Profit = amountA' - amountA
```

## Price Impact

Larger trades move the price more. The effective exchange rate degrades with size:

```
Effective rate = amountOut / amountIn
Spot rate = reserveOut / reserveIn
Slippage = 1 - (effective_rate / spot_rate)
```

## Token Decimals

Reserves are stored in raw integer units. A token with 18 decimals stores `1.0` as `1000000000000000000`. When computing profit in human-readable terms:

```
profit_human = profit_raw / 10^decimals
```

All internal arithmetic should use raw integer values to avoid floating-point errors.

## Edge Cases

| Case | Handling |
|------|----------|
| `amountIn = 0` | Output is 0, skip |
| `reserveIn = 0` or `reserveOut = 0` | Pool has no liquidity, skip edge |
| Very small reserves | May cause integer overflow in multiplication; use u128/u256 |
| Same token as token0 and token1 | Invalid pool, skip |
| Multiple pools for same pair | Keep all as separate edges (different liquidity = different rates) |
