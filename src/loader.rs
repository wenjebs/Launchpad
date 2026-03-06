use crate::types::{Pool, RawPool};
use std::fs;
use std::path::Path;

/// Parse a decimal string like "0.001505" into a float.
fn parse_decimal(s: &str) -> f64 {
    s.parse::<f64>().unwrap_or(0.0)
}

/// Convert a human-readable reserve string to raw token units (as f64).
/// E.g. "0.001505" with 18 decimals -> 0.001505 * 10^18
fn to_raw(reserve_str: &str, decimals: u32) -> f64 {
    let val = parse_decimal(reserve_str);
    val * 10f64.powi(decimals as i32)
}

pub fn load_pools(path: &Path) -> Vec<Pool> {
    let data = fs::read_to_string(path).expect("Failed to read pool data file");
    let raw_pools: Vec<RawPool> = serde_json::from_str(&data).expect("Failed to parse JSON");

    let mut pools = Vec::new();
    for rp in &raw_pools {
        let reserve_usd = parse_decimal(&rp.reserve_usd);
        if reserve_usd < 1000.0 {
            continue;
        }

        let dec0: u32 = rp.token0.decimals.parse().unwrap_or(18);
        let dec1: u32 = rp.token1.decimals.parse().unwrap_or(18);

        let r0 = to_raw(&rp.reserve0, dec0);
        let r1 = to_raw(&rp.reserve1, dec1);

        let r0_human = parse_decimal(&rp.reserve0);
        let r1_human = parse_decimal(&rp.reserve1);
        if r0 <= 0.0 || r1 <= 0.0 || r0_human < 0.01 || r1_human < 0.01 {
            continue;
        }

        pools.push(Pool {
            id: rp.id.clone(),
            token0: rp.token0.id.to_lowercase(),
            token1: rp.token1.id.to_lowercase(),
            reserve0: r0,
            reserve1: r1,
            reserve_usd,
        });
    }

    eprintln!(
        "Loaded {} pools from {} total (filtered by reserveUSD >= $1000)",
        pools.len(),
        raw_pools.len()
    );
    pools
}
