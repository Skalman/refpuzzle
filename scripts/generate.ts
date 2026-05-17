import { writeFileSync, readFileSync } from "node:fs";
import { generateConstructive as generate } from "../src/generator/construct.ts";
import { profiles } from "../src/generator/difficulty.ts";
import { RNG } from "../src/generator/rng.ts";
import type { Puzzle, QuestionType, Claim } from "../src/engine/types.ts";

// ── CLI ──

const args = process.argv.slice(2);
let year = 2026;
let startDate: string | null = null;
let endDate: string | null = null;
let outputPath: string | null = null;
let merge = false;
let levelFilter: number | null = null;
let maxAttempts = 100;
let showStats = false;

for (let i = 0; i < args.length; i++) {
  switch (args[i]) {
    case "--year":
    case "-y":
      year = Number(args[++i]);
      break;
    case "--start":
      startDate = args[++i];
      break;
    case "--end":
      endDate = args[++i];
      break;
    case "--output":
    case "-o":
      outputPath = args[++i];
      break;
    case "--merge":
    case "-m":
      merge = true;
      break;
    case "--level":
    case "-l": {
      const l = Number(args[++i]);
      if (l < 1 || l > 6) {
        console.error("level must be 1-6");
        process.exit(1);
      }
      levelFilter = l;
      break;
    }
    case "--attempts":
    case "-a":
      maxAttempts = Number(args[++i]);
      break;
    case "--stats":
      showStats = true;
      break;
    case "--help":
    case "-h":
      console.error(
        "Usage: node scripts/generate.ts --year YYYY -o FILE [--start YYYY-MM-DD] [--end YYYY-MM-DD] [--level 1-6] [-m] [--stats]",
      );
      console.error("  -o FILE  output file (required, use - for stdout)");
      console.error("  -m       merge into existing file");
      console.error("  --level  generate only this level (default: all 6)");
      console.error("  --start  defaults to YYYY-01-01 (or 2026-04-19 for 2026)");
      console.error("  --end    defaults to YYYY-12-31");
      console.error("  --stats  show generation statistics");
      process.exit(0);
      break;
    default:
      console.error(`Unknown argument: ${args[i]}`);
      process.exit(1);
  }
}

if (!outputPath) {
  console.error("Error: -o/--output is required (use -o - for stdout)");
  process.exit(1);
}

const start = startDate ?? (year === 2026 ? "2026-04-19" : `${year}-01-01`);
const end = endDate ?? `${year}-12-31`;
const startMm = Number(start.slice(5, 7));
const startDd = Number(start.slice(8, 10));
const endMm = Number(end.slice(5, 7));
const endDd = Number(end.slice(8, 10));

// ── Date enumeration ──

function datesInYear(
  yr: number,
  sMm: number,
  sDd: number,
  eMm: number,
  eDd: number,
): [number, number][] {
  const daysInMonth = (m: number) => {
    if (m === 2) {
      return yr % 4 === 0 && (yr % 100 !== 0 || yr % 400 === 0) ? 29 : 28;
    }
    return [0, 31, 0, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][m];
  };
  const result: [number, number][] = [];
  let mm = sMm;
  let dd = sDd;
  while (mm <= 12) {
    while (dd <= daysInMonth(mm)) {
      if (mm > eMm || (mm === eMm && dd > eDd)) return result;
      result.push([mm, dd]);
      dd++;
    }
    mm++;
    dd = 1;
  }
  return result;
}

// ── Seed derivation (matches Rust exactly) ──

function taskSeeds(yr: number, mm: number, dd: number, level: number, count: number): number[] {
  const dateKey = yr * 10000 + mm * 100 + dd;
  const seeds: number[] = [];
  for (let retry = 0; retry < count; retry++) {
    seeds.push(
      (Math.imul(dateKey, 31) + level) * 17 + Math.imul(retry, 0x9e3779b9),
    );
  }
  return seeds;
}

// ── Compact JSON serialization (matches Rust output) ──

