import { execSync } from "node:child_process";

const args = process.argv.slice(2);
if (args.length === 0) {
  console.error("Usage: pnpm check-diff <file.json> [MMDD-level]");
  process.exit(1);
}

const file = args[0];
const target = args[1] ?? "";
const targetArg = target ? ` ${target}` : "";

const rustJson = execSync(
  `cargo run --release -- check ${file}${targetArg} --json 2>/dev/null`,
  { encoding: "utf8", maxBuffer: 50 * 1024 * 1024 },
);

const tsJson = execSync(
  `node --permission --allow-fs-read='*' --experimental-transform-types scripts/check.ts ${file}${targetArg} --json 2>/dev/null`,
  { encoding: "utf8", maxBuffer: 50 * 1024 * 1024 },
);

const rustFormatted = execSync(`cargo run --release -- format-check`, {
  input: rustJson,
  encoding: "utf8",
  maxBuffer: 50 * 1024 * 1024,
});

const tsFormatted = execSync(`cargo run --release -- format-check`, {
  input: tsJson,
  encoding: "utf8",
  maxBuffer: 50 * 1024 * 1024,
});

if (rustFormatted === tsFormatted) {
  console.error("Rust and TypeScript impls match.");
  console.log(rustFormatted);
} else {
  console.error("Rust and TypeScript impls differ.\n");
  console.error("=== Rust ===");
  console.log(rustFormatted);
  console.error("\n=== TypeScript ===");
  console.log(tsFormatted);
  process.exit(1);
}
