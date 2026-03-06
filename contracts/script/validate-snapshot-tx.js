// Same as validate-snapshot but sends actual transactions so they show in Tenderly explorer
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
  console.log("Mode:    SNAPSHOT + TX (visible in Tenderly explorer)\n");

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

  // Deploy validator (non-view version needed for tx visibility)
  // We'll use the view version but call it as a tx — Tenderly records it either way
  const validatorFactory = new ethers.ContractFactory(validatorArtifact.abi, "0x" + validatorArtifact.evm.bytecode.object, signer);
  const validator = await validatorFactory.deploy();
  await validator.waitForDeployment();
  const validatorAddr = await validator.getAddress();
  console.log(`Validator deployed at ${validatorAddr}\n`);

  // Load data
  const cycles = JSON.parse(fs.readFileSync(cyclesPath, "utf8"));
  const allPools = JSON.parse(fs.readFileSync(poolsPath, "utf8"));
  const poolMap = {};
  for (const p of allPools) poolMap[p.id.toLowerCase()] = p;

  // Collect needed pools
  const neededPools = new Set();
  for (const cycle of cycles) {
    for (const addr of cycle.path_pools) neededPools.add(addr.toLowerCase());
  }

  // Deploy mock pools
  console.log(`Deploying ${neededPools.size} mock pools...`);
  const mockFactory = new ethers.ContractFactory(mockArtifact.abi, "0x" + mockArtifact.evm.bytecode.object, signer);
  const mockAddresses = {};

  for (const poolAddr of neededPools) {
    const poolData = poolMap[poolAddr];
    if (!poolData) continue;
    const dec0 = parseInt(poolData.token0.decimals);
    const dec1 = parseInt(poolData.token1.decimals);
    const r0 = decimalToBigInt(poolData.reserve0, dec0);
    const r1 = decimalToBigInt(poolData.reserve1, dec1);
    if (r0 === 0n || r1 === 0n) continue;
    const MAX_UINT112 = (1n << 112n) - 1n;
    const mock = await mockFactory.deploy(
      poolData.token0.id, poolData.token1.id,
      r0 > MAX_UINT112 ? MAX_UINT112 : r0,
      r1 > MAX_UINT112 ? MAX_UINT112 : r1
    );
    await mock.waitForDeployment();
    mockAddresses[poolAddr] = await mock.getAddress();
  }
  console.log(`Deployed ${Object.keys(mockAddresses).length} mock pools\n`);

  // Validate each cycle as an actual TRANSACTION (not staticCall)
  const validatorWithSigner = new ethers.Contract(validatorAddr, validatorArtifact.abi, signer);

  console.log("=".repeat(100));
  console.log("  SNAPSHOT VALIDATION (transactions visible in Tenderly explorer)");
  console.log("=".repeat(100));
  console.log();

  for (const cycle of cycles) {
    const tokens = cycle.path_tokens;
    const pools = cycle.path_pools.map(a => mockAddresses[a.toLowerCase()]).filter(Boolean);
    if (pools.length !== cycle.path_pools.length) {
      console.log(`Rank ${cycle.rank}: SKIP (missing pool)`);
      continue;
    }

    const amountIn = BigInt(Math.round(cycle.optimal_input_raw));
    const minOut = amountIn;

    try {
      // Send as actual transaction so it appears in Tenderly
      const tx = await validatorWithSigner.validateCycle(tokens, pools, amountIn, minOut);
      const receipt = await tx.wait();
      console.log(`Rank ${cycle.rank}: PROFITABLE  tx=${receipt.hash}`);
    } catch (err) {
      // Even reverts show up in Tenderly
      const reason = err.reason || "Reverted";
      console.log(`Rank ${cycle.rank}: REVERTED    reason=${reason}`);
    }
  }

  console.log("\nDone! Check your Tenderly explorer to see all transactions.");
  console.log(`Explorer: https://dashboard.tenderly.co/explorer/vnet/${rpcUrl.split('/').pop()}`);
}

function decimalToBigInt(decStr, decimals) {
  const parts = decStr.split(".");
  const intPart = parts[0] || "0";
  let fracPart = parts[1] || "";
  if (fracPart.length > decimals) fracPart = fracPart.slice(0, decimals);
  else fracPart = fracPart.padEnd(decimals, "0");
  const cleaned = (intPart + fracPart).replace(/^0+/, "") || "0";
  return BigInt(cleaned);
}

main().catch((err) => { console.error(err); process.exit(1); });
