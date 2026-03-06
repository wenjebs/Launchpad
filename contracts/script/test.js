import { ethers } from "ethers";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import { compile } from "./compile.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const contractsDir = path.resolve(__dirname, "..");

async function main() {
  // Compile contracts
  console.log("Compiling contracts...\n");
  const contracts = compile({
    "ArbitrageValidator.sol": path.join(contractsDir, "src", "ArbitrageValidator.sol"),
    "ArbitrageValidatorTest.sol": path.join(contractsDir, "test", "ArbitrageValidator.t.sol"),
    "MockUniswapV2Pair.sol": path.join(contractsDir, "test", "mocks", "MockUniswapV2Pair.sol"),
  });

  const testArtifact = contracts["ArbitrageValidatorTest.sol"]["ArbitrageValidatorTest"];
  const bytecode = "0x" + testArtifact.evm.bytecode.object;
  const abi = testArtifact.abi;

  // Use a local hardhat-style provider (anvil, hardhat node, or ganache)
  // Default to a simple in-memory provider via ethers
  const provider = new ethers.JsonRpcProvider("http://127.0.0.1:8545");

  let signer;
  try {
    // Try to get a signer from the local node
    const accounts = await provider.listAccounts();
    signer = accounts[0];
  } catch {
    console.log("No local Ethereum node found at http://127.0.0.1:8545");
    console.log("To run tests, start a local node:");
    console.log("  npx hardhat node");
    console.log("  # or: anvil");
    console.log("Then re-run: npm test");
    process.exit(1);
  }

  // Deploy test contract
  console.log("Deploying ArbitrageValidatorTest...");
  const factory = new ethers.ContractFactory(abi, bytecode, signer);
  const testContract = await factory.deploy();
  await testContract.waitForDeployment();
  const addr = await testContract.getAddress();
  console.log(`Deployed at ${addr}\n`);

  // Run individual tests
  const tests = [
    "testProfitable2Hop",
    "testUnprofitable2Hop",
    "testMinHopsRevert",
    "testLengthMismatchRevert",
    "testNotCycleRevert",
    "testProfitable3Hop",
  ];

  let passed = 0;
  let failed = 0;

  for (const testName of tests) {
    try {
      await testContract[testName].staticCall();
      console.log(`  PASS  ${testName}`);
      passed++;
    } catch (err) {
      console.log(`  FAIL  ${testName}: ${err.reason || err.message}`);
      failed++;
    }
  }

  console.log(`\n${passed} passed, ${failed} failed out of ${tests.length} tests`);
  process.exit(failed > 0 ? 1 : 0);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
