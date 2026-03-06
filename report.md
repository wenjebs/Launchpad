# DEX Arbitrage Detector — Final Report

## Part 1: Off-Chain Cycle Detection

### Graph Construction

- **Input**: 38,141 Uniswap V2 pool snapshots from `data/v2pools.json`
- **Filtering**: Pools are discarded if `reserveUSD < $1,000`, either raw reserve is zero, or either reserve in human-readable units is `< 0.01`, reducing to **5,390 pools**
- **Nodes**: 4,598 unique token addresses, indexed via `HashMap<String, usize>`
- **Edges**: 10,780 directed edges (2 per pool — forward and reverse), stored as an adjacency list (`Vec<Vec<usize>>`)
- **Reserve conversion**: Human-readable decimal strings (e.g. `"0.001505"`) are multiplied by `10^decimals` to get raw token units, stored as `f64`

### Cycle Detection Logic

**Algorithm**: SCC-pruned bounded DFS, anchored at WETH.

**Phase 1 — Strongly Connected Components**: Before any DFS, `compute_sccs()` runs Tarjan's iterative SCC algorithm over all 4,598 tokens. This identifies 16 non-trivial SCCs; the largest contains 4,561 tokens (99% of the graph) and includes all five hub tokens.

**Phase 2 — Anchor selection**: Instead of searching for cycles starting from every token, the detector picks a single fixed start/end point — configurable via `--anchor` (default: WETH). Since WETH appears in 83% of filtered pools, almost every profitable loop in the graph will pass through it. This means only **1 search sweep** is needed instead of one per hub token, while still finding virtually all meaningful cycles. All five hubs share the same giant SCC, so any of them can serve as anchor — the choice affects which cycles surface in the top 10, not overall completeness.

**Phase 3 — SCC-restricted DFS**: During traversal, any neighbor not in the same SCC as the start token is pruned immediately — it provably cannot be part of a cycle through the start node. Combined with the standard constraints below, this eliminates a large fraction of dead-end traversals:

- **Depth limit**: 2 to 4 hops per cycle
- **Visited-token set**: prevents revisiting a token within the same path
- **Used-pool set**: prevents reusing the same pool twice in a single cycle
- **Deduplication**: cycles are canonicalized by rotating the edge list so the smallest edge index comes first, stored in a `HashSet`

- **Result**: **53,104 candidate cycles** detected in **0.14 seconds**

### AMM Swap Formula

Standard Uniswap V2 `getAmountOut`:

```
amountOut = (amountIn * 997 * reserveOut) / (reserveIn * 1000 + amountIn * 997)
```

The 997/1000 factor encodes the 0.3% swap fee. Cycle simulation chains this formula through each hop, feeding the output of one swap as the input to the next.

### Ranking Metric

1. **Optimal trade size**: For each cycle, a golden section search finds the input amount that maximizes `output - input` (absolute profit in raw token units). The search range is `[0, 0.3 × first_edge.reserve_in]` — using the first edge's reserve (which is in the starting token's own raw units) as the upper bound. The search uses a relative convergence tolerance (`max_input * 1e-12`) to handle tokens with varying decimal scales.
2. **USD conversion**: Profit is converted to USD using known token decimals and approximate prices (WETH ~ $2,000, stablecoins ~ $1, WBTC ~ $40,000).
3. **Ranking**: Cycles are sorted by estimated USD profit descending; top 10 are output.

### Top-10 Results

| Rank | Profit (USD) | Hops | Starting Token | Path Summary |
|------|-------------|------|----------------|--------------|
| 1 | $71,805 | 3 | WETH | WETH → DAI → 0xd233... → WETH |
| 2 | $71,416 | 4 | WETH | WETH → USDT → DAI → 0xd233... → WETH |
| 3 | $71,413 | 4 | WETH | WETH → USDC → DAI → 0xd233... → WETH |
| 4 | $70,820 | 4 | WETH | WETH → 0x03ab... → DAI → 0xd233... → WETH |
| 5 | $70,166 | 3 | WETH | WETH → 0xd46b... → 0xd233... → WETH |
| 6 | $68,588 | 4 | WETH | WETH → 0xeef9... → DAI → 0xd233... → WETH |
| 7 | $68,249 | 4 | WETH | WETH → 0xa3be... → DAI → 0xd233... → WETH |
| 8 | $66,842 | 4 | WETH | WETH → 0x1337... → DAI → 0xd233... → WETH |
| 9 | $65,930 | 4 | WETH | WETH → WBTC → DAI → 0xd233... → WETH |
| 10 | $57,787 | 4 | WETH | WETH → 0x15f0... → DAI → 0xd233... → WETH |

All top cycles route through token `0xd233d1f6fd...`. These are phantom profits from stale snapshot data — see findings below.

