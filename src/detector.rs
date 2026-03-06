use crate::graph::Graph;
use crate::types::Cycle;
use std::collections::HashSet;

const MAX_HOPS: usize = 4;

/// Hub token addresses (lowercased)
const HUB_TOKENS: &[&str] = &[
    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2", // WETH
    "0xdac17f958d2ee523a2206206994597c13d831ec7", // USDT
    "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48", // USDC
    "0x6b175474e89094c44da98b954eedeac495271d0f", // DAI
    "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599", // WBTC
];

/// Detect arbitrage cycles using bounded DFS from hub tokens.
pub fn detect_cycles(graph: &Graph) -> Vec<Cycle> {
    let mut all_cycles: Vec<Cycle> = Vec::new();
    let mut seen_canonical: HashSet<Vec<usize>> = HashSet::new();

    for &hub in HUB_TOKENS {
        if let Some(&start_idx) = graph.token_to_idx.get(hub) {
            let mut visited = vec![false; graph.idx_to_token.len()];
            visited[start_idx] = true;

            let mut path_tokens = vec![start_idx];
            let mut path_edges: Vec<usize> = Vec::new();
            let mut used_pools: HashSet<usize> = HashSet::new();

            dfs(
                graph,
                start_idx,
                start_idx,
                &mut visited,
                &mut path_tokens,
                &mut path_edges,
                &mut used_pools,
                0,
                &mut all_cycles,
                &mut seen_canonical,
            );
        }
    }

    eprintln!("Detected {} candidate cycles", all_cycles.len());
    all_cycles
}

fn dfs(
    graph: &Graph,
    start: usize,
    current: usize,
    visited: &mut Vec<bool>,
    path_tokens: &mut Vec<usize>,
    path_edges: &mut Vec<usize>,
    used_pools: &mut HashSet<usize>,
    depth: usize,
    results: &mut Vec<Cycle>,
    seen: &mut HashSet<Vec<usize>>,
) {
    for &edge_idx in &graph.adjacency[current] {
        let edge = &graph.edges[edge_idx];
        let next = edge.token_out;

        // Don't reuse the same pool in a cycle
        if used_pools.contains(&edge.pool_idx) {
            continue;
        }

        if next == start && depth >= 1 {
            // Found a cycle back to start
            let mut cycle_pools: Vec<usize> = path_edges.clone();
            cycle_pools.push(edge_idx);

            // Canonical form: rotate so smallest pool index is first
            let canonical = canonical_form(&cycle_pools);
            if seen.insert(canonical) {
                results.push(Cycle {
                    edges: cycle_pools,
                    tokens: path_tokens.clone(),
                });
            }
        } else if !visited[next] && depth < MAX_HOPS - 1 {
            visited[next] = true;
            path_tokens.push(next);
            path_edges.push(edge_idx);
            used_pools.insert(edge.pool_idx);

            dfs(
                graph, start, next, visited, path_tokens, path_edges, used_pools,
                depth + 1, results, seen,
            );

            used_pools.remove(&edge.pool_idx);
            path_edges.pop();
            path_tokens.pop();
            visited[next] = false;
        }
    }
}

fn canonical_form(edge_indices: &[usize]) -> Vec<usize> {
    if edge_indices.is_empty() {
        return vec![];
    }
    // Use the rotation that starts with the smallest edge index
    let min_pos = edge_indices
        .iter()
        .enumerate()
        .min_by_key(|(_, &v)| v)
        .unwrap()
        .0;
    let mut canonical = Vec::with_capacity(edge_indices.len());
    for i in 0..edge_indices.len() {
        canonical.push(edge_indices[(min_pos + i) % edge_indices.len()]);
    }
    canonical
}
