import { readFileSync } from "fs";
import { parseCompactYear } from "../src/puzzles/daily.ts";
import type { Marks } from "../src/engine/types.ts";
import { LETTERS, L2I, flattenPuzzle } from "../src/engine/types.ts";

const raw = JSON.parse(readFileSync(process.argv[2], "utf8"));
const parsed = parseCompactYear(raw);
const arg = process.argv[3] || "";
const day = arg.slice(0, 4);
const lvl = arg.slice(5) || "level-5";
const puzzle = parsed[day]?.[lvl];
if (!puzzle) { console.error("Not found"); process.exit(1); }
const n = puzzle.questions.length;
const fp = flattenPuzzle(puzzle);

// Check: for every answered question, does the PrevSame contradiction check give false positives?
// Simulate: answer each question with every possible letter and check
for (let qi = 0; qi < n; qi++) {
  const r = fp.rules[qi];
  if (r.t !== 16) continue; // RT_PREV_SAME = ? let me check by name
  console.log("Q" + (qi+1) + ": previous_same_answer");
  for (let oi = 0; oi < 5; oi++) {
    const v = fp.optionValues[qi][oi];
    const letter = LETTERS[oi];
    console.log("  option " + letter + ": value=" + v + (v == null ? " (None)" : " (Q" + (v+1) + ")"));
  }
}