function compactQuestionType(qt: QuestionType): Record<string, unknown> {
  const obj: Record<string, unknown> = {};
  if ("answer" in qt) obj.a = "ABCDE".indexOf(qt.answer);
  if ("questionIndex" in qt) obj.q = qt.questionIndex;
  if ("afterIndex" in qt) obj.q = qt.afterIndex;
  if ("beforeIndex" in qt) obj.q = qt.beforeIndex;
  obj.t = qt.type;
  return obj;
}

function compactClaim(claim: Claim): Record<string, unknown> {
  const obj = compactQuestionType(claim.questionType) as Record<string, unknown>;
  obj.v = claim.value;
  return obj;
}

function puzzleToJson(puzzle: Puzzle): Record<string, unknown> {
  const oc = puzzle.optionCount ?? 5;
  const questions = puzzle.questions.map((q) => {
    const obj: Record<string, unknown> = {};
    if (q.questionType.type === "TrueStmt") {
      obj.c = q.options.slice(0, oc).map((o) => ("claim" in o ? compactClaim(o.claim) : null));
    } else {
      obj.o = q.options.slice(0, oc).map((o) => o.value);
    }
    obj.t = compactQuestionType(q.questionType);
    return obj;
  });
  return { q: questions };
}

// ── Generation ──

const days = datesInYear(year, startMm, startDd, endMm, endDd);
const levels = levelFilter ? [levelFilter] : [1, 2, 3, 4, 5, 6];

console.error(`Generating ${days.length} days for year ${year} (${start}..${end})...`);
const t0 = performance.now();

const yearMap: Record<string, Record<string, unknown>> = {};
for (const [mm, dd] of days) {
  yearMap[`${String(mm).padStart(2, "0")}${String(dd).padStart(2, "0")}`] = {};
}

let okCount = 0;
let failCount = 0;
let totalAttempts = 0;

for (const [mm, dd] of days) {
  const key = `${String(mm).padStart(2, "0")}${String(dd).padStart(2, "0")}`;
  for (const level of levels) {
    const profile = profiles.find((p) => p.level === level);
    if (!profile) continue;

    const seeds = taskSeeds(year, mm, dd, level, 100);
    let result = null;

    for (const seed of seeds) {
      totalAttempts++;
      const rng = new RNG(seed);
      result = generate(profile, rng, maxAttempts);
      if (result) break;
    }

    if (result) {
      okCount++;
      result.puzzle.optionCount = profile.optionCount;
      yearMap[key][String(level)] = puzzleToJson(result.puzzle);
    } else {
      failCount++;
      console.error(`  FAILED: ${key} L${level}`);
    }
  }
}

const elapsed = (performance.now() - t0) / 1000;

console.error();
console.error("=== Summary ===");
console.error(`  Year:    ${year}`);
console.error(`  Start:   ${start}`);
console.error(`  Days:    ${days.length}`);
console.error(`  Puzzles: ${okCount}/${days.length * levels.length} (${failCount} failed)`);
console.error(`  Time:    ${elapsed.toFixed(1)}s (${((elapsed * 1000) / days.length).toFixed(1)}ms per day)`);
console.error(`  Output:  ${outputPath}`);
if (showStats) {
  console.error(`  Attempts: ${totalAttempts}`);
}

// ── Output ──

if (merge) {
  if (outputPath === "-") {
    console.error("--merge requires -o FILE (cannot merge to stdout)");
    process.exit(1);
  }
  let existing: Record<string, Record<string, unknown>> = {};
  try {
    existing = JSON.parse(readFileSync(outputPath, "utf8"));
  } catch {
    // file doesn't exist yet
  }
  for (const [date, levels] of Object.entries(yearMap)) {
    if (!existing[date]) existing[date] = {};
    for (const [lvl, puzzle] of Object.entries(levels)) {
      existing[date][lvl] = puzzle;
    }
  }
  writeFileSync(outputPath, JSON.stringify(existing));
} else {
  const out = JSON.stringify(yearMap);
  if (outputPath === "-") {
    process.stdout.write(out + "\n");
  } else {
    writeFileSync(outputPath, out);
  }
}
