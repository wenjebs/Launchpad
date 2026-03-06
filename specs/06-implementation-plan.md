# Implementation Plan

## Project Structure

```
dex-arbitrage/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI entry point
│   ├── types.rs             # Token, Pool, Edge, Cycle structs
│   ├── loader.rs            # JSON data ingestion
│   ├── graph.rs             # Graph construction
│   ├── amm.rs               # Swap math (getAmountOut, optimal sizing)
│   ├── detector.rs          # Cycle detection (Bellman-Ford + DFS)
│   ├── ranker.rs            # Profit simulation and ranking
│   └── output.rs            # Format and print results
├── data/
│   └── pools.json           # Input dataset
├── contracts/               # (Part 2)
│   ├── foundry.toml
│   ├── src/
│   │   └── ArbitrageValidator.sol
│   ├── test/
│   │   └── ArbitrageValidator.t.sol
│   └── script/
│       └── Submit.s.sol
└── report.md
```

## Dependencies (Cargo.toml)

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
petgraph = "0.7"
num = "0.4"              # BigInt/BigUint for reserve arithmetic
clap = { version = "4", features = ["derive"] }  # CLI args
```

Note: For U256 arithmetic, evaluate `ethnum`, `ruint`, or `primitive-types`. If reserves fit in u128 (most Uniswap V2 pools do), plain u128 arithmetic suffices and avoids extra dependencies.

## Implementation Steps

### Step 1: Data Types and Loading (~15 min)

- Define `Token`, `Pool` structs with serde derives
- Parse JSON file into `Vec<Pool>`
- Handle string-to-integer conversion for reserves
- Validate: skip pools with zero reserves

### Step 2: Graph Construction (~20 min)

- Build `SwapGraph` with adjacency list
- Create bidirectional edges per pool
- Build token index (address -> node ID)
- Print stats: node count, edge count

### Step 3: AMM Math (~15 min)

- Implement `get_amount_out(amount_in, reserve_in, reserve_out, fee_bps) -> u128`
- Implement `simulate_cycle(cycle, amount_in, edges) -> u128`
- Implement optimal input calculation (virtual reserve method)
- Unit tests against known values

### Step 4: Cycle Detection (~30 min)

- Implement bounded DFS (max 4 hops)
- Start from high-degree nodes first
- Deduplicate using canonical form
- Pruning: skip if intermediate amount drops below threshold

### Step 5: Profit Ranking (~15 min)

- For each candidate cycle, compute optimal input and max profit
- Sort by profit descending
- Take top 10
- Format output table + JSON

### Step 6: Report (~15 min)

- Document approach in report.md
- Include graph construction, algorithm choice, ranking logic
- Note AI tool usage

### Step 7 (Optional): Solidity Contract (~30 min)

- Write `ArbitrageValidator.sol`
- Fork test with Foundry
- Write submission script

## Key Design Decisions

### Integer Arithmetic

Use `u128` for reserve math. Uniswap V2 reserves are `uint112` (max ~5.19e33), and intermediate products `amount * 997 * reserve` fit in u128 for realistic values. Overflow check: `u112 * u16 * u112 < u128`.

If the dataset has unusually large reserves, fall back to `U256`.

### Cycle Length Limit

Cap at 4 hops:
- 2-hop: Two pools for same pair, one mispriced (rare but very profitable)
- 3-hop: Most common triangular arbitrage
- 4-hop: Diminishing returns, more gas, more fee erosion

### Hub-First Search

Most profitable cycles flow through high-liquidity tokens (WETH, USDC, USDT, DAI, WBTC). Start DFS from these "hub" nodes to find the best opportunities faster.

### Floating Point vs Integer

- Graph edge weights (for Bellman-Ford): use f64 for -ln(rate) — acceptable for detection
- Profit simulation: use integer arithmetic (u128) — required for accuracy
- Final ranking: based on integer simulation, not float approximation

## Verification Checklist

- [ ] AMM formula matches Uniswap V2 getAmountOut exactly
- [ ] Bidirectional edges created for each pool
- [ ] Cycles are deduplicated
- [ ] Optimal trade size is positive and reasonable
- [ ] Profit is computed as integer difference, not float
- [ ] Output includes both raw and human-readable values
- [ ] Edge cases: zero reserves, single-token pools, duplicate pools
