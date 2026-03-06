use serde::Deserialize;

#[derive(Deserialize)]
pub struct RawToken {
    pub decimals: String,
    pub id: String,
}

#[derive(Deserialize)]
pub struct RawPool {
    pub id: String,
    pub reserve0: String,
    pub reserve1: String,
    #[serde(rename = "reserveUSD")]
    pub reserve_usd: String,
    #[serde(rename = "reserveETH")]
    pub reserve_eth: Option<String>,
    pub token0: RawToken,
    pub token1: RawToken,
}

pub struct Pool {
    pub id: String,
    pub token0: String,
    pub token1: String,
    pub reserve0: f64,
    pub reserve1: f64,
    pub reserve_usd: f64,
}

pub struct Edge {
    pub pool_idx: usize,
    pub token_in: usize,
    pub token_out: usize,
    pub reserve_in: f64,
    pub reserve_out: f64,
}

pub struct Cycle {
    pub edges: Vec<usize>,
    pub tokens: Vec<usize>,
}

pub struct RankedCycle {
    pub cycle: Cycle,
    pub optimal_input: f64,
    pub profit: f64,
    pub profit_usd: f64,
    pub start_token: String,
}
