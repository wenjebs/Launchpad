mod amm;
mod detector;
mod graph;
mod loader;
mod ranker;
mod types;

use std::path::Path;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let data_path = args
        .iter()
        .find(|a| !a.starts_with("--") && **a != args[0])
        .cloned()
        .unwrap_or_else(|| "data/v2pools.json".to_string());

    // --anchor WETH|USDT|USDC|DAI|WBTC
    let anchor: Option<String> = args
        .windows(2)
        .find(|w| w[0] == "--anchor")
        .map(|w| w[1].to_uppercase());

    if let Some(ref a) = anchor {
        eprintln!("[Config] Anchor token: {}", a);
    }

    let t0 = Instant::now();

    // Step 1: Load and filter pools
    let pools = loader::load_pools(Path::new(&data_path));

    // Step 2: Build graph
    let graph = graph::build_graph(&pools);

    // Step 3: Detect cycles
    let cycles = detector::detect_cycles(&graph, anchor.as_deref());

    // Step 4: Rank and output
    let ranked = ranker::rank_cycles(&graph, cycles, 10);
    ranker::print_results(&ranked, &graph);
    ranker::write_json(&ranked, &graph, "output/top10.json");

    eprintln!("Total time: {:.2}s", t0.elapsed().as_secs_f64());
}
