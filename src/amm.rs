use crate::graph::Graph;
use crate::types::Cycle;

/// Uniswap V2 getAmountOut with 0.3% fee (997/1000)
pub fn get_amount_out(amount_in: f64, reserve_in: f64, reserve_out: f64) -> f64 {
    if amount_in <= 0.0 || reserve_in <= 0.0 || reserve_out <= 0.0 {
        return 0.0;
    }
    let amount_in_with_fee = amount_in * 997.0;
    let numerator = amount_in_with_fee * reserve_out;
    let denominator = reserve_in * 1000.0 + amount_in_with_fee;
    numerator / denominator
}

/// Simulate a cycle: given an input amount of the starting token,
/// chain swaps through each edge and return the final output amount.
pub fn simulate_cycle(graph: &Graph, cycle: &Cycle, amount_in: f64) -> f64 {
    let mut amount = amount_in;
    for &edge_idx in &cycle.edges {
        let edge = &graph.edges[edge_idx];
        amount = get_amount_out(amount, edge.reserve_in, edge.reserve_out);
        if amount <= 0.0 {
            return 0.0;
        }
    }
    amount
}

/// Find optimal input using golden section search.
/// Returns (optimal_input, output_at_optimal).
pub fn optimal_input(graph: &Graph, cycle: &Cycle) -> (f64, f64) {
    // Upper bound: use a fraction of the first edge's reserve_in.
    // The first edge's reserve_in is always in the starting token's own units,
    // so it's a safe bound. Using min across all edges would mix raw reserves
    // of different tokens with different decimals, producing a wrong cap.
    let max_input = graph.edges[cycle.edges[0]].reserve_in * 0.3;

    if max_input <= 0.0 {
        return (0.0, 0.0);
    }

    // Profit function: output - input
    let profit = |x: f64| -> f64 { simulate_cycle(graph, cycle, x) - x };

    // Golden section search for maximum
    let gr = (5f64.sqrt() - 1.0) / 2.0;
    let mut a = 0.0_f64;
    let mut b = max_input;
    let tol = max_input * 1e-12;

    for _ in 0..100 {
        let c = b - gr * (b - a);
        let d = a + gr * (b - a);
        if profit(c) > profit(d) {
            b = d;
        } else {
            a = c;
        }
        if (b - a).abs() < tol {
            break;
        }
    }

    let opt = (a + b) / 2.0;
    let out = simulate_cycle(graph, cycle, opt);
    (opt, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_amount_out() {
        // 1 ETH in, reserves 100 ETH / 200000 USDC
        let amount_out = get_amount_out(1e18, 100e18, 200_000e6);
        // Expected: ~1970.05 USDC (with 0.3% fee)
        let usdc = amount_out / 1e6;
        assert!(usdc > 1960.0 && usdc < 1980.0, "got {}", usdc);
    }

    #[test]
    fn test_get_amount_out_zero() {
        assert_eq!(get_amount_out(0.0, 100.0, 200.0), 0.0);
        assert_eq!(get_amount_out(100.0, 0.0, 200.0), 0.0);
    }
}