**Finding 1 — stale `reserveUSD` and drained pools**: Initial results showed all top cycles routing through pool `0x50b6...fafb`, with apparent profits of $8,000–$15,000. Investigation revealed this pool had 43,422 tokens on one side but only 0.00001 USDT on the other — effectively drained — yet its `reserveUSD` field still read $56,992 from a stale subgraph snapshot. The AMM formula computed an implied price of ~4.3 billion USDT per token, producing phantom profit for any input amount. A filter was added in `loader.rs` to reject pools where either reserve in human-readable units is below 0.01, regardless of `reserveUSD`.

**Finding 2 — cross-pool price inconsistency**: Adding `--anchor` support and running experiments across all five hub tokens revealed a deeper data quality issue. Token `0xd233d1f6fd...` (9 decimals) is priced at **$1.07** in the DAI/0xd233 pool but **$5.71** in the WETH/0xd233 pool — a 5.4× discrepancy. Both pools have healthy reserves on both sides and pass all filters. The difference is that the two pools were captured at different block times from the subgraph, so their prices are inconsistent. The AMM formula correctly computes a 653% round-trip profit given these reserves, but no such profit exists on a live chain. This type of phantom cannot be caught by reserve-size filters; it would require a cross-pool price consistency check (e.g. reject any token whose implied price varies more than 2× across its pools).

**Finding 4 — profit cliff between rank 2 and rank 3**: The top-10 results show a sharp drop-off after rank 2 ($93k → $50k → $64 → ...). Ranks 1 and 2 both route through `0xd233d1f6fd...` and inherit the full 5.4× phantom price discrepancy. Rank 3 onward routes through different tokens with no stale price mismatch, so profits reflect genuine (tiny) reserve imbalances in the snapshot — in the $15–$65 range. The cliff is a direct fingerprint of the stale snapshot: exactly the cycles that exploit `0xd233...` are vastly inflated, and everything else is negligible.

