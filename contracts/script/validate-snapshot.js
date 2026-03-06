import { ethers } from "ethers";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import { compile } from "./compile.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const contractsDir = path.resolve(__dirname, "..");
const projectDir = path.resolve(contractsDir, "..");

async function main() {
  const rpcUrl = process.env.RPC_URL || "http://127.0.0.1:8545";
  const cyclesPath = path.join(projectDir, "output", "top10.json");
  const poolsPath = path.join(projectDir, "data", "v2pools.json");

  console.log(`RPC URL: ${rpcUrl}`);
  console.log("Mode:    SNAPSHOT (deploying mock pools with v2pools.json reserves)\n");

  const provider = new ethers.JsonRpcProvider(rpcUrl);

  let signer;
  try {
    const accounts = await provider.listAccounts();
    signer = accounts[0];
  } catch {
    console.error("Cannot connect to RPC at", rpcUrl);
    process.exit(1);
  }

  // Compile
  console.log("Compiling contracts...");
  const contracts = compile({
    "ArbitrageValidator.sol": path.join(contractsDir, "src", "ArbitrageValidator.sol"),
    "MockUniswapV2Pair.sol": path.join(contractsDir, "test", "mocks", "MockUniswapV2Pair.sol"),
  });

  const validatorArtifact = contracts["ArbitrageValidator.sol"]["ArbitrageValidator"];
  const mockArtifact = contracts["MockUniswapV2Pair.sol"]["MockUniswapV2Pair"];

  // Deploy validator
  const validatorFactory = new ethers.ContractFactory(validatorArtifact.abi, "0x" + validatorArtifact.evm.bytecode.object, signer);
  const validator = await validatorFactory.deploy();
  await validator.waitForDeployment();
  console.log(`Validator deployed at ${await validator.getAddress()}\n`);

  // Load pool data and cycles
  const cycles = JSON.parse(fs.readFileSync(cyclesPath, "utf8"));
  const allPools = JSON.parse(fs.readFileSync(poolsPath, "utf8"));

  // Index pools by address
  const poolMap = {};
  for (const p of allPools) {
    poolMap[p.id.toLowerCase()] = p;
  }

  // Collect all unique pool addresses needed
  const neededPools = new Set();
  for (const cycle of cycles) {
    for (const addr of cycle.path_pools) {
      neededPools.add(addr.toLowerCase());
    }
  }

  // Deploy mock pools with snapshot reserves
  console.log(`Deploying ${neededPools.size} mock pools with snapshot reserves...`);
  const mockFactory = new ethers.ContractFactory(mockArtifact.abi, "0x" + mockArtifact.evm.bytecode.object, signer);
  const mockAddresses = {};

  for (const poolAddr of neededPools) {
    const poolData = poolMap[poolAddr];
    if (!poolData) {
      console.log(`  WARNING: pool ${poolAddr} not found in v2pools.json`);
      continue;
    }

    const dec0 = parseInt(poolData.token0.decimals);
    const dec1 = parseInt(poolData.token1.decimals);

    // Convert decimal string reserves to raw BigInt
    const r0 = decimalToBigInt(poolData.reserve0, dec0);
    const r1 = decimalToBigInt(poolData.reserve1, dec1);

    if (r0 === 0n || r1 === 0n) {
      console.log(`  WARNING: pool ${poolAddr} has zero reserves, skipping`);
      continue;
    }

    // Check reserves fit in uint112
    const MAX_UINT112 = (1n << 112n) - 1n;
    const r0Clamped = r0 > MAX_UINT112 ? MAX_UINT112 : r0;
    const r1Clamped = r1 > MAX_UINT112 ? MAX_UINT112 : r1;

    const mock = await mockFactory.deploy(
      poolData.token0.id,
      poolData.token1.id,
      r0Clamped,
      r1Clamped
    );
    await mock.waitForDeployment();
    mockAddresses[poolAddr] = await mock.getAddress();
  }

  console.log(`Deployed ${Object.keys(mockAddresses).length} mock pools\n`);

  // Validate each cycle using mock pool addresses
  const validatorContract = new ethers.Contract(
    await validator.getAddress(),
    validatorArtifact.abi,
    provider
  );

  console.log("=".repeat(100));
  console.log("  SNAPSHOT VALIDATION RESULTS (v2pools.json reserves)");
  console.log("=".repeat(100));
  console.log();
  console.log(
    `${"Rank".padEnd(6)}${"Status".padEnd(14)}${"AmountIn".padEnd(14)}${"ActualOut".padEnd(22)}${"Profit (raw)".padEnd(22)}${"Hops".padEnd(6)}Profit USD`
  );
  console.log("-".repeat(100));

  for (const cycle of cycles) {
    const tokens = cycle.path_tokens;
    // Replace real pool addresses with mock addresses
    const pools = cycle.path_pools.map(a => mockAddresses[a.toLowerCase()]).filter(Boolean);

    if (pools.length !== cycle.path_pools.length) {
      console.log(`${String(cycle.rank).padEnd(6)}${"SKIP (pool missing)".padEnd(14)}`);
      continue;
    }

    const amountIn = BigInt(Math.round(cycle.optimal_input_raw));
    const minOut = amountIn;

    try {
      const actualOut = await validatorContract.validateCycle.staticCall(
        tokens, pools, amountIn, minOut
      );
      const profit = actualOut - amountIn;

      // Estimate USD profit (USDT/USDC = 6 decimals)
      const startToken = cycle.start_token.toLowerCase();
      let profitUsd;
      if (startToken === "0xdac17f958d2ee523a2206206994597c13d831ec7" ||
          startToken === "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48") {
        profitUsd = Number(profit) / 1e6;
      } else if (startToken === "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2") {
        profitUsd = Number(profit) / 1e18 * 2000;
      } else {
        profitUsd = Number(profit) / 1e18;
      }

      console.log(
        `${String(cycle.rank).padEnd(6)}${"PROFITABLE".padEnd(14)}${amountIn.toString().padEnd(14)}${actualOut.toString().padEnd(22)}${profit.toString().padEnd(22)}${String(cycle.hops).padEnd(6)}$${profitUsd.toFixed(2)}`
      );
    } catch (err) {
      const reason = err.reason || err.revert?.args?.[0] || "Reverted";
      console.log(
        `${String(cycle.rank).padEnd(6)}${"REVERTED".padEnd(14)}${amountIn.toString().padEnd(14)}${"--".padEnd(22)}${reason.padEnd(22)}${String(cycle.hops).padEnd(6)}--`
      );
    }
  }

  console.log();
  console.log("Mock pool addresses (for Tenderly explorer):");
  for (const [orig, mock] of Object.entries(mockAddresses)) {
    console.log(`  ${orig} -> ${mock}`);
  }
  console.log();
}

function decimalToBigInt(decStr, decimals) {
  // Parse "0.001505" with `decimals` decimal places into a BigInt
  const parts = decStr.split(".");
  const intPart = parts[0] || "0";
  let fracPart = parts[1] || "";

  if (fracPart.length > decimals) {
    fracPart = fracPart.slice(0, decimals);
  } else {
    fracPart = fracPart.padEnd(decimals, "0");
  }

  const raw = intPart + fracPart;
  // Remove leading zeros but keep at least "0"
  const cleaned = raw.replace(/^0+/, "") || "0";
  return BigInt(cleaned);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
