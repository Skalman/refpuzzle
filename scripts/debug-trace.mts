import { readFileSync } from "fs";
import { parseCompactYear } from "../src/puzzles/daily.ts";
import type { Marks } from "../src/engine/types.ts";
import { LETTERS, L2I } from "../src/engine/types.ts";
import { findHint, findActionFast } from "../src/engine/hints.ts";

const raw = JSON.parse(readFileSync(process.argv[2], "utf8"));
const parsed = parseCompactYear(raw);
const arg = process.argv[3] || "";
const day = arg.slice(0, 4);
const lvl = arg.slice(5) || "level-5";
const puzzle = parsed[day]?.[lvl || "level-5"];
if (!puzzle) { console.error("Not found"); process.exit(1); }
const n = puzzle.questions.length;

const marks: Marks[] = puzzle.questions.map(
  () => ["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"] as Marks,
);
const answers: (string | null)[] = new Array(n).fill(null);

for (let step = 0; step < 300; step++) {
  if (answers.filter(a => a != null).length === n) { console.log("SOLVED at step " + step); break; }
  const hint = findHint(puzzle, marks);
  const action = hint?.action;
  if (!action) {
    console.log("STUCK at step " + step);
    for (let qi = 0; qi < n; qi++) {
      const rem = marks[qi].map((m: string, i: number) => m === "incorrect" ? "." : LETTERS[i]).join("");
      console.log("  Q" + (qi+1) + ": " + (answers[qi] ?? rem));
    }
    break;
  }
  if (action.type === "contradiction") {
    console.log("Step " + step + ": CONTRADICTION Q" + (action.questionIndex+1));
    break;
  }
  if (action.type === "force") {
    const oi = L2I[action.letter];
    answers[action.questionIndex] = action.letter;
    for (let j = 0; j < 5; j++) marks[action.questionIndex][j] = "incorrect";
    marks[action.questionIndex][oi] = "correct";
    console.log("Step " + step + ": " + (action.questionIndex+1) + action.letter);
  } else if (action.type === "eliminate") {
    marks[action.questionIndex][action.optionIndex] = "incorrect";
    const letters = "abcde";
    console.log("Step " + step + ": " + (action.questionIndex+1) + letters[action.optionIndex]);
  }
}
