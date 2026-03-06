# Cycle Detection Algorithm Specification

## Problem Statement

Find all profitable arbitrage cycles in the token swap graph, where a cycle `A -> B -> ... -> A` yields `amountOut > amountIn` after simulating swaps through each pool.

## Approach: Hybrid (Bellman-Ford + DFS Enumeration)

### Phase 1: Fast Screening with Bellman-Ford

Use negative-log edge weights to quickly identify if profitable cycles exist.

**Edge weight computation:**
```
For edge (u -> v) with reserveIn, reserveOut, fee_bps:
    r = (10000 - fee_bps) / 10000.0
    spot_rate = (r * reserveOut) / reserveIn
    weight = -ln(spot_rate)
```

**Algorithm:**
```
for each node s in graph:
    result = bellman_ford(graph, s)
    if result contains negative cycle:
        extract cycle nodes
        add to candidate set
```

**Complexity:** O(|V| * |V| * |E|) worst case, but early termination helps.

**Limitation:** Bellman-Ford finds ONE cycle per source node, and spot-rate weights are an approximation (they ignore price impact from trade size). Use this phase only for fast candidate identification.

### Phase 2: Bounded DFS Enumeration

Enumerate all simple cycles up to a maximum length (recommended: 2-4 hops).

**Algorithm:**
```
MAX_HOPS = 4
candidates = []

for each starting node s:
    dfs(path=[s], current=s, depth=0):
        if depth > 0 and current == s:
            candidates.append(path)
            return
        if depth >= MAX_HOPS:
            return
        for each neighbor n of current:
            if n == s or n not in path[1:]:  # allow returning to start
                dfs(path + [n], n, depth + 1)
```

**Optimizations:**
- Skip edges with zero reserves
- Prune paths where intermediate output drops below a minimum threshold
- Start DFS only from "hub" tokens (high degree nodes like WETH, USDC, USDT)
- Early termination: if simulated amount at any hop is < amountIn * 0.5, prune

**Complexity:** O(|V| * d^MAX_HOPS) where d = average node degree. Manageable for MAX_HOPS <= 4.

### Why both phases?

| Phase | Pros | Cons |
|-------|------|------|
| Bellman-Ford | Fast, guaranteed to find a cycle if one exists | One cycle per source, uses spot rate approximation |
| Bounded DFS | Finds ALL short cycles, uses exact simulation | Exponential in MAX_HOPS |

Use Bellman-Ford to validate that opportunities exist, then DFS to exhaustively enumerate them.

## Cycle Representation

```rust
struct ArbitrageCycle {
    /// Token addresses in order (last == first)
    tokens: Vec<usize>,
    /// Pool edges used at each hop
    edges: Vec<usize>,
    /// Number of hops (excluding return to start)
    hops: usize,
}
```

## Deduplication

Same cycle can be found starting from different nodes: `A->B->C->A` == `B->C->A->B`.

Canonical form: rotate cycle so the smallest token index is first.

```rust
fn canonicalize(cycle: &[usize]) -> Vec<usize> {
    let min_pos = cycle.iter().position(|&x| x == *cycle.iter().min().unwrap()).unwrap();
    let mut canonical = cycle[min_pos..].to_vec();
    canonical.extend_from_slice(&cycle[..min_pos]);
    canonical
}
```

Store canonical forms in a `HashSet` to deduplicate.

## Expected Scale

For a dataset of ~1000 pools:
- ~500-2000 unique tokens
- ~2000 directed edges
- Cycles of length 2: rare (requires 2 pools for same pair with rate imbalance)
- Cycles of length 3: most common arbitrage pattern
- Cycles of length 4: less common but possible

For ~10,000 pools the DFS approach needs the pruning optimizations above to stay tractable.
