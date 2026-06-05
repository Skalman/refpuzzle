import { writeFileSync, readFileSync, statSync } from "node:fs";
import { generateConstructive as generate } from "../src/generator/construct.ts";
import { profiles } from "../src/generator/difficulty.ts";
import { RNG } from "../src/generator/rng.ts";
import type { Puzzle, QuestionType } from "../src/engine/types.ts";

// ── CLI ──

const PROJECT_LAUNCH = "2026-04-19";

function printHelp(): never {
  console.error(`Usage: pnpm gen <date-range> -o FILE [options]

Date range formats:
  2051              full year (2051-01-01..2051-12-31)
  2051-03           full month (2051-03-01..2051-03-31)
  2051-03-15        single day
  2051-03..2051-06  month range (2051-03-01..2051-06-30)
  2051-01-01..2051-06-30  exact range

Options:
  -o FILE           output file (required, - for stdout)
  -m, --merge       merge into existing file
  --overwrite       overwrite existing output file
  -l, --level       generate only this level (1-6)
  -a, --attempts    max attempts per seed (default 100)
  --stats           show generation statistics
  --trace           show trace output

Examples:
  pnpm generate 2051 -o out.json
  pnpm generate 2051-03 -o out.json -l 4
  pnpm generate 2051-01..2051-06 -o out.json -m`);
  process.exit(0);
}

/** Return the last day of the given month. */
function lastDay(y: number, m: number): number {
  if (m === 2) return y % 4 === 0 && (y % 100 !== 0 || y % 400 === 0) ? 29 : 28;
  return [0, 31, 0, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][m];
}

/** Parse a date-range string and return [year, startDate, endDate]. */
function parseDateRange(raw: string): { year: number; start: string; end: string } {
  const parts = raw.split("..");

  function parseEndpoint(s: string, side: "start" | "end"): string {
    if (/^\d{4}$/.test(s)) {
      const y = Number(s);
      return side === "start" ? `${y}-01-01` : `${y}-12-31`;
    }
    if (/^\d{4}-\d{2}$/.test(s)) {
      const y = Number(s.slice(0, 4));
      const m = Number(s.slice(5, 7));
      if (m < 1 || m > 12) {
        console.error(`Invalid month in date range: ${s}`);
        return process.exit(1);
      }
      return side === "start" ? `${s}-01` : `${s}-${String(lastDay(y, m)).padStart(2, "0")}`;
    }
    if (/^\d{4}-\d{2}-\d{2}$/.test(s)) {
      return s;
    }
    console.error(`Invalid date format: ${s}`);
    return process.exit(1);
  }

  let start: string;
  let end: string;

  if (parts.length === 1) {
    start = parseEndpoint(parts[0], "start");
    end = parseEndpoint(parts[0], "end");
  } else if (parts.length === 2) {
    start = parseEndpoint(parts[0], "start");
    end = parseEndpoint(parts[1], "end");
  } else {
    console.error(`Invalid date range: ${raw}`);
    process.exit(1);
  }

  const startYear = Number(start.slice(0, 4));
  const endYear = Number(end.slice(0, 4));
  if (startYear !== endYear) {
    console.error(`Date range must not cross year boundaries: ${start}..${end}`);
    process.exit(1);
  }

  const year = startYear;

  // Backward compat: for year 2026, clamp start to project launch date
  if (year === 2026 && start < PROJECT_LAUNCH) {
    start = PROJECT_LAUNCH;
  }

  if (start < PROJECT_LAUNCH) {
    console.error(`Date range must not start before ${PROJECT_LAUNCH}: ${start}`);
    process.exit(1);
  }

  if (start > end) {
    console.error(`Start date is after end date: ${start}..${end}`);
    process.exit(1);
  }

  return { year, start, end };
}

const args = process.argv.slice(2);
let dateRangeArg: string | null = null;
let outputPath: string | null = null;
let merge = false;
let overwrite = false;
let levelFilter: number | null = null;
let maxAttempts = 100;
let showStats = false;
let tracing = false;

