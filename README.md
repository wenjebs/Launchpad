# DEX Arbitrage Detector

A two-part system for finding and validating profitable arbitrage cycles on Uniswap V2.

**Part 1** — Rust CLI that loads 38,141 pool snapshots, builds a token swap graph, and detects the top-10 most profitable cycles in under a second.

**Part 2** — Solidity smart contract that fetches live on-chain reserves and validates whether a cycle is still profitable, with a Node.js submission script.

---

## How it works

In AMM-based DEXes, tokens and pools form a directed graph:

```
Token A ──[Pool AB]──▶ Token B ──[Pool BC]──▶ Token C ──[Pool CA]──▶ Token A
                                                                      ▲
                                                              (more than you started with = profit)
```

Each edge represents a Uniswap V2 swap. If chaining swaps through a cycle returns more tokens than you put in — accounting for the 0.3% fee per hop — that's an arbitrage opportunity.

This tool finds those cycles automatically.

---

## Quickstart

### Part 1 — detect cycles

```bash
cargo run --release -- data/v2pools.json
```

Prints a top-10 table and writes `output/top10.json`.

### Part 2 — validate on-chain

```bash
cd contracts
bun install

# Terminal 1: start local Ethereum node (requires Foundry)
anvil --steps-tracing --host 0.0.0.0 --port 8545

# Terminal 2: validate using snapshot reserves
bun run validate:snapshot
```

To browse transactions in a local block explorer, start Otterscan before running validation:
```bash
docker run -d --name otterscan -p 5100:80 -e ERIGON_URL=http://localhost:8545 otterscan/otterscan:latest
# Open http://localhost:5100
```

Or against live mainnet:

```bash
RPC_URL=https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY bun run validate:live
```

---

## Repository structure

```
├── src/                        # Rust off-chain detector
│   ├── main.rs                 # CLI entry point
│   ├── types.rs                # Data structures
│   ├── loader.rs               # Parse + filter v2pools.json
│   ├── graph.rs                # Build directed token graph
│   ├── amm.rs                  # Uniswap V2 swap math
│   ├── detector.rs             # Bounded DFS cycle detection
│   └── ranker.rs               # Rank by profit, write output
│
├── contracts/                  # Solidity on-chain validator
│   ├── src/
│   │   ├── ArbitrageValidator.sol
│   │   └── interfaces/IUniswapV2Pair.sol
│   ├── test/
│   │   ├── ArbitrageValidator.t.sol
│   │   └── mocks/MockUniswapV2Pair.sol
│   └── script/
│       ├── compile.js          # Compile via solc
│       ├── test.js             # Run unit tests
│       ├── validate.js         # Validate against live reserves
│       └── validate-snapshot.js # Validate against snapshot reserves
│
├── data/
│   └── v2pools.json            # 38,141 Uniswap V2 pool snapshots
├── output/
│   └── top10.json              # Detection results
├── specs/                      # Challenge spec documents
└── report.md                   # Full writeup
```

---

## Documentation

| Document | Description |
|----------|-------------|
| [Part 1 — Off-Chain Detector](src/README.md) | Graph construction, cycle detection algorithm, AMM math, results |
| [Part 2 — On-Chain Validator](contracts/README.md) | Contract interface, data encoding, revert conditions, how to test |
| [Full Report](report.md) | Complete writeup including AI tool usage and findings |

---

## Results snapshot

```
Rank  Profit USD   Hops  Path
1     $1,911.02    4     WETH → SUSHI → 0x90d7... → 0x6971... → WETH
2     $1,148.01    3     WETH → 0x86fa... → 0x4d13... → WETH
3     $839.46      3     WETH → 0xe0e4... → LINK → WETH
4     $818.00      4     WETH → 0xe0e4... → LINK → 0x990f... → WETH
5     $814.10      4     WETH → 0xe0e4... → LINK → UNI → WETH
...
```

53,104 candidate cycles found in 0.14s. All top cycles anchor at WETH (the single giant SCC covers 4,561 of 4,598 tokens). The LINK→WETH path appears repeatedly in ranks 3–10, reflecting a persistent price discrepancy in Chainlink-adjacent pools.

---

## Tech stack

| Layer | Tech |
|-------|------|
| Cycle detection | Rust (no external graph library) |
| Contract | Solidity 0.8.20 |
| Compilation | solc via Node.js |
| Local node | Anvil (Foundry) with `--steps-tracing` |
| Block explorer | Otterscan (local, Docker) at `localhost:5100` |
| Testing | ethers.js v6 + mock Uniswap V2 pairs |
| Package manager | bun |
