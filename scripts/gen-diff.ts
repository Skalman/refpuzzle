import { x } from "./common.ts";

const args = process.argv.slice(2);
if (args.length === 0) {
  console.error("Usage: pnpm gen-diff <date-range>");
  process.exit(1);
}

const dateRange = args[0];
if (!/^[0-9.-]+$/.test(dateRange)) {
  console.error(`Invalid date range: ${dateRange}`);
  process.exit(1);
}

const rsJson = x(`cargo run --release -- gen ${dateRange} -o -`);
const tsJson = x(`node scripts/gen.ts ${dateRange} -o -`);

const rs = JSON.parse(rsJson);
const ts = JSON.parse(tsJson);

if (JSON.stringify(rs) === JSON.stringify(ts)) {
  console.log("OK");
} else {
  console.error("DIFF");
  process.exit(1);
}
