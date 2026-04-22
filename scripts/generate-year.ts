/**
 * CLI to generate a year of daily puzzles, matching the Rust generator's output.
 *
 * Usage: node --experimental-strip-types src/generator/generate-year.ts --year YYYY [--start YYYY-MM-DD] [--attempts A]
 *
 * Seeds are derived from the date, so the same date always produces the same puzzle.
 * Output: JSON to stdout with { "MMDD": { "level-1": ..., "level-2": ..., ... }, ... }
 */
import { profiles } from "../src/generator/difficulty.ts";
import { RNG } from "../src/generator/rng.ts";
import { generateConstructive } from "../src/generator/construct.ts";

function datesInYear(year: number, startMm: number, startDd: number): [number, number][] {
  const daysInMonth = (m: number) => {
    if ([1, 3, 5, 7, 8, 10, 12].includes(m)) return 31;
    if ([4, 6, 9, 11].includes(m)) return 30;
    return year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0) ? 29 : 28;
  };
  const result: [number, number][] = [];
  let mm = startMm;
  let dd = startDd;
  while (mm <= 12) {
    while (dd <= daysInMonth(mm)) {
      result.push([mm, dd]);
      dd++;
    }
    mm++;
    dd = 1;
  }
  return result;
}

function dateSeed(year: number, mm: number, dd: number, level: number, retry: number): number {
  const dateKey = year * 10000 + mm * 100 + dd;
  return (
    Math.imul(Math.imul(dateKey, 31) + level, 17) + Math.imul(retry, 0x9e3779b9)
  ) >>> 0;
}

// ── Parse args ──

const args = process.argv.slice(2);
let year = 2026;
let startDate: string | null = null;
let maxAttempts = 100;

for (let i = 0; i < args.length; i++) {
  if (args[i] === "--year" || args[i] === "-y") year = Number(args[++i]);
  else if (args[i] === "--start") startDate = args[++i];
  else if (args[i] === "--attempts" || args[i] === "-a") maxAttempts = Number(args[++i]);
  else if (args[i] === "--help" || args[i] === "-h") {
    process.stderr.write(
      "Usage: generate-year.ts --year YYYY [--start YYYY-MM-DD] [--attempts A]\n" +
      "  Generates a year of daily puzzles (all 5 levels per day).\n" +
      "  Seeds are derived from the date, so the same date always produces the same puzzle.\n" +
      "  --start defaults to YYYY-01-01 (or 2026-04-19 for 2026).\n",
    );
    process.exit(0);
  }
}

const start = startDate ?? (year === 2026 ? "2026-04-19" : `${year}-01-01`);
const startMm = Number(start.slice(5, 7));
const startDd = Number(start.slice(8, 10));
const days = datesInYear(year, startMm, startDd);

process.stderr.write(
  `Generating ${days.length} days for year ${year} (start=${start})...\n`,
);
const t0 = Date.now();

// ── Generate ──

const yearMap: Record<string, Record<string, unknown>> = {};
let okCount = 0;
let failCount = 0;
const doneByLevel = [0, 0, 0, 0, 0];
let lastReport = Date.now();

for (const [mm, dd] of days) {
  const key = String(mm).padStart(2, "0") + String(dd).padStart(2, "0");
  const dayPuzzles: Record<string, unknown> = {};

  for (let level = 1; level <= 5; level++) {
    const profile = profiles[level - 1];
    let generated = false;

    for (let retry = 0; retry < 100; retry++) {
      const seed = dateSeed(year, mm, dd, level, retry);
      const rng = new RNG(seed);
      const result = generateConstructive(profile, rng, maxAttempts);
      if (result) {
        const { puzzle } = result;
        dayPuzzles[`level-${level}`] = {
          difficulty: puzzle.difficulty,
          questions: puzzle.questions,
        };
        okCount++;
        doneByLevel[level - 1]++;
        generated = true;
        break;
      }
    }

    if (!generated) {
      failCount++;
      process.stderr.write(`  FAILED: ${key} level-${level}\n`);
    }

    const now = Date.now();
    if (now - lastReport >= 15000) {
      const total = days.length * 5;
      process.stderr.write(
        `  ${okCount}/${total}: L1=${doneByLevel[0]} L2=${doneByLevel[1]} L3=${doneByLevel[2]} L4=${doneByLevel[3]} L5=${doneByLevel[4]}\n`,
      );
      lastReport = now;
    }
  }

  yearMap[key] = dayPuzzles;
}

// ── Summary ──

const elapsed = (Date.now() - t0) / 1000;
const json = JSON.stringify(yearMap);

process.stderr.write(`\n=== Summary ===\n`);
process.stderr.write(`  Year:    ${year}\n`);
process.stderr.write(`  Start:   ${start}\n`);
process.stderr.write(`  Days:    ${days.length}\n`);
process.stderr.write(`  Puzzles: ${okCount}/${days.length * 5} (${failCount} failed)\n`);
process.stderr.write(`  Time:    ${elapsed.toFixed(1)}s (${(elapsed / days.length * 1000).toFixed(0)}ms per day)\n`);
process.stderr.write(`  Output:  ${(json.length / 1024).toFixed(0)}KB JSON\n`);

process.stdout.write(json + "\n");
