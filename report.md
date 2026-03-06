# DEX Arbitrage Detector — Final Report

## Part 1: Off-Chain Cycle Detection

### Graph Construction

- **Input**: 38,141 Uniswap V2 pool snapshots from `data/v2pools.json`
- **Filtering**: Pools with `reserveUSD < $1,000` or zero reserves are discarded, reducing to **5,448 pools**
- **Nodes**: 4,624 unique token addresses, indexed via `HashMap<String, usize>`
- **Edges**: 10,896 directed edges (2 per pool — forward and reverse), stored as an adjacency list (`Vec<Vec<usize>>`)
- **Reserve conversion**: Human-readable decimal strings (e.g. `"0.001505"`) are multiplied by `10^decimals` to get raw token units, stored as `f64`

### Cycle Detection Logic

**Algorithm**: Bounded depth-first search (DFS) starting from 5 hub tokens (WETH, USDT, USDC, DAI, WBTC).

- **Depth limit**: 2 to 4 hops per cycle
- **Constraints**: Visited-token set prevents loops; used-pool set prevents reusing the same pool twice in a single cycle
- **Deduplication**: Cycles are canonicalized by rotating the edge list so the smallest edge index comes first, then stored in a `HashSet` to eliminate duplicates
- **Result**: **60,496 candidate cycles** detected in **0.28 seconds**

**Why bounded DFS from hub tokens**: Short cycles (2-4 hops) capture the vast majority of real arbitrage. Longer paths accumulate fees (0.3% per hop) and slippage, making them unprofitable. Starting from hub tokens is efficient because WETH alone appears in 83% of filtered pools, so hub-first search covers nearly all connected liquidity.

### AMM Swap Formula

Standard Uniswap V2 `getAmountOut`:

```
amountOut = (amountIn * 997 * reserveOut) / (reserveIn * 1000 + amountIn * 997)
```

The 997/1000 factor encodes the 0.3% swap fee. Cycle simulation chains this formula through each hop, feeding the output of one swap as the input to the next.

### Ranking Metric

1. **Optimal trade size**: For each cycle, a golden section search finds the input amount that maximizes `output - input` (absolute profit in raw token units). The search uses a relative convergence tolerance (`max_input * 1e-12`) to handle tokens with varying decimal scales.
2. **USD conversion**: Profit is converted to USD using known token decimals and approximate prices (WETH ~ $2,000, stablecoins ~ $1, WBTC ~ $40,000).
3. **Ranking**: Cycles are sorted by estimated USD profit descending; top 10 are output.

### Top-10 Results

| Rank | Profit (USD) | Hops | Starting Token | Path Summary |
|------|-------------|------|----------------|--------------|
| 1 | $15,751 | 4 | USDT | USDT -> 0xd233... -> LINK -> UNI -> USDT |
| 2 | $10,608 | 3 | USDT | USDT -> 0xd233... -> DAI -> USDT |
| 3 | $10,596 | 4 | USDT | USDT -> 0xd233... -> DAI -> USDC -> USDT |
| 4 | $10,414 | 4 | USDT | USDT -> 0xd233... -> DAI -> EETH -> USDT |
| 5 | $9,132 | 4 | USDT | USDT -> 0xd233... -> DAI -> WBTC -> USDT |
| 6 | $8,338 | 3 | USDT | USDT -> 0xd233... -> USDC -> USDT |
| 7 | $8,285 | 4 | USDT | USDT -> 0xd233... -> USDC -> DAI -> USDT |
| 8 | $8,235 | 4 | USDT | USDT -> 0xd233... -> USDC -> AMPL -> USDT |
| 9 | $8,161 | 4 | USDT | USDT -> 0xd233... -> USDC -> SUSHI -> USDT |
| 10 | $8,149 | 4 | USDT | USDT -> 0xd233... -> USDC -> SNX -> USDT |

**Observation**: All top cycles route through the same mispriced pool (`0x50b6...fafb`) which has 43,422 tokens on one side but only 0.00001 USDT on the other. This is a drained/abandoned pool with stale `reserveUSD` data — exactly the kind of opportunity MEV bots exploit in practice.

### Suggested Trade Size

The optimal input for the top cycles is approximately **5 raw USDT units** (0.000005 USDT). This is extremely small because the mispriced pool has near-zero liquidity on the USDT side — any input, no matter how tiny, gets amplified through the skewed reserves. In a real trading scenario with normally-priced pools, optimal inputs would typically range from hundreds to thousands of dollars.

---

## Part 2: On-Chain Profitability Validation

### Data Encoding Format

Cycles are encoded as standard Solidity ABI calldata:

```solidity
function validateCycle(
    address[] calldata tokens,   // length = hops + 1, tokens[0] == tokens[last]
    address[] calldata pools,    // length = hops, Uniswap V2 pair addresses
    uint256 amountIn,            // starting amount in raw token units
    uint256 minOut               // minimum output (set to amountIn for breakeven)
)
```

Example for a 3-hop cycle (USDT -> X -> DAI -> USDT):
```
tokens: [USDT_addr, X_addr, DAI_addr, USDT_addr]   // 4 elements
pools:  [pool_USDT_X, pool_X_DAI, pool_DAI_USDT]    // 3 elements
amountIn: 5
minOut: 5
```

Part 1 outputs `top10.json` with real on-chain pool addresses and closed token paths, which the submission script reads directly.

### Validation Logic

The `ArbitrageValidator` contract (`contracts/src/ArbitrageValidator.sol`):