**Finding 3 — optimizer decimal mismatch bug**: The original golden section search used `min(reserve_in across all edges)` as the upper bound. Since reserves are stored in raw units (each multiplied by its token's decimals), comparing them across different tokens is meaningless. For WETH→DAI→0xd233→WETH, the 0xd233 side has raw reserve 679,288 × 10⁹ ≈ 6.8×10¹⁴, which treated as WETH (18 decimals) = 0.00034 WETH — 58,885× below the actual optimal of ~20 WETH. This bug made WETH anchor appear to produce small realistic profits ($1,911) while stablecoin anchors surfaced $100K+ phantoms. The asymmetry was entirely caused by the mismatch, not real differences between anchors. Fixed by using only the first edge's `reserve_in` as the search bound, which is always in the starting token's own units.

### Suggested Trade Size

After the optimizer fix, optimal inputs for the top WETH cycles range from ~7.9×10¹⁸ (rank 5, ~7.9 WETH via the AMPL pool) to ~2.1×10¹⁹ (rank 1, ~21 WETH). These are all phantom opportunities, so the trade sizes are academic. In a production system the optimizer bound should also account for downstream pool depth — a cycle may be technically profitable at 20 WETH in but practically constrained by thin liquidity in intermediate pools.

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

**Snapshot validation**: All 10 cycles from Part 1 validated as **PROFITABLE** on local Hardhat/Anvil node using mock pools deployed with `v2pools.json` reserve data. The Solidity `uint256` math matches the Rust `f64` results exactly.

| Rank | Status | AmountIn (raw) | ActualOut (raw) | Profit (raw) | Hops | Profit USD |
|------|--------|---------------|-----------------|--------------|------|------------|
| 1 | PROFITABLE | 134,028,387 | 367,324,889 | 233,296,502 | 4 | $93,318.60 |
| 2 | PROFITABLE | 73,060,640 | 198,047,213 | 124,986,573 | 4 | $49,994.63 |
| 3 | PROFITABLE | 2,831,294 | 2,991,415 | 160,121 | 4 | $64.05 |
| 4 | PROFITABLE | 484,476 | 604,461 | 119,985 | 4 | $47.99 |
| 5 | PROFITABLE | 1,166,126 | 1,280,608 | 114,482 | 4 | $45.79 |
| 6 | PROFITABLE | 655,681 | 701,872 | 46,191 | 4 | $18.48 |
| 7 | PROFITABLE | 340,289 | 381,783 | 41,494 | 4 | $16.60 |
| 8 | PROFITABLE | 358,744 | 397,848 | 39,104 | 4 | $15.64 |
| 9 | PROFITABLE | 350,160 | 389,010 | 38,850 | 4 | $15.54 |
| 10 | PROFITABLE | 1,116,463 | 1,153,982 | 37,519 | 4 | $15.01 |

### How to Run


https://github.com/user-attachments/assets/0512c3d2-fd70-4bbf-a13a-9ad2aab12182


```bash
# Part 1: Detect cycles (default anchor: WETH)
cargo run --release -- data/v2pools.json

# Part 1: Detect cycles with a different anchor token 
cargo run --release -- data/v2pools.json --anchor USDT

# Part 2: Compile contracts
cd contracts && bun run compile

# Part 2: Run unit tests
bun run node          # Terminal 1: kill port 8545 + start fresh Hardhat node
bun run test          # Terminal 2: deploy and run 6 tests

# Part 2: Validate with snapshot reserves (mock pools)
bun run node          # Terminal 1
or 
anvil --steps-tracing --host 0.0.0.0 --port 8545 # require foundry

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

Claude Code drove the entire project end-to-end. The human role was steering — providing the spec, answering clarifying questions, and making judgment calls on direction. Every implementation artifact was produced by the AI.

- **Brainstorming and architecture**: Before any code was written, produced the full implementation plan — file structure, algorithm choices, data flow — by reasoning through the challenge spec
- **Code generation**: Wrote all Rust and Solidity source files, including the AMM math, DFS cycle detection, golden section optimizer, graph construction, smart contract, mock contracts, and test harness
- **Automated validation**: Spawned 5 parallel sub-agents to review the codebase for correctness — checking AMM math against the Uniswap V2 formula, verifying graph edge counts, auditing DFS backtracking logic, and validating output plausibility
- **Bug detection and fixing**: The review agents identified 2 critical bugs (wrong USDT address, optimizer convergence threshold too small in absolute units) and 1 medium bug, all fixed before final output
- **Toolchain setup**: Configured the Solidity compilation pipeline (solc via Node.js), Hardhat local node, Tenderly Virtual TestNet integration, and bun-based script runner
- **Iterative debugging**: When the Tenderly API key didn't work as expected, adapted the approach to use the dashboard-created RPC URL instead
- **Version control**: Authored all commits and pushed all branches

### Problems and Limitations Encountered

1. **Hallucinated USDT address**: The AI generated an incorrect USDT contract address (`0x...4272cf4d5a0500` instead of `0x...4597c13d831ec7`). This was caught by the validation sub-agent comparing against the actual dataset. Lesson: always verify hardcoded constants against source data.

2. **Optimizer convergence bug**: The golden section search used an absolute threshold (`< 1.0` raw units) that was fine for 18-decimal tokens but far too aggressive for 6-decimal tokens like USDC/USDT, causing the optimizer to converge near zero. The sub-agent flagged the implausible results ($10K profit from 0.000005 USDC input), leading to a fix using relative tolerance.

3. **Stale `reserveUSD` in subgraph data**: The dataset contains abandoned/drained pools where one reserve side is essentially zero but `reserveUSD` remains at its historical peak. Filtering on `reserveUSD >= $1,000` alone let these through, and the skewed reserves produced phantom arbitrage opportunities with implied prices in the billions. Fixed by adding a direct check that both reserves in human-readable units are at least 0.01.

4. **Solidity `view` constraint**: Test functions that deploy contracts with `new` cannot be marked `view`. The compiler error was straightforward to fix but required iteration.

5. **Tenderly free tier limits**: Deploying 19 mock pools + running validations exhausted the free-tier RPC quota. Worked around this by using a local Hardhat node for subsequent testing.

6. **f64 precision vs. uint256**: The off-chain detector uses `f64` (sufficient for ranking) while the on-chain contract uses exact `uint256` arithmetic. Results matched for these cycles, but for cycles involving very large reserves (>10^15 raw units), f64's 53-bit mantissa could introduce rounding differences. A production system would use `U256` or `u128` with checked arithmetic.

7. **Optimizer decimal mismatch bug (found and fixed)**: The golden section search originally used `min(reserve_in across all edges)` as the search upper bound. Because reserves are stored in raw units per each token's decimal count, this compared numbers across incompatible scales — e.g. a 9-decimal token's reserve appeared ~1 billion times smaller than an 18-decimal token's reserve. For some cycles this capped the search 50,000× below the true optimum, making certain phantom profits invisible from some anchor tokens but visible from others. Fixed by using the first edge's `reserve_in` only, which is always in the starting token's units.

8. **Cross-pool price inconsistency not filtered**: The dust-reserve filter catches drained pools (near-zero reserves on one side) but not pools with healthy reserves that were snapshotted at different block times. Token `0xd233d1f6fd...` has a 5.4× price difference between its DAI pool and its WETH pool purely because of snapshot timing. Both pools pass every filter, but any cycle through them produces a phantom profit. Filtering this out without external price data would require comparing implied prices across pools for the same token and rejecting outliers.

9. **Domain unfamiliarity and blind steering**: I have no prior background in crypto, DeFi, or Web3. Concepts like AMM mechanics, liquidity pools, reserve ratios, and on-chain validation were entirely new. Understanding what the system was actually doing — not just implementing it — was genuinely difficult. I largely trusted the AI's explanations and verified correctness through tests and output plausibility rather than independent domain knowledge. This made steering unreliable: when the AI proposed an approach, I had no independent basis to evaluate it. I could ask clarifying questions, but I couldn't tell if the questions themselves were the right ones. The human retains control in theory, but in an unfamiliar domain the AI's framing shapes every decision.
