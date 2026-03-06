# Graph Construction Specification

## Data Model

### Input: Pool Data (v2pools.json — 38,141 pools)

Actual schema per pool entry:

```json
{
  "id": "0x0006bc3e52137a1873d7d8cd779a7e138bb7e929",
  "reserve0": "0.000000000000020216",
  "reserve1": "0.00000000000000005",
  "reserveETH": "0.00000000000000009999...",
  "reserveUSD": "0.00000000000006271...",
  "token0": {
    "decimals": "18",
    "id": "0x1828e7a548bbd4a90ac74e0b411503512dd25268"
  },
  "token0Price": "404.32",
  "token1": {
    "decimals": "18",
    "id": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
  },
  "token1Price": "0.002473288484368816778789077958053027"
}
```

Key observations:
- **Reserves are decimal strings** (human-readable, NOT raw integers). Must convert to raw:
  `raw_reserve = parse_decimal(reserve_str) * 10^decimals`
- **No fee field** — Uniswap V2 fee is hardcoded at 0.3% (997/1000)
- **No token symbols** — only addresses. Can hardcode known addresses (WETH=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2)
- **`decimals` is a string** — parse to u8
- **`reserveETH`/`reserveUSD`** — useful for liquidity filtering
- Many pools have dust-level reserves — filter aggressively

### Graph Representation

**Directed weighted graph** using adjacency list:

- **Nodes**: Unique token addresses
- **Edges**: Directed swap opportunities (each pool creates 2 directed edges)

For each pool with `(token0, token1, reserve0, reserve1, fee)`:
- Edge `token0 -> token1` with metadata `{pool_address, reserveIn=reserve0, reserveOut=reserve1, fee}`
- Edge `token1 -> token0` with metadata `{pool_address, reserveIn=reserve1, reserveOut=reserve0, fee}`

### Edge Weight Strategies

#### Strategy A: Log-transformed weights (for Bellman-Ford)

Convert multiplicative exchange rates to additive weights using negative log:

```
spot_rate = (r * reserveOut) / reserveIn    # where r = fee factor
weight = -ln(spot_rate)
```

A negative-weight cycle in this graph corresponds to a profitable arbitrage:

```
sum of weights < 0  =>  product of rates > 1  =>  profit
```

#### Strategy B: Raw reserves (for simulation-based approach)

Store reserves directly on edges. Run the actual AMM formula during cycle evaluation rather than relying on linear approximation.

**Recommendation**: Use Strategy B for ranking accuracy, Strategy A for fast cycle detection.

## Data Structures (Rust)

```rust
struct Token {
    address: String,
    symbol: String,
    decimals: u8,
}

struct PoolEdge {
    pool_address: String,
    token_in: usize,      // node index
    token_out: usize,     // node index
    reserve_in: U256,     // use ethnum or ruint
    reserve_out: U256,
    fee_bps: u16,         // typically 30
}

struct SwapGraph {
    tokens: Vec<Token>,                        // indexed by node ID
    token_index: HashMap<String, usize>,       // address -> node ID
    adjacency: Vec<Vec<usize>>,                // node -> [edge indices]
    edges: Vec<PoolEdge>,
}
```

## Construction Algorithm

```
1. Parse JSON pool data
2. For each pool:
   a. Get or create node index for token0 and token1
   b. Skip if reserve0 == 0 or reserve1 == 0
   c. Create forward edge (token0 -> token1)
   d. Create reverse edge (token1 -> token0)
   e. Add edge indices to adjacency lists
3. Log graph stats: |V| nodes, |E| edges
```

## Filtering and Pruning

Consider filtering pools to reduce graph size:

| Filter | Rationale |
|--------|-----------|
| Min liquidity threshold | Skip dust pools with tiny reserves |
| Known stablecoins / major tokens | Focus on high-liquidity paths |
| Remove self-loops | Same token on both sides is invalid |
| Deduplicate by pair | Keep only the highest-liquidity pool per pair (optional) |

## Petgraph Integration

If using `petgraph`:

```rust
use petgraph::graph::DiGraph;

// Node weight = token index, Edge weight = PoolEdge or -ln(rate)
let mut graph = DiGraph::<usize, f64>::new();
```

For Bellman-Ford negative cycle detection:
```rust
use petgraph::algo::bellman_ford::find_negative_cycle;
```

Note: `petgraph::find_negative_cycle` returns a single cycle from a source node. Must iterate over all nodes as source to find all cycles, or use a custom implementation.
