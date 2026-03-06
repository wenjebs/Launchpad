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
[loader.rs] ── parse JSON, convert reserves, filter reserveUSD < $1K
     │
     ▼  5,448 pools
[graph.rs] ── build directed adjacency list (2 edges per pool)
     │
     ▼  4,624 tokens, 10,896 edges
[detector.rs] ── bounded DFS from hub tokens, max 4 hops
     │
     ▼  60,496 candidate cycles
[ranker.rs] ── golden section search → optimal input → USD profit
     │
     ▼
output/top10.json + stdout table
```

## Graph construction

- **Nodes**: unique token addresses, indexed `0..N` via `HashMap<String, usize>`
- **Edges**: 2 per pool — forward (`token0 → token1`) and reverse (`token1 → token0`)
- **Reserves**: decimal strings like `"0.001505"` converted to raw units: `value * 10^decimals`
- **Filter**: pools with `reserveUSD < $1,000` or zero reserves are dropped (38,141 → 5,448)

## Cycle detection

**Algorithm**: bounded depth-first search from 5 hub tokens.

Hub tokens (cover >95% of liquidity):
- WETH `0xc02a...c756cc2` — 83% of pools
- USDT `0xdac1...31ec7`
- USDC `0xa0b8...06eb48`
- DAI `0x6b17...91d0f`
- WBTC `0x2260...c2c599`

**DFS rules**:
- Max depth: 4 hops
- `visited` set: prevents revisiting a token within the same path
- `used_pools` set: prevents using the same pool twice in one cycle
- Canonical deduplication: rotate edge list so smallest edge index is first, store in `HashSet`

**Why not Bellman-Ford / Floyd-Warshall?**
Those work with logarithmic edge weights, which require full matrix construction. Bounded DFS is simpler, naturally handles the pool-reuse constraint, and is fast enough at this scale (0.28s for 60K cycles).

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
  "profit_usd": 15751.45,
  "optimal_input_raw": 5,
  "profit_raw": 15751454646,
  "start_token": "0xdac17f...",
  "hops": 4,
  "path_tokens": ["0xdac1...", "0xd233...", "0x5149...", "0x1f98...", "0xdac1..."],
  "path_pools": ["0x50b6...", "0x81a8...", "0x9b26...", "0x5ac1..."]
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
