# Part 2 — On-Chain Validator

Solidity contract that validates arbitrage cycles against live Uniswap V2 reserves, plus Node.js scripts for compilation, testing, and submission.

## Contract interface

```solidity
function validateCycle(
    address[] calldata tokens,  // closed path: tokens[0] == tokens[last]
    address[] calldata pools,   // Uniswap V2 pair addresses, one per hop
    uint256 amountIn,           // starting amount in raw token units
    uint256 minOut              // minimum output — set to amountIn for breakeven
) external returns (uint256 actualOut)
```

For each hop the contract:
1. Calls `IUniswapV2Pair(pool).getReserves()` — fetches live on-chain reserves
2. Calls `IUniswapV2Pair(pool).token0()` — determines swap direction
3. Applies exact `uint256` Uniswap V2 swap math: `(amountIn × 997 × reserveOut) / (reserveIn × 1000 + amountIn × 997)`

If the final output is below `minOut`, the transaction reverts.

## Revert conditions

| Condition | Error |
|-----------|-------|
| `tokens.length < 3` | `"Min 2 hops"` |
| `tokens.length != pools.length + 1` | `"Length mismatch"` |
| `tokens[0] != tokens[last]` | `"Not a cycle"` |
| `output < minOut` | `"Below minimum output"` |
| Pool `getReserves()` fails | External call revert |
| Zero reserves | Division by zero (arithmetic revert) |

## Data encoding

Cycles from Part 1 (`output/top10.json`) are passed directly. The `path_tokens` array is already closed (`tokens[0] == tokens[last]`) and `path_pools` contains real Uniswap V2 pair addresses.

Example — 3-hop cycle:
```
tokens: [USDT, 0xd233..., DAI, USDT]     // 4 elements (hops + 1)
pools:  [0x50b6..., 0x1d11..., 0xb20b...] // 3 elements (one per hop)
amountIn: 5
minOut: 5
```

## Setup

```bash
bun install
```

Requires [bun](https://bun.sh). Node.js also works for all scripts.

## Commands

| Command | Description |
|---------|-------------|
| `bun run compile` | Compile all Solidity → `out/` |
| `bun run node` | Kill anything on :8545 + start fresh Hardhat node |
| `bun run test` | Deploy and run 6 unit tests |
| `bun run validate:snapshot` | Deploy mock pools with v2pools.json reserves, validate top-10 cycles |
| `bun run validate:live` | Validate against live mainnet reserves (requires `RPC_URL`) |

## Running tests

```bash
# Terminal 1
bun run node

# Terminal 2
bun run test
```

Expected output:
```
  PASS  testProfitable2Hop
  PASS  testUnprofitable2Hop
  PASS  testMinHopsRevert
  PASS  testLengthMismatchRevert
  PASS  testNotCycleRevert
  PASS  testProfitable3Hop

6 passed, 0 failed out of 6 tests
```

## Validating with snapshot data

Uses `MockUniswapV2Pair` contracts deployed with the exact reserves from `data/v2pools.json`. Lets you verify the Solidity math matches the Rust output without needing a live RPC.

```bash
bun run node              # Terminal 1
bun run validate:snapshot # Terminal 2
```

All 10 Part 1 cycles should show as **PROFITABLE** — the on-chain `uint256` arithmetic matches the off-chain `f64` results exactly.

## Validating with live reserves

```bash
RPC_URL=https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY bun run validate:live
```

Most cycles will revert as unprofitable — the snapshot data is historical and MEV bots would have already captured any real opportunities. That's expected behaviour and confirms the contract is working correctly.

## Tenderly Virtual TestNet

To see transactions in a block explorer:

1. Create a Virtual TestNet at [dashboard.tenderly.co](https://dashboard.tenderly.co) (fork Ethereum Mainnet)
2. Copy the RPC URL
3. Run:

```bash
RPC_URL=https://virtual.mainnet.rpc.tenderly.co/YOUR_ID bun run validate:snapshot
```

All contract deployments and validation calls appear in the Tenderly explorer with full call traces and decoded inputs.

## File structure

```
contracts/
├── src/
│   ├── ArbitrageValidator.sol        # Main contract
│   └── interfaces/
│       └── IUniswapV2Pair.sol        # Uniswap V2 pair interface
├── test/
│   ├── ArbitrageValidator.t.sol      # 6 unit tests
│   └── mocks/
│       └── MockUniswapV2Pair.sol     # Configurable mock pool
└── script/
    ├── compile.js                    # solc compiler wrapper
    ├── test.js                       # test runner
    ├── validate.js                   # live reserves validation
    ├── validate-snapshot.js          # snapshot reserves validation
    └── validate-snapshot-tx.js       # snapshot validation as txs (explorer-visible)
```
