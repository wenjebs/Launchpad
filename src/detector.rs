use crate::graph::Graph;
use crate::types::Cycle;
use std::collections::{HashMap, HashSet};

const MAX_HOPS: usize = 4;

const WETH: &str = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";

/// Hub token addresses (lowercased), WETH first
const HUB_TOKENS: &[(&str, &str)] = &[
    (WETH, "WETH"),
    ("0xdac17f958d2ee523a2206206994597c13d831ec7", "USDT"),
    ("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48", "USDC"),
    ("0x6b175474e89094c44da98b954eedeac495271d0f", "DAI"),
    ("0x2260fac5e5542a773aa44fbcfedf7c193bc2c599", "WBTC"),
];

/// Compute Strongly Connected Components using Tarjan's iterative algorithm.
/// Returns `scc_id[token_idx]` for each token (IDs start at 1).
fn compute_sccs(graph: &Graph) -> Vec<usize> {
    let n = graph.idx_to_token.len();
    const UNDEFINED: usize = usize::MAX;

    let mut index = vec![UNDEFINED; n];
    let mut lowlink = vec![0usize; n];
    let mut on_stack = vec![false; n];
    let mut tarjan_stack: Vec<usize> = Vec::new();
    let mut scc_id = vec![0usize; n];

    let mut index_counter: usize = 0;
    let mut next_scc: usize = 1;

    // call_stack: (node, index into adjacency[node])
    let mut call_stack: Vec<(usize, usize)> = Vec::new();

    for start in 0..n {
        if index[start] != UNDEFINED {
            continue;
        }

        index[start] = index_counter;
        lowlink[start] = index_counter;
        index_counter += 1;
        on_stack[start] = true;
        tarjan_stack.push(start);
        call_stack.push((start, 0));

        while !call_stack.is_empty() {
            let v = call_stack.last().unwrap().0;
            let i = call_stack.last().unwrap().1;

            if i < graph.adjacency[v].len() {
                let edge_idx = graph.adjacency[v][i];
                call_stack.last_mut().unwrap().1 += 1;
                let w = graph.edges[edge_idx].token_out;

                if index[w] == UNDEFINED {
                    index[w] = index_counter;
                    lowlink[w] = index_counter;
                    index_counter += 1;
                    on_stack[w] = true;
                    tarjan_stack.push(w);
                    call_stack.push((w, 0));
                } else if on_stack[w] && index[w] < lowlink[v] {
                    lowlink[v] = index[w];
                }
            } else {
                call_stack.pop();

                // Propagate lowlink to parent
                if let Some(&(parent, _)) = call_stack.last() {
                    if lowlink[v] < lowlink[parent] {
                        lowlink[parent] = lowlink[v];
                    }
                }

                // If v is root of an SCC, pop members off tarjan_stack
                if lowlink[v] == index[v] {
                    let current_scc = next_scc;
                    next_scc += 1;
                    loop {
                        let w = tarjan_stack.pop().unwrap();
                        on_stack[w] = false;
                        scc_id[w] = current_scc;
                        if w == v {
                            break;
                        }
                    }
                }
            }
        }
    }

    scc_id
}

/// Detect arbitrage cycles using bounded DFS from hub tokens, pruned by SCC membership.
/// `anchor` is an optional token name (e.g. "USDT") to use as the primary starting hub.
pub fn detect_cycles(graph: &Graph, anchor: Option<&str>) -> Vec<Cycle> {
    let scc_ids = compute_sccs(graph);
    let n = graph.idx_to_token.len();

    // Count members per SCC
    let mut scc_sizes: HashMap<usize, usize> = HashMap::new();
    for &id in &scc_ids {
        *scc_sizes.entry(id).or_insert(0) += 1;
    }

    let non_trivial: Vec<usize> = scc_sizes
        .iter()
        .filter(|(_, &sz)| sz >= 2)
        .map(|(&id, _)| id)
        .collect();
    let largest_scc = scc_sizes.values().copied().max().unwrap_or(0);

    eprintln!(
        "[SCC] {} tokens total | {} non-trivial SCCs | largest: {} tokens",
        n,
        non_trivial.len(),
        largest_scc
    );

    // Determine which hub tokens to run DFS from
    let mut seen_sccs: HashSet<usize> = HashSet::new();
    let mut active_hubs: Vec<(usize, usize, &str)> = Vec::new(); // (token_idx, scc_id, name)

    // Reorder so the chosen anchor comes first
    let mut ordered_hubs: Vec<(&str, &str)> = HUB_TOKENS.to_vec();
    if let Some(name) = anchor {
        if let Some(pos) = ordered_hubs.iter().position(|&(_, n)| n.eq_ignore_ascii_case(name)) {
            let hub = ordered_hubs.remove(pos);
            ordered_hubs.insert(0, hub);
        } else {
            eprintln!("[Config] Unknown anchor '{}', falling back to WETH", name);
        }
    }
    let anchor_name = ordered_hubs[0].1;

    for &(addr, name) in &ordered_hubs {
        if let Some(&token_idx) = graph.token_to_idx.get(addr) {
            let scc = scc_ids[token_idx];
            let size = scc_sizes.get(&scc).copied().unwrap_or(0);
            if size < 2 {
                eprintln!("[SCC] Skipping {} (singleton SCC)", name);
                continue;
            }
            if seen_sccs.insert(scc) {
                let anchor_note = if name == anchor_name { " — using as anchor" } else { "" };
                eprintln!(
                    "[SCC] {} is in SCC #{} ({} tokens){}",
                    name, scc, size, anchor_note
                );
                active_hubs.push((token_idx, scc, name));
            } else {
                eprintln!("[SCC] Skipping {} (same SCC as earlier hub)", name);
            }
        } else {
            eprintln!("[SCC] Skipping {} (not in graph)", name);
        }
    }

    let mut all_cycles: Vec<Cycle> = Vec::new();
    let mut seen_canonical: HashSet<Vec<usize>> = HashSet::new();

    for (start_idx, start_scc, _name) in active_hubs {
        let mut visited = vec![false; n];
        visited[start_idx] = true;

        let mut path_tokens = vec![start_idx];
        let mut path_edges: Vec<usize> = Vec::new();
        let mut used_pools: HashSet<usize> = HashSet::new();

        dfs(
            graph,
            start_idx,
            start_idx,
            start_scc,
            &scc_ids,
            &mut visited,
            &mut path_tokens,
            &mut path_edges,
            &mut used_pools,
            0,
            &mut all_cycles,
            &mut seen_canonical,
        );
    }

    eprintln!("Detected {} candidate cycles", all_cycles.len());
    all_cycles
}

fn dfs(
    graph: &Graph,
    start: usize,
    current: usize,
    start_scc: usize,
    scc_ids: &[usize],
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
            // Prune: skip tokens outside the starting SCC
            if scc_ids[next] != start_scc {
                continue;
            }

            visited[next] = true;
            path_tokens.push(next);
            path_edges.push(edge_idx);
            used_pools.insert(edge.pool_idx);

            dfs(
                graph, start, next, start_scc, scc_ids, visited, path_tokens,
                path_edges, used_pools, depth + 1, results, seen,
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