for (let i = 0; i < args.length; i++) {
  switch (args[i]) {
    case "--output":
    case "-o":
      outputPath = args[++i];
      break;
    case "--merge":
    case "-m":
      merge = true;
      break;
    case "--overwrite":
      overwrite = true;
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
    case "--trace":
      tracing = true;
      break;
    case "--help":
    case "-h":
      printHelp();
      break;
    default:
      if (args[i].startsWith("-")) {
        console.error(`Unknown option: ${args[i]}`);
        process.exit(1);
      }
      if (dateRangeArg !== null) {
        console.error(`Unexpected positional argument: ${args[i]}`);
        process.exit(1);
      }
      dateRangeArg = args[i];
  }
}

if (!dateRangeArg) {
  console.error("Error: date range argument is required");
  console.error("Run with --help for usage");
  process.exit(1);
}
if (!outputPath) {
  console.error("Error: -o/--output is required (use -o - for stdout)");
  process.exit(1);
}
if (merge && overwrite) {
  console.error("Error: --merge and --overwrite are mutually exclusive");
  process.exit(1);
}
if (
  outputPath !== "-" &&
  !merge &&
  !overwrite &&
  statSync(outputPath, { throwIfNoEntry: false })?.isFile()
) {
  console.error(
    `Error: output file ${outputPath} already exists. Pass --merge to add to it, or --overwrite to replace it.`,
  );
  process.exit(1);
}

const { year, start, end } = parseDateRange(dateRangeArg);
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
    seeds.push((Math.imul(dateKey, 31) + level) * 17 + Math.imul(retry, 0x9e3779b9));
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

function puzzleToJson(puzzle: Puzzle): Record<string, unknown> {
  const oc = puzzle.optionCount ?? 5;
  const qs = puzzle.questions.map((q) => compactQuestionType(q.questionType));
  const opts = puzzle.questions.map((q) => {
    // Identity-option types: emit letter indices verbatim (matches Rust).
    if (q.questionType.type === "NoOtherHasAnswer" || q.questionType.type === "AnswerIsSelf") {
      return Array.from({ length: oc }, (_, oi) => oi);
    }
    return q.options.slice(0, oc).map((o) => o.value);
  });
  // Alphabetical key order (o, q[, t]) matches Rust's serde_json BTreeMap.
  const out: Record<string, unknown> = { o: opts, q: qs };
  if (puzzle.trueStmtQuestionTypes) {
    out.t = puzzle.trueStmtQuestionTypes.slice(0, 5).map((qt) => compactQuestionType(qt));
  }
  return out;
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
      result = generate(profile, rng, maxAttempts, tracing, `${key}-${level}`);
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
console.error(
  `  Time:    ${elapsed.toFixed(1)}s (${((elapsed * 1000) / days.length).toFixed(1)}ms per day)`,
);
console.error(`  Output:  ${outputPath}`);
if (showStats) {
  console.error(`  Attempts: ${totalAttempts}`);
}

// ── Output ──

// Indent the date and level keys for readable git diffs; keep each puzzle on
// one compact line.
function formatYear(year: Record<string, Record<string, unknown>>): string {
  const dates = Object.keys(year).sort();
  let out = "{\n";
  dates.forEach((date, i) => {
    const levels = Object.keys(year[date]).sort();
    out += `  ${JSON.stringify(date)}: {\n`;
    levels.forEach((lvl, j) => {
      out += `    ${JSON.stringify(lvl)}: ${JSON.stringify(year[date][lvl])}`;
      out += j + 1 < levels.length ? ",\n" : "\n";
    });
    out += "  }";
    out += i + 1 < dates.length ? ",\n" : "\n";
  });
  out += "}\n";
  return out;
}

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
  writeFileSync(outputPath, formatYear(existing));
} else {
  const out = formatYear(yearMap);
  if (outputPath === "-") {
    process.stdout.write(out);
  } else {
    writeFileSync(outputPath, out);
  }
}
