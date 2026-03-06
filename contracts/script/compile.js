import solc from "solc";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const contractsDir = path.resolve(__dirname, "..");

function findImports(importPath) {
  // Resolve relative imports from src/ and test/
  const candidates = [
    path.join(contractsDir, "src", importPath),
    path.join(contractsDir, "test", importPath),
    path.join(contractsDir, importPath),
  ];
  for (const p of candidates) {
    if (fs.existsSync(p)) {
      return { contents: fs.readFileSync(p, "utf8") };
    }
  }
  // Try stripping leading ./
  const stripped = importPath.replace(/^\.\//, "");
  for (const base of [
    path.join(contractsDir, "src"),
    path.join(contractsDir, "test"),
  ]) {
    const p = path.join(base, stripped);
    if (fs.existsSync(p)) {
      return { contents: fs.readFileSync(p, "utf8") };
    }
  }
  return { error: `File not found: ${importPath}` };
}

export function compile(sourceFiles) {
  const sources = {};
  for (const [name, filePath] of Object.entries(sourceFiles)) {
    sources[name] = { content: fs.readFileSync(filePath, "utf8") };
  }

  const input = {
    language: "Solidity",
    sources,
    settings: {
      outputSelection: {
        "*": {
          "*": ["abi", "evm.bytecode.object"],
        },
      },
      optimizer: { enabled: true, runs: 200 },
    },
  };

  const output = JSON.parse(
    solc.compile(JSON.stringify(input), { import: findImports })
  );

  if (output.errors) {
    const errors = output.errors.filter((e) => e.severity === "error");
    if (errors.length > 0) {
      console.error("Compilation errors:");
      errors.forEach((e) => console.error(e.formattedMessage));
      process.exit(1);
    }
  }

  return output.contracts;
}

// When run directly, compile and save artifacts
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const contracts = compile({
    "ArbitrageValidator.sol": path.join(
      contractsDir,
      "src",
      "ArbitrageValidator.sol"
    ),
    "ArbitrageValidatorTest.sol": path.join(
      contractsDir,
      "test",
      "ArbitrageValidator.t.sol"
    ),
    "MockUniswapV2Pair.sol": path.join(
      contractsDir,
      "test",
      "mocks",
      "MockUniswapV2Pair.sol"
    ),
  });

  const outDir = path.join(contractsDir, "out");
  fs.mkdirSync(outDir, { recursive: true });

  for (const [fileName, fileContracts] of Object.entries(contracts)) {
    for (const [contractName, contractData] of Object.entries(fileContracts)) {
      const artifact = {
        abi: contractData.abi,
        bytecode: "0x" + contractData.evm.bytecode.object,
      };
      fs.writeFileSync(
        path.join(outDir, `${contractName}.json`),
        JSON.stringify(artifact, null, 2)
      );
      console.log(`Compiled ${contractName} -> out/${contractName}.json`);
    }
  }
}
