mod amm;
mod detector;
mod graph;
mod loader;
mod ranker;
mod types;

use std::path::Path;
use std::time::Instant;

fn main() {
    let data_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "data/v2pools.json".to_string());

    let t0 = Instant::now();

    // Step 1: Load and filter pools
    let pools = loader::load_pools(Path::new(&data_path));

    // Step 2: Build graph
    let graph = graph::build_graph(&pools);

    // Step 3: Detect cycles
    let cycles = detector::detect_cycles(&graph);

    // Step 4: Rank and output
    let ranked = ranker::rank_cycles(&graph, cycles, 10);
    ranker::print_results(&ranked, &graph);
    ranker::write_json(&ranked, &graph, "output/top10.json");

    eprintln!("Total time: {:.2}s", t0.elapsed().as_secs_f64());
}
