import type { Answer, Puzzle } from "../src/engine/types.ts";
import { flattenPuzzle } from "../src/engine/types.ts";
import { checkQuestionAgainstSolution } from "../src/engine/evaluate.ts";
import { checkAnswerValidity } from "../src/engine/check-validity.ts";
import { V_VALID } from "../src/engine/state.ts";
import { solvePuzzle, checkPuzzleSolved } from "../src/engine/solve-deduce.ts";
import { solve } from "../src/generator/solve-brute.ts";
import { parseCompactYear } from "../src/puzzles/daily.ts";
import { validatePuzzleForm } from "../src/engine/validate-form.ts";
import { readFileSync } from "node:fs";
import { basename } from "node:path";

const [file, target] = process.argv.slice(2);
if (!file) {
  console.error("Usage: node scripts/check.ts <file.json> [MMDD-level]");
  process.exit(1);
}

const year = basename(file, ".json");
const parsed = parseCompactYear(JSON.parse(readFileSync(file, "utf8")));
const allPuzzles: { id: string; puzzle: Puzzle }[] = [];
for (const [mmdd, levels] of Object.entries(parsed)) {
  for (const [lvl, puzzle] of Object.entries(levels)) {
    allPuzzles.push({ id: `${mmdd}-${lvl}`, puzzle });
  }
}

const entries = target ? allPuzzles.filter((e) => e.id === target) : allPuzzles;

if (entries.length === 0) {
  console.error(`No puzzles matched${target ? ` "${target}"` : ""}`);
  process.exit(1);
}

let total = 0;
let solved = 0;
const failures: string[] = [];

for (const { id, puzzle } of entries) {
  total++;
  const formErrors = validatePuzzleForm(puzzle);
  if (formErrors.length > 0) {
    console.error(`${id}: FORM ERRORS`);
    for (const e of formErrors) console.error(`  Q${e.qi + 1}: ${e.message}`);
    failures.push(`${id} (form)`);
    continue;
  }
  const fp = flattenPuzzle(puzzle);
  const n = fp.n;
  const { answers, steps } = solvePuzzle(fp);
  const empty: number[] = new Array(n).fill(0);
  const ok = checkPuzzleSolved(fp, answers, empty);
  const answeredCount = answers.slice(0, n).filter((a) => a != null).length;

  if (target) {
    const status = ok ? "solved" : answeredCount === n ? "INVALID" : "STUCK";
    console.error(`Hint engine: ${status} ${answeredCount}/${n} answered`);
    console.error(`  ${steps.join(".")}`);

    const bruteSolutions = solve(puzzle, undefined, 10);
    console.error(`Brute-force: ${bruteSolutions.length} solution(s)`);
    for (let i = 0; i < bruteSolutions.length; i++) {
      console.error(`  #${i + 1}: ${bruteSolutions[i].join("")}`);
    }

    if (bruteSolutions.length >= 1) {
      const sol = bruteSolutions[0] as (Answer | null)[];
      console.error("Evaluate:");
      for (let qi = 0; qi < n; qi++) {
        const evalOk = checkQuestionAgainstSolution(fp.questions[qi], qi, sol[qi]!, sol, fp);
        const validOk = checkAnswerValidity(fp, sol, empty, qi);
        const match = evalOk === (validOk === V_VALID) ? "" : " ← MISMATCH";
        console.error(`  Q${qi + 1}: eval=${evalOk} validity=${validOk}${match}`);
      }
    }
  }

  if (ok) {
    solved++;
  } else {
    const mm = id.slice(0, 2);
    const dd = id.slice(2, 4);
    const lvl = id.split("-")[1];
    failures.push(`${id}: ${answeredCount}/${n} — http://localhost:5173/${year}-${mm}-${dd}/${lvl}?debug`);
  }
}

if (!target) {
  console.error(`${solved}/${total} solved`);
  for (const f of failures) console.error(`  FAIL: ${f}`);
}
