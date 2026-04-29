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
const puzzle = parsed[day]?.[lvl];
if (!puzzle) { console.error("Not found"); process.exit(1); }
const n = puzzle.questions.length;

// Run findHint until stuck
const marks: Marks[] = puzzle.questions.map(
  () => ["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"] as Marks,
);
const answers: (string | null)[] = new Array(n).fill(null);

for (let step = 0; step < 300; step++) {
  if (answers.filter(a => a != null).length === n) break;
  const hint = findHint(puzzle, marks);
  if (!hint?.action) break;
  const a = hint.action;
  if (a.type === "force") {
    const oi = L2I[a.letter];
    answers[a.questionIndex] = a.letter;
    for (let j = 0; j < 5; j++) marks[a.questionIndex][j] = "incorrect";
    marks[a.questionIndex][oi] = "correct";
  } else if (a.type === "eliminate") {
    marks[a.questionIndex][a.optionIndex] = "incorrect";
  }
}

console.log("State:", answers.map((a, i) => a ?? marks[i].map((m: string, j: number) => m === "incorrect" ? "." : LETTERS[j]).join("")).join(" | "));
console.log("Answered:", answers.filter(a => a != null).length + "/" + n);

// Now check findActionFast directly
console.log("\nfindActionFast from stuck state:");
const action = findActionFast(puzzle, answers as any, marks, n);
console.log("  Result:", JSON.stringify(action));

// Try lookahead manually on remaining options
console.log("\nManual lookahead from stuck state:");
for (let qi = 0; qi < n; qi++) {
  if (answers[qi] != null) continue;
  for (let oi = 0; oi < 5; oi++) {
    if (marks[qi][oi] === "incorrect") continue;
    // Try this assumption
    const tAns = [...answers] as (string | null)[];
    const tMarks = marks.map(m => [...m] as unknown as Marks);
    tAns[qi] = LETTERS[oi];
    for (let j = 0; j < 5; j++) tMarks[qi][j] = "incorrect";
    tMarks[qi][oi] = "correct";

    let found = false;
    for (let iter = 0; iter < n * 5; iter++) {
      const act = findActionFast(puzzle, tAns as any, tMarks, n);
      if (!act) break;
      if (act.type === "contradiction") {
        console.log("  Q" + (qi+1) + "=" + LETTERS[oi] + " → contradiction Q" + (act.questionIndex+1) + " at iter " + iter);
        found = true;
        break;
      }
      if (act.type === "force") {
        tAns[act.questionIndex] = act.letter;
        const foi = L2I[act.letter];
        for (let j = 0; j < 5; j++) tMarks[act.questionIndex][j] = "incorrect";
        tMarks[act.questionIndex][foi] = "correct";
      } else if (act.type === "eliminate") {
        tMarks[act.questionIndex][act.optionIndex] = "incorrect";
      }
    }
    if (!found) {
      const tAnswered = tAns.filter(a => a != null).length;
      console.log("  Q" + (qi+1) + "=" + LETTERS[oi] + " → no contradiction (" + tAnswered + "/" + n + ")");
    }
  }
}