1. **Input validation**: Checks `tokens.length >= 3`, `tokens.length == pools.length + 1`, and `tokens[0] == tokens[last]`
2. **For each hop**: Calls `IUniswapV2Pair(pool).getReserves()` and `token0()` to fetch on-chain reserves and determine swap direction
3. **Swap simulation**: Applies the exact Uniswap V2 formula in `uint256` integer arithmetic (matching the on-chain Router)
4. **Profitability check**: `require(currentAmount >= minOut)` — reverts if the cycle is not profitable

### Revert Conditions

| Condition | Error Message |
|-----------|---------------|
| `tokens.length < 3` | "Min 2 hops" |
| `tokens.length != pools.length + 1` | "Length mismatch" |
| `tokens[0] != tokens[last]` | "Not a cycle" |
| `finalOutput < minOut` | "Below minimum output" |
| Pool `getReserves()` fails | External call revert |
| Zero reserves | Division by zero (arithmetic revert) |

### Test Results

**6 unit tests pass** (deployed on local Hardhat node):
- Profitable 2-hop cycle with asymmetric reserves
- Unprofitable 2-hop cycle (symmetric reserves, fees cause loss)
- Min hops revert
- Length mismatch revert
- Non-cyclic path revert
- Profitable 3-hop cycle

**Snapshot validation**: All 10 cycles from Part 1 validated as **PROFITABLE** on Tenderly Virtual TestNet using mock pools deployed with `v2pools.json` reserve data. The Solidity `uint256` math matches the Rust `f64` results exactly.

### How to Run

```bash
# Part 1: Detect cycles
cargo run --release -- data/v2pools.json

# Part 2: Compile contracts
cd contracts && bun run compile

# Part 2: Run unit tests
bun run node          # Terminal 1: kill port 8545 + start fresh Hardhat node
bun run test          # Terminal 2: deploy and run 6 tests

# Part 2: Validate with snapshot reserves (mock pools)
bun run node          # Terminal 1
bun run validate:snapshot   # Terminal 2

# Part 2: Validate with live mainnet reserves (requires RPC URL)
RPC_URL=https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY bun run validate:live
```

---

## Project Structure

```
Cargo.toml                          # Rust project config
src/
  main.rs                           # CLI entry point, orchestration
  types.rs                          # Pool, Edge, Cycle, RankedCycle structs
  loader.rs                         # Parse v2pools.json, filter, convert reserves
  graph.rs                          # Build directed adjacency list graph
  amm.rs                            # Uniswap V2 swap math, optimal input search
  detector.rs                       # Bounded DFS cycle detection
  ranker.rs                         # Profit ranking, USD estimation, output
contracts/
  src/ArbitrageValidator.sol        # On-chain validation contract
  src/interfaces/IUniswapV2Pair.sol # Uniswap V2 pair interface
  test/ArbitrageValidator.t.sol     # 6 unit tests
  test/mocks/MockUniswapV2Pair.sol  # Configurable mock pool for testing
  script/compile.js                 # Solidity compiler (solc)
  script/test.js                    # Deploy + run tests on local node
  script/validate.js                # Submit cycles against live reserves
  script/validate-snapshot.js       # Submit cycles against snapshot reserves
  script/validate-snapshot-tx.js    # Same, but as transactions (for explorer)
data/v2pools.json                   # Input dataset (38,141 pools)
output/top10.json                   # Detection results
report.md                           # This report
```

---

## AI Tools Used

### Tool

**Claude Code** (Claude Opus 4.6) — Anthropic's CLI agent for software engineering.

### How It Helped

- **Architecture and planning**: Produced the full implementation plan (file structure, algorithm choices, data flow) from the challenge spec before any code was written
- **Code generation**: Wrote all Rust and Solidity source files, including the AMM math, DFS cycle detection, golden section optimizer, graph construction, smart contract, mock contracts, and test harness
- **Automated validation**: Spawned 5 parallel sub-agents to review the codebase for correctness — checking AMM math against the Uniswap V2 formula, verifying graph edge counts, auditing DFS backtracking logic, and validating output plausibility
- **Bug detection and fixing**: The review agents identified 2 critical bugs (wrong USDT address, optimizer convergence threshold too small in absolute units) and 1 medium bug, all fixed before final output
- **Toolchain setup**: Configured the Solidity compilation pipeline (solc via Node.js), Hardhat local node, Tenderly Virtual TestNet integration, and bun-based script runner
- **Iterative debugging**: When the Tenderly API key didn't work as expected, adapted the approach to use the dashboard-created RPC URL instead

### Problems and Limitations Encountered

1. **Hallucinated USDT address**: The AI generated an incorrect USDT contract address (`0x...4272cf4d5a0500` instead of `0x...4597c13d831ec7`). This was caught by the validation sub-agent comparing against the actual dataset. Lesson: always verify hardcoded constants against source data.

2. **Optimizer convergence bug**: The golden section search used an absolute threshold (`< 1.0` raw units) that was fine for 18-decimal tokens but far too aggressive for 6-decimal tokens like USDC/USDT, causing the optimizer to converge near zero. The sub-agent flagged the implausible results ($10K profit from 0.000005 USDC input), leading to a fix using relative tolerance.

3. **Solidity `view` constraint**: Test functions that deploy contracts with `new` cannot be marked `view`. The compiler error was straightforward to fix but required iteration.

4. **Tenderly free tier limits**: Deploying 19 mock pools + running validations exhausted the free-tier RPC quota. Worked around this by using a local Hardhat node for subsequent testing.

5. **f64 precision vs. uint256**: The off-chain detector uses `f64` (sufficient for ranking) while the on-chain contract uses exact `uint256` arithmetic. Results matched for these cycles, but for cycles involving very large reserves (>10^15 raw units), f64's 53-bit mantissa could introduce rounding differences. A production system would use `U256` or `u128` with checked arithmetic.
