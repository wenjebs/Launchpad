# DEX Arbitrage Challenge - Project Overview

## Goal

Build a system that detects and ranks profitable arbitrage cycles across Uniswap V2 pools, with optional on-chain validation via a Solidity smart contract.

## Architecture

```
+--------------------+       +---------------------+       +------------------+
|  Data Ingestion    | ----> |  Graph Construction | ----> | Cycle Detection  |
|  (JSON pool data)  |       |  (Directed graph)   |       | (Bellman-Ford /  |
|                    |       |  Nodes = tokens     |       |  DFS enumeration)|
|                    |       |  Edges = pools      |       |                  |
+--------------------+       +---------------------+       +--------+---------+
                                                                    |
                                                           +--------v---------+
                                                           | Profit Simulation|
                                                           | (AMM formula,    |
                                                           |  optimal sizing) |
                                                           +--------+---------+
                                                                    |
                                                           +--------v---------+
                                                           | Ranking & Output |
                                                           | (Top-10 cycles)  |
                                                           +--------+---------+
                                                                    |
                                                           +--------v---------+
                                                           | On-Chain Valid.  |
                                                           | (Solidity, opt.) |
                                                           +------------------+
```

## Deliverables

### Part 1: Off-Chain (Required)
- Rust binary that loads pool data, builds swap graph, detects arbitrage cycles
- Outputs top-10 most profitable cycles with suggested trade sizes
- Report: graph construction, detection logic, ranking metric

### Part 2: On-Chain (Optional)
- Solidity contract accepting `(tokens[], pools[], amountIn, minOut)`
- Submission script for cycle validation
- Report: encoding format, validation logic, revert conditions

## Tech Stack

| Component         | Technology          | Rationale                                    |
|-------------------|---------------------|----------------------------------------------|
| Cycle detection   | Rust                | Performance, safety, challenge requirement   |
| Graph library     | `petgraph`          | Mature Rust graph lib with Bellman-Ford       |
| Math              | `num` / `ruint`     | Arbitrary precision for reserve arithmetic   |
| Serialization     | `serde` + `serde_json` | Pool data ingestion                       |
| Smart contract    | Solidity ^0.8.x     | On-chain validation                          |
| Contract tooling  | Foundry (forge)     | Testing and deployment                       |
| Testnet           | Sepolia             | Free ETH via faucets                         |

## Data Flow

1. **Input**: JSON file with Uniswap V2 pool snapshots (reserves, tokens, fees)
2. **Process**: Build directed graph, detect cycles, simulate swaps, rank by profit
3. **Output**: Top-10 cycles as structured output (JSON + human-readable table)
