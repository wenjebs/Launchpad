# Profit Simulation and Ranking Specification

## Profit Simulation

For each candidate cycle, simulate the actual swap using the AMM formula.

### Single-Hop Swap

```rust
fn get_amount_out(amount_in: U256, reserve_in: U256, reserve_out: U256, fee_bps: u16) -> U256 {
    let fee_factor = 10000 - fee_bps;  // e.g., 9970 for 30bps
    let amount_in_with_fee = amount_in * fee_factor;
    let numerator = amount_in_with_fee * reserve_out;
    let denominator = reserve_in * 10000 + amount_in_with_fee;
    numerator / denominator
}
```

### Multi-Hop Simulation

```
fn simulate_cycle(cycle: &ArbitrageCycle, amount_in: U256, edges: &[PoolEdge]) -> U256 {
    let mut current_amount = amount_in;
    for &edge_idx in &cycle.edges {
        let edge = &edges[edge_idx];
        current_amount = get_amount_out(
            current_amount,
            edge.reserve_in,
            edge.reserve_out,
            edge.fee_bps,
        );
    }
    current_amount  // this is amount_out; profit = amount_out - amount_in
}
```

## Optimal Trade Size

The profit function `f(x) = simulate_cycle(x) - x` is concave: it rises, peaks, then falls as price impact increases. We need to find the peak.

### Method 1: Closed-Form via Virtual Reserves (Recommended)

Collapse multi-hop path into a single "virtual pool" with effective reserves.

For a 2-hop path through pools (R0, R1) and (R2, R3) with fee factor r:

```
Ea = (R0 * R2) / (R2 + r * R1)     # virtual reserveIn
Eb = (r * R1 * R3) / (R2 + r * R1)  # virtual reserveOut
```

For a 3-hop path, apply the reduction iteratively:
1. Combine hops 1+2 into virtual pool (Ea, Eb)
2. Combine virtual pool + hop 3 into final virtual pool (Ea', Eb')

**Optimal input amount:**
```
x_opt = (sqrt(Ea * Eb * r) - Ea) / r
```

**Profitability condition:** `Ea < Eb` (virtual reserveIn < virtual reserveOut)

**Maximum profit:**
```
profit_max = simulate_cycle(x_opt) - x_opt
```

### Method 2: Golden Section Search (Fallback)

If the closed-form is too complex or for verification:

```
Search for maximum of f(x) = simulate_cycle(x) - x
over interval [0, max_reasonable_input]

Golden section search:
  a, b = 0, max_input
  phi = (sqrt(5) - 1) / 2
  while b - a > tolerance:
      x1 = b - phi * (b - a)
      x2 = a + phi * (b - a)
      if f(x1) < f(x2):
          a = x1
      else:
          b = x2
  return (a + b) / 2
```

`max_reasonable_input`: use 10% of the smallest reserve along the cycle as upper bound.

### Method 3: Binary Search on Derivative

Since f(x) is concave, f'(x) is monotonically decreasing. Binary search for f'(x) = 0:

```
Approximate f'(x) numerically:
  f'(x) ~ (f(x + dx) - f(x - dx)) / (2 * dx)

Binary search:
  lo, hi = 0, max_input
  while hi - lo > tolerance:
      mid = (lo + hi) / 2
      if f'(mid) > 0: lo = mid
      else: hi = mid
  return mid
```

## Ranking Metric

### Primary: Absolute Profit at Optimal Size

```
rank_score = profit_max = simulate_cycle(x_opt) - x_opt
```

Convert to a common denomination (e.g., WETH or USD) for cross-token comparison.

### Secondary Metrics (for tiebreaking / filtering)

| Metric | Formula | Use |
|--------|---------|-----|
| ROI | `profit / x_opt` | Capital efficiency |
| Profit after gas | `profit - estimated_gas_cost` | Practical viability |
| Hop count | `cycle.hops` | Fewer hops = less gas |
| Min pool liquidity | `min(reserves along path)` | Execution reliability |

### Gas Cost Estimation

Approximate gas costs per hop:
- Uniswap V2 swap: ~150,000 gas per hop
- Total for n-hop cycle: ~(n * 150,000 + 21,000) gas
- Convert to ETH: `gas_used * gas_price`
- Convert to token terms for comparison with profit

## Output Format

### Top-10 Table

```
Rank | Cycle                          | Hops | Optimal Input    | Profit          | ROI    | Profit (USD est)
-----|--------------------------------|------|------------------|-----------------|--------|------------------
1    | WETH->USDC->DAI->WETH          | 3    | 2.5 ETH          | 0.015 ETH       | 0.60%  | $45.00
2    | WETH->USDT->WBTC->WETH         | 3    | 1.2 ETH          | 0.008 ETH       | 0.67%  | $24.00
...
```

### JSON Output

```json
{
  "cycles": [
    {
      "rank": 1,
      "path": ["WETH", "USDC", "DAI", "WETH"],
      "pool_addresses": ["0x...", "0x...", "0x..."],
      "hops": 3,
      "optimal_input_raw": "2500000000000000000",
      "optimal_input_human": "2.5 WETH",
      "profit_raw": "15000000000000000",
      "profit_human": "0.015 WETH",
      "roi_pct": 0.60
    }
  ]
}
```
