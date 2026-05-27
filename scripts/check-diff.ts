import { x } from "./common.ts";

const args = process.argv.slice(2);
if (args.length === 0) {
  console.error("Usage: pnpm check-diff <file.json> [MMDD-level]");
  process.exit(1);
}

const file = args[0];
const target = args[1] ?? "";
const targetArg = target ? ` ${target}` : "";

const rustJson = x(`cargo run --release -- check ${file}${targetArg} --json`);

const tsJson = x(
  `node --permission --allow-fs-read='*' scripts/check.ts ${file}${targetArg} --json`,
);

const rustFormatted = x(`cargo run --release -- format-check`, { input: rustJson });
const tsFormatted = x(`cargo run --release -- format-check`, { input: tsJson });

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
