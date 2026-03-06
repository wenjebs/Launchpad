use crate::types::{Edge, Pool};
use std::collections::HashMap;

pub struct Graph {
    pub token_to_idx: HashMap<String, usize>,
    pub idx_to_token: Vec<String>,
    pub adjacency: Vec<Vec<usize>>, // token_idx -> list of edge indices
    pub edges: Vec<Edge>,
    pub pool_addresses: Vec<String>, // pool_idx -> on-chain pair address
}

pub fn build_graph(pools: &[Pool]) -> Graph {
    let mut token_to_idx: HashMap<String, usize> = HashMap::new();
    let mut idx_to_token: Vec<String> = Vec::new();

    let get_or_insert = |token: &str, map: &mut HashMap<String, usize>, names: &mut Vec<String>| -> usize {
        if let Some(&idx) = map.get(token) {
            idx
        } else {
            let idx = names.len();
            map.insert(token.to_string(), idx);
            names.push(token.to_string());
            idx
        }
    };

    // First pass: collect all tokens
    for pool in pools {
        get_or_insert(&pool.token0, &mut token_to_idx, &mut idx_to_token);
        get_or_insert(&pool.token1, &mut token_to_idx, &mut idx_to_token);
    }

    let num_tokens = idx_to_token.len();
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); num_tokens];
    let mut edges: Vec<Edge> = Vec::new();
    let pool_addresses: Vec<String> = pools.iter().map(|p| p.id.clone()).collect();

    for (pool_idx, pool) in pools.iter().enumerate() {
        let t0 = token_to_idx[&pool.token0];
        let t1 = token_to_idx[&pool.token1];

        // Forward: token0 -> token1
        let e0 = edges.len();
        edges.push(Edge {
            pool_idx,
            token_in: t0,
            token_out: t1,
            reserve_in: pool.reserve0,
            reserve_out: pool.reserve1,
        });
        adjacency[t0].push(e0);

        // Reverse: token1 -> token0
        let e1 = edges.len();
        edges.push(Edge {
            pool_idx,
            token_in: t1,
            token_out: t0,
            reserve_in: pool.reserve1,
            reserve_out: pool.reserve0,
        });
        adjacency[t1].push(e1);
    }

    eprintln!(
        "Graph: {} tokens, {} edges (from {} pools)",
        num_tokens,
        edges.len(),
        pools.len()
    );

    Graph {
        token_to_idx,
        idx_to_token,
        adjacency,
        edges,
        pool_addresses,
    }
}
