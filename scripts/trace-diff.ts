import { execSync } from "node:child_process";

const args = process.argv.slice(2);
let dateRange: string | null = null;
let level: string | null = null;

for (let i = 0; i < args.length; i++) {
  if (args[i] === "-l" || args[i] === "--level") {
    level = args[++i];
  } else if (!args[i].startsWith("-")) {
    dateRange = args[i];
  }
}

if (!dateRange || !level) {
  console.error("Usage: pnpm trace-diff <date> -l <level>");
  console.error("Example: pnpm trace-diff 2051-01-01 -l 6");
  process.exit(1);
}

const tsCmd = `pnpm --silent generate ${dateRange} -l ${level} --trace -o /dev/null`;
const rsCmd = `cargo run --release -- gen ${dateRange} -l ${level} --trace -o /dev/null`;
function run(cmd: string): string {
  return execSync(`${cmd} 2>&1`, { encoding: "utf8" })
    .split("\n")
    .filter(
      (l) =>
        !/^\s*(Compiling|Finished|Running|warning:|package:|workspace:)/.test(l) &&
        !/^\s*Time:/.test(l),
    )
    .join("\n");
}

const tsOut = run(tsCmd);
const rsOut = run(rsCmd);

const tsLines = tsOut.trimEnd().split("\n");
const rsLines = rsOut.trimEnd().split("\n");

let i = 0;
while (i < tsLines.length && i < rsLines.length && tsLines[i] === rsLines[i]) {
  console.log(tsLines[i]);
  i++;
}

if (i >= tsLines.length && i >= rsLines.length) {
  console.log(`\n=== IDENTICAL (${String(i)} lines) ===`);
  process.exit(0);
}

console.log(`\n=== DIVERGENCE at line ${String(i + 1)} ===`);

if (i < tsLines.length) {
  console.log("\n--- TypeScript ---");
  for (let j = i; j < Math.min(i + 30, tsLines.length); j++) console.log(tsLines[j]);
  if (tsLines.length > i + 30) console.log(`... (${String(tsLines.length - i - 30)} more lines)`);
}

if (i < rsLines.length) {
  console.log("\n--- Rust ---");
  for (let j = i; j < Math.min(i + 30, rsLines.length); j++) console.log(rsLines[j]);
  if (rsLines.length > i + 30) console.log(`... (${String(rsLines.length - i - 30)} more lines)`);
}

process.exit(1);
