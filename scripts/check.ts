import type { Puzzle } from "../src/engine/types.ts";
import { checkAnswer } from "../src/engine/check-answer.ts";
import { formatTypeTag } from "../src/engine/format.ts";
import { flattenPuzzle } from "../src/engine/types.ts";
import { isValid, type Validity } from "../src/engine/state.ts";
import { solvePuzzle } from "../src/engine/solve-deduce.ts";
import { solve } from "../src/generator/solve-brute.ts";
import { parseCompactYear } from "../src/puzzles/daily.ts";
import { checkForm } from "../src/engine/check-form.ts";
import { readFileSync } from "node:fs";
import { basename } from "node:path";

const LETTERS = "ABCDE";

const args = process.argv.slice(2);
const jsonOutput = args.includes("--json");
const positional = args.filter((a) => a !== "--json");
const [file, target] = positional;

if (!file) {
  console.error("Usage: node scripts/check.ts <file.json> [MMDD-level] [--json]");
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

interface ClaimInfo {
  label: string;
  text: string;
}

interface QuestionInfo {
  type_tag: string;
  options: (number | null)[];
  claims: ClaimInfo[] | null;
}

interface PuzzleCheckResult {
  key: string;
  n: number;
  option_count: number;
  questions: QuestionInfo[];
  form_warnings: string[];
  form_errors: string[];
  solve_ok: boolean;
  solve_answered: number;
  solve_steps: string[];
  brute_count: number;
  brute_solutions: string[];
  hint_brute_match: boolean;
  validity_ok: boolean;
  validity_per_question: string[];
}

interface CheckOutput {
  path: string;
  year: string;
  target: string | null;
  puzzles: PuzzleCheckResult[];
}

function countAnswered(steps: string[]): number {
  const set = new Set<string>();
  for (const s of steps) {
    const last = s[s.length - 1];
    if (last >= "A" && last <= "Z") {
      const qi = s.slice(0, -1);
      set.add(qi);
    }
  }
  return set.size;
}


function checkOnePuzzle(id: string, puzzle: Puzzle): PuzzleCheckResult {
  const fp = flattenPuzzle(puzzle);
  const n = fp.n;
  const oc = fp.optionCount;

  const { answers, eliminated, steps } = solvePuzzle(fp);
  const answered = countAnswered(steps);
  const solveOk = answers.slice(0, n).every((a) => a != null);

  const bruteSolutions = solve(puzzle, undefined, 10);
  const bruteCount = bruteSolutions.length;
  const bruteStrs = bruteSolutions.map((sol) => sol.join(""));

  const uniqueSolution = bruteSolutions.length === 1 ? bruteSolutions[0] : undefined;
  const formErrors = checkForm(puzzle, uniqueSolution);
  const formWarnings = formErrors
    .filter((e) => e.severity === "warning")
    .map((e) => `Q${e.qi + 1}: ${e.message}`);
  const formErrs = formErrors
    .filter((e) => e.severity === "error")
    .map((e) => `Q${e.qi + 1}: ${e.message}`);

  const hintBruteMatch =
    solveOk && bruteCount === 1
      ? answers.slice(0, n).every((a, i) => a === bruteSolutions[0][i])
      : true;

  const validityPerQuestion: string[] = [];
  let validityOk = true;
  for (let i = 0; i < n; i++) {
    if (solveOk) {
      const v: Validity = checkAnswer(fp, { answers, eliminated }, i);
      validityPerQuestion.push(v);
      if (!isValid(v) && v !== "pending") validityOk = false;
    } else {
      validityPerQuestion.push("n/a");
    }
  }

  const questions: QuestionInfo[] = [];
  for (let qi = 0; qi < n; qi++) {
    const q = puzzle.questions[qi];
    const qt = q.questionType;
    const typeTag = formatTypeTag(qt);
    const options: (number | null)[] = [];
    let claims: ClaimInfo[] | null = null;
    if (qt.type === "TrueStmt") {
      for (let oi = 0; oi < oc; oi++) options.push(null);
      claims = [];
      for (let oi = 0; oi < oc; oi++) {
        const label = LETTERS[oi];
        const claim = fp.optionClaims[qi][oi];
        const text = claim
          ? `${formatTypeTag(claim.questionType)} = ${claim.value}`
          : "null";
        claims.push({ label, text });
      }
    } else {
      for (let oi = 0; oi < oc; oi++) {
        options.push(fp.optionValues[qi][oi]);
      }
    }
    questions.push({ type_tag: typeTag, options, claims });
  }

  return {
    key: id,
    n,
    option_count: oc,
    questions,
    form_warnings: formWarnings,
    form_errors: formErrs,
    solve_ok: solveOk,
    solve_answered: answered,
    solve_steps: steps,
    brute_count: bruteCount,
    brute_solutions: bruteStrs,
    hint_brute_match: hintBruteMatch,
    validity_ok: validityOk,
    validity_per_question: validityPerQuestion,
  };
}

const puzzles: PuzzleCheckResult[] = entries.map((e) =>
  checkOnePuzzle(e.id, e.puzzle),
);

const output: CheckOutput = {
  path: file,
  year,
  target: target ?? null,
  puzzles,
};

if (jsonOutput) {
  process.stdout.write(JSON.stringify(output));
} else {
  // Legacy text output for direct use
  let total = 0;
  let solved = 0;
  const failures: string[] = [];

  for (const r of puzzles) {
    total++;
    if (r.solve_ok && r.brute_count === 1 && r.hint_brute_match && r.validity_ok) {
      solved++;
    } else {
      const mm = r.key.slice(0, 2);
      const dd = r.key.slice(2, 4);
      const lvl = r.key.split("-")[1];
      failures.push(
        `${r.key}: ${r.solve_answered}/${r.n} — http://localhost:5173/${year}-${mm}-${dd}/${lvl}?debug`,
      );
    }
  }

  if (!target) {
    console.error(`${solved}/${total} solved`);
    for (const f of failures) console.error(`  FAIL: ${f}`);
  } else if (puzzles.length === 1) {
    const r = puzzles[0];
    const status = r.solve_ok ? "solved" : r.solve_answered === r.n ? "INVALID" : "STUCK";
    console.error(`Hint engine: ${status} ${r.solve_answered}/${r.n} answered`);
    console.error(`  ${r.solve_steps.join(".")}`);
    console.error(`Brute-force: ${r.brute_count} solution(s)`);
    for (let i = 0; i < r.brute_solutions.length; i++) {
      console.error(`  #${i + 1}: ${r.brute_solutions[i]}`);
    }
  }
}
