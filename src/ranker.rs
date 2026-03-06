use crate::amm;
use crate::graph::Graph;
use crate::types::{Cycle, RankedCycle};

/// Rank cycles by profit. Returns top N.
pub fn rank_cycles(graph: &Graph, cycles: Vec<Cycle>, top_n: usize) -> Vec<RankedCycle> {
    let mut ranked: Vec<RankedCycle> = Vec::new();

    for cycle in cycles {
        let (optimal_input, output) = amm::optimal_input(graph, &cycle);
        let profit = output - optimal_input;

        if profit <= 0.0 || optimal_input <= 0.0 {
            continue;
        }

        let start_token_idx = cycle.tokens[0];
        let start_token = graph.idx_to_token[start_token_idx].clone();

        // Estimate USD value of profit using the first edge's pool reserves
        let profit_usd = estimate_profit_usd(graph, &cycle, profit);

        ranked.push(RankedCycle {
            cycle,
            optimal_input,
            profit,
            profit_usd,
            start_token,
        });
    }

    // Sort by profit descending (in raw token units — we'll use USD estimate)
    ranked.sort_by(|a, b| b.profit_usd.partial_cmp(&a.profit_usd).unwrap());
    ranked.truncate(top_n);
    ranked
}

fn estimate_profit_usd(graph: &Graph, cycle: &Cycle, profit_raw: f64) -> f64 {
    // Find the first edge and estimate token price from reserves
    // This is a rough estimate — the starting token's USD price
    // We use the first pool's data to estimate
    let first_edge = &graph.edges[cycle.edges[0]];

    // We need pool-level reserveUSD info. Since we store reserves in raw units,
    // and we know the starting token, we can estimate:
    // token_price ≈ (pool_reserveUSD / 2) / reserve_of_start_token
    // But we don't have reserveUSD on edges. Instead, use WETH price as anchor.

    // Simple heuristic: if start token is WETH, 1 WETH ≈ $2000 (approximate)
    // For other tokens, use the first edge's reserves to estimate
    let start_token = &graph.idx_to_token[cycle.tokens[0]];
    if start_token == "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2" {
        // WETH: profit is in raw units (wei), convert to ETH then USD
        profit_raw / 1e18 * 2000.0
    } else if start_token == "0xdac17f958d2ee523a2206206994597c13d831ec7"
        || start_token == "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    {
        // USDT/USDC (6 decimals)
        profit_raw / 1e6
    } else if start_token == "0x6b175474e89094c44da98b954eedeac495271d0f" {
        // DAI (18 decimals)
        profit_raw / 1e18
    } else if start_token == "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599" {
        // WBTC (8 decimals) ~$40000
        profit_raw / 1e8 * 40000.0
    } else {
        // Unknown token — rough estimate using first edge reserves
        // Assume the total reserve value is split equally
        profit_raw / first_edge.reserve_in * 1000.0 // very rough
    }
}

pub fn print_results(ranked: &[RankedCycle], graph: &Graph) {
    println!("\n{:=<100}", "");
    println!("  TOP {} ARBITRAGE OPPORTUNITIES", ranked.len());
    println!("{:=<100}\n", "");

    println!(
        "{:<4} {:<12} {:<18} {:<18} {:<6} {}",
        "Rank", "Profit USD", "Optimal Input", "Profit (raw)", "Hops", "Path"
    );
    println!("{:-<100}", "");

    for (i, r) in ranked.iter().enumerate() {
        let path = format_path(graph, &r.cycle);
        println!(
            "{:<4} ${:<11.2} {:<18.2} {:<18.2} {:<6} {}",
            i + 1,
            r.profit_usd,
            r.optimal_input,
            r.profit,
            r.cycle.edges.len(),
            path
        );
    }
    println!();
}

fn format_path(graph: &Graph, cycle: &Cycle) -> String {
    let mut parts: Vec<String> = cycle
        .tokens
        .iter()
        .map(|&t| shorten_address(&graph.idx_to_token[t]))
        .collect();
    // Add the start token at the end to show the cycle
    if let Some(&first) = cycle.tokens.first() {
        parts.push(shorten_address(&graph.idx_to_token[first]));
    }
    parts.join(" -> ")
}

fn shorten_address(addr: &str) -> String {
    if addr.len() > 10 {
        format!("{}...{}", &addr[..6], &addr[addr.len() - 4..])
    } else {
        addr.to_string()
    }
}

pub fn write_json(ranked: &[RankedCycle], graph: &Graph, path: &str) {
    use std::fs;
    use std::io::Write;

    let mut entries = Vec::new();
    for (i, r) in ranked.iter().enumerate() {
        // Closed cycle: tokens[0] == tokens[last]
        let mut tokens: Vec<&str> = r.cycle.tokens.iter().map(|&t| graph.idx_to_token[t].as_str()).collect();
        if let Some(&first) = tokens.first() {
            tokens.push(first);
        }
        let pools: Vec<&str> = r.cycle.edges.iter().map(|&e| {
            let edge = &graph.edges[e];
            graph.pool_addresses[edge.pool_idx].as_str()
        }).collect();

        entries.push(format!(
            r#"  {{
    "rank": {},
    "profit_usd": {:.2},
    "optimal_input_raw": {:.0},
    "profit_raw": {:.0},
    "start_token": "{}",
    "hops": {},
    "path_tokens": {:?},
    "path_pools": {:?}
  }}"#,
            i + 1,
            r.profit_usd,
            r.optimal_input,
            r.profit,
            r.start_token,
            r.cycle.edges.len(),
            tokens,
            pools,
        ));
    }

    let json = format!("[\n{}\n]\n", entries.join(",\n"));

    if let Some(parent) = std::path::Path::new(path).parent() {
        fs::create_dir_all(parent).ok();
    }
    let mut f = fs::File::create(path).expect("Failed to create output file");
    f.write_all(json.as_bytes()).expect("Failed to write output");
    eprintln!("Wrote results to {}", path);
}
