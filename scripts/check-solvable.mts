#!/usr/bin/env node --experimental-transform-types
/**
 * Check that the JS hint engine can solve all puzzles in a compact JSON file.
 *
 * Usage:
 *   node --experimental-transform-types scripts/check-solvable.mts <file.json> [day-level]
 *
 * Batch mode (no day-level):
 *   242/257 solved
 *   Failed (15):
 *     1009-level-5: 0/12
 *     ...
 *
 * Single puzzle mode (e.g. 0803-level-5):
 *   1a.2c.2d.3b.3e._3A.4b...
 *   (exits 1 if not fully solved)
 */
import { readFileSync } from "fs";
import { parseCompactYear } from "../src/puzzles/daily.ts";
import type { Marks } from "../src/engine/types.ts";
import { LETTERS } from "../src/engine/types.ts";
import { findHint } from "../src/engine/hints.ts";

const file = process.argv[2];
if (!file) {
  console.error("Usage: check-solvable.mts <file.json> [day-level]");
  process.exit(1);
}

const raw = JSON.parse(readFileSync(file, "utf8"));
const parsed = parseCompactYear(raw);
const target = process.argv[3];

interface SolveResult {
  ok: boolean;
  answered: number;
  total: number;
  steps: string[];
}

function solvePuzzle(puzzle: { questions: { rule: any; options: any[] }[] }): SolveResult {
  const n = puzzle.questions.length;
  const marks: Marks[] = puzzle.questions.map(
    () => ["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"] as Marks,
  );
  const steps: string[] = [];
  const letters = "abcde";

  for (let step = 0; step < n * 30; step++) {
    if (marks.filter((m) => m.includes("correct")).length === n)
      return { ok: true, answered: n, total: n, steps };
    const hint = findHint(puzzle, marks);
    if (!hint?.action) break;
    const a = hint.action;
    if (a.type === "eliminate") {
      marks[a.questionIndex][a.optionIndex] = "incorrect";
      steps.push(`${a.questionIndex + 1}${letters[a.optionIndex]}`);
    } else if (a.type === "force") {
      const oi = LETTERS.indexOf(a.letter);
      marks[a.questionIndex][oi] = "correct";
      for (let j = 0; j < 5; j++) {
        if (j !== oi && marks[a.questionIndex][j] === "unmarked")
          marks[a.questionIndex][j] = "incorrect";
      }
      steps.push(`${a.questionIndex + 1}${a.letter}`);
    }
  }
  const answered = marks.filter((m) => m.includes("correct")).length;
  return { ok: answered === n, answered, total: n, steps };
}

if (target) {
  const sep = target.indexOf("-");
  const day = sep >= 0 ? target.slice(0, sep) : target;
  const lvl = sep >= 0 ? target.slice(sep + 1) : "level-5";
  const entry = Object.entries(parsed).find(([d]) => d === day);
  if (!entry) { console.error("Day not found:", day); process.exit(1); }
  const puzzle = entry[1][lvl];
  if (!puzzle) { console.error("Level not found:", lvl); process.exit(1); }
  const result = solvePuzzle(puzzle);
  console.log(result.steps.join("."));
  if (!result.ok) {
    console.error(`FAILED: ${result.answered}/${result.total}`);
    process.exit(1);
  }
} else {
  let total = 0, solved = 0;
  const failures: string[] = [];
  for (const [day, levels] of Object.entries(parsed)) {
    for (const [lvl, puzzle] of Object.entries(levels)) {
      total++;
      const result = solvePuzzle(puzzle);
      if (result.ok) solved++;
      else failures.push(`${day}-${lvl}: ${result.answered}/${result.total}`);
    }
  }
  console.log(`${solved}/${total} solved`);
  if (failures.length > 0) {
    console.log(`\nFailed (${failures.length}):`);
    for (const f of failures) console.log("  " + f);
  }
  process.exit(failures.length > 0 ? 1 : 0);
}
