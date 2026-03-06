# Part 1 — Off-Chain Cycle Detector

Rust CLI that finds profitable arbitrage cycles in a Uniswap V2 pool snapshot.

## Usage

```bash
# From project root
cargo run --release -- data/v2pools.json

# Or with the compiled binary
./target/release/dex-arb-detector data/v2pools.json
```

The data path defaults to `data/v2pools.json` if omitted.

## Pipeline

```
v2pools.json
     │
     ▼
[loader.rs] ── parse JSON, convert reserves, filter reserveUSD < $1K + dust reserves < 0.01
     │
     ▼  5,390 pools
[graph.rs] ── build directed adjacency list (2 edges per pool)
     │
     ▼  4,598 tokens, 10,780 edges
[detector.rs] ── Tarjan SCC pruning, anchor DFS from WETH, max 4 hops
     │
     ▼  53,104 candidate cycles
[ranker.rs] ── golden section search → optimal input → USD profit
     │
     ▼
output/top10.json + stdout table
```

## Graph construction

- **Nodes**: unique token addresses, indexed `0..N` via `HashMap<String, usize>`
- **Edges**: 2 per pool — forward (`token0 → token1`) and reverse (`token1 → token0`)
- **Reserves**: decimal strings like `"0.001505"` converted to raw units: `value * 10^decimals`
- **Filter**: pools are dropped if `reserveUSD < $1,000`, either raw reserve is zero, or either reserve in human-readable units is `< 0.01` (38,141 → 5,390)
- **Why the dust filter**: `reserveUSD` from the subgraph can be stale — a drained pool (e.g. one side = 0.00001 USDT) may still report a high `reserveUSD` from when it was healthy, producing phantom arbitrage opportunities with multi-billion-dollar implied prices. Checking the actual reserve values directly catches these.

## Cycle detection

**Algorithm**: SCC-pruned bounded depth-first search, anchored at WETH.

### Phase 1 — Strongly Connected Components (Tarjan's)

Before any DFS, `compute_sccs()` partitions all tokens into Strongly Connected Components using an iterative (stack-based) implementation of Tarjan's algorithm. This avoids recursion-limit issues on large graphs.

Result on the filtered dataset:
```
[SCC] 4598 tokens total | 16 non-trivial SCCs | largest: 4561 tokens
[SCC] WETH is in SCC #1 (4561 tokens) — using as anchor
[SCC] Skipping USDT/USDC/DAI/WBTC (same SCC as earlier hub)
```

### Phase 2 — Anchor selection

Hub tokens are checked against their SCC:
- Only one DFS sweep is started per distinct non-trivial SCC
- WETH anchors the main cluster (4,561 tokens — 99% of the graph)
- Tokens in singleton SCCs are skipped — they provably cannot participate in any cycle

On this dataset all five hub tokens (WETH, USDT, USDC, DAI, WBTC) fall in the same giant SCC, so only **1 DFS sweep** runs instead of 5.

### Phase 3 — SCC-restricted DFS

During DFS, any neighbor token not in the same SCC as the start token is pruned immediately:
```rust
if scc_ids[next] != start_scc {
    continue; // can't be part of a cycle through start
}
```

**Additional DFS rules**:
- Max depth: 4 hops
- `visited` set: prevents revisiting a token within the same path
- `used_pools` set: prevents using the same pool twice in one cycle
- Canonical deduplication: rotate edge list so smallest edge index is first, store in `HashSet`

**Why not Bellman-Ford / Floyd-Warshall?**
Those work with logarithmic edge weights and require full matrix construction. Bounded DFS is simpler, naturally handles the pool-reuse constraint, and with SCC pruning is fast enough at this scale (0.14s for 53K cycles).

## AMM math

Uniswap V2 `getAmountOut` with 0.3% fee:

```
amountOut = (amountIn × 997 × reserveOut) / (reserveIn × 1000 + amountIn × 997)
```

Cycle profit = `simulate(edges, amountIn) - amountIn`

Optimal input found via **golden section search** on `[0, 0.5 × min(reserve_in)]` with relative convergence tolerance `max_input × 1e-12`.

## Output format

`output/top10.json`:
```json
{
  "rank": 1,
  "profit_usd": 2752.65,
  "optimal_input_raw": 3639803600,
  "profit_raw": 2752652737,
  "start_token": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
  "hops": 4,
  "path_tokens": ["0xa0b8...eb48", "0xd233...29dc", "0x5149...86ca", "0x1f98...f984", "0xa0b8...eb48"],
  "path_pools": ["0x...", "0x...", "0x...", "0x..."]
}
```

- `path_tokens`: closed cycle — first and last element are the same token
- `path_pools`: real Uniswap V2 pair contract addresses (used directly by Part 2)
- `optimal_input_raw`: raw token units (divide by `10^decimals` for human-readable amount)

## Running tests

```bash
cargo test
```

Two unit tests in `src/amm.rs` verify the swap formula against known values.
