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
  process.exit(0);
}

console.error("DIFF");

// Find the first divergent (date, level) pair and show both puzzles side by side.
type Year = Record<string, Record<string, unknown>>;
const rsY = rs as Year;
const tsY = ts as Year;

const dates = [...new Set([...Object.keys(rsY), ...Object.keys(tsY)])].sort();
let shown = 0;
const MAX = 3;

for (const date of dates) {
  if (shown >= MAX) break;
  const rDay = rsY[date] ?? {};
  const tDay = tsY[date] ?? {};
  if (JSON.stringify(rDay) === JSON.stringify(tDay)) continue;

  const levels = [...new Set([...Object.keys(rDay), ...Object.keys(tDay)])].sort();
  for (const lvl of levels) {
    if (shown >= MAX) break;
    const rJ = JSON.stringify(rDay[lvl] ?? null);
    const tJ = JSON.stringify(tDay[lvl] ?? null);
    if (rJ === tJ) continue;
    console.error(`\n--- diff at date=${date} level=${lvl} ---`);
    console.error(`  RS: ${rJ}`);
    console.error(`  TS: ${tJ}`);
    shown++;
  }
}

const totalDates = dates.length;
const diffDates = dates.filter(
  (d) => JSON.stringify(rsY[d] ?? {}) !== JSON.stringify(tsY[d] ?? {}),
).length;
console.error(`\n(${diffDates}/${totalDates} dates differ; showing up to ${MAX} entries)`);

process.exit(1);
