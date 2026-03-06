import { ethers } from "ethers";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import { compile } from "./compile.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const contractsDir = path.resolve(__dirname, "..");
const projectDir = path.resolve(contractsDir, "..");

// Validator contract ABI (just the function we need)
const VALIDATOR_ABI = [
  "function validateCycle(address[] calldata tokens, address[] calldata pools, uint256 amountIn, uint256 minOut) external view returns (uint256 actualOut)",
];

async function main() {
  // --- Configuration ---
  const rpcUrl = process.env.RPC_URL || "http://127.0.0.1:8545";
  const cyclesPath = process.env.CYCLES_PATH || path.join(projectDir, "output", "top10.json");
  let validatorAddress = process.env.VALIDATOR_ADDRESS;

  console.log(`RPC URL: ${rpcUrl}`);
  console.log(`Cycles:  ${cyclesPath}\n`);

  const provider = new ethers.JsonRpcProvider(rpcUrl);

  // If no deployed address, compile and deploy
  if (!validatorAddress) {
    console.log("No VALIDATOR_ADDRESS set — compiling and deploying...\n");

    const contracts = compile({
      "ArbitrageValidator.sol": path.join(contractsDir, "src", "ArbitrageValidator.sol"),
    });

    const artifact =
      contracts["ArbitrageValidator.sol"]["ArbitrageValidator"];
    const bytecode = "0x" + artifact.evm.bytecode.object;
    const abi = artifact.abi;

    let signer;
    try {
      const accounts = await provider.listAccounts();
      signer = accounts[0];
    } catch {
      console.error("Cannot connect to RPC at", rpcUrl);
      console.error("Start a local node or set RPC_URL to a mainnet fork.");
      process.exit(1);
    }

    const factory = new ethers.ContractFactory(abi, bytecode, signer);
    const contract = await factory.deploy();
    await contract.waitForDeployment();
    validatorAddress = await contract.getAddress();
    console.log(`Deployed ArbitrageValidator at ${validatorAddress}\n`);
  }

  // --- Load cycles ---
  const cycles = JSON.parse(fs.readFileSync(cyclesPath, "utf8"));
  const validator = new ethers.Contract(validatorAddress, VALIDATOR_ABI, provider);

  // --- Validate each cycle ---
  console.log("=".repeat(90));
  console.log("  ON-CHAIN VALIDATION RESULTS");
  console.log("=".repeat(90));
  console.log();
  console.log(
    `${"Rank".padEnd(6)}${"Status".padEnd(16)}${"AmountIn".padEnd(16)}${"ActualOut".padEnd(20)}${"Profit".padEnd(16)}Hops`
  );
  console.log("-".repeat(90));

  for (const cycle of cycles) {
    const tokens = cycle.path_tokens;
    const pools = cycle.path_pools;
    const amountIn = BigInt(Math.round(cycle.optimal_input_raw));
    const minOut = amountIn; // Profitable means output > input

    try {
      const actualOut = await validator.validateCycle.staticCall(
        tokens,
        pools,
        amountIn,
        minOut
      );
      const profit = actualOut - amountIn;
      console.log(
        `${String(cycle.rank).padEnd(6)}${"PROFITABLE".padEnd(16)}${amountIn.toString().padEnd(16)}${actualOut.toString().padEnd(20)}${profit.toString().padEnd(16)}${cycle.hops}`
      );
    } catch (err) {
      const reason = err.reason || err.revert?.args?.[0] || "Unknown revert";
      console.log(
        `${String(cycle.rank).padEnd(6)}${"REVERTED".padEnd(16)}${amountIn.toString().padEnd(16)}${"—".padEnd(20)}${reason.padEnd(16)}${cycle.hops}`
      );
    }
  }

  console.log();
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
