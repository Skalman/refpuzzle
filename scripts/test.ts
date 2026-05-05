import type { AnswerLetter, Puzzle, Marks } from "../src/engine/types.ts";
import { LETTERS, flattenPuzzle } from "../src/engine/types.ts";
import {
  checkQuestionAgainstSolution as evaluate,
  evaluateClaim,
} from "../src/engine/evaluators.ts";
import { checkAnswerValidity } from "../src/engine/check-validity.ts";
import { deduce } from "../src/engine/deduce.ts";
import { lookahead } from "../src/engine/lookahead.ts";
import { solve } from "../src/generator/solver.ts";
import { parseCompactYear } from "../src/puzzles/daily.ts";
import { readFileSync, readdirSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const dailyDir = resolve(__dirname, "../public/puzzles/daily");

const allPuzzles: Puzzle[] = [];
for (const file of readdirSync(dailyDir).filter((f: string) =>
  f.endsWith(".json"),
)) {
  const yearData = parseCompactYear(
    JSON.parse(readFileSync(resolve(dailyDir, file), "utf8")),
  );
  for (const dateKey of Object.keys(yearData)) {
    for (const [levelKey, puzzle] of Object.entries(yearData[dateKey])) {
      puzzle.id = `${file.replace(".json", "")}-${dateKey}-${levelKey}`;
      allPuzzles.push(puzzle);
    }
  }
}
import { encodeHistory, decodeHistory } from "../src/lib/store.ts";
import type { SavedState } from "../src/lib/store.ts";

let passed = 0;
let failed = 0;

function assert(condition: boolean, msg: string) {
  if (condition) {
    passed++;
  } else {
    failed++;
    console.error(`  FAIL: ${msg}`);
  }
}

function assertEq<T>(actual: T, expected: T, msg: string) {
  assert(
    JSON.stringify(actual) === JSON.stringify(expected),
    `${msg}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`,
  );
}

// ════════════════════════════════════════════════
// Evaluator tests
// ════════════════════════════════════════════════

function testEvaluators() {
  console.log("Evaluator tests...");

  const puzzle: Puzzle = {
    id: "test",
    title: "Test",
    difficulty: "1",
    questions: [
      {
        options: [
          { value: 0 },
          { value: 1 },
          { value: 2 },
          { value: 3 },
          { value: 4 },
        ],
        questionType: { type: "CountAnswer", answer: "B" },
      },
      {
        options: [
          { value: 0 },
          { value: 1 },
          { value: 2 },
          { value: 3 },
          { value: 4 },
        ],
        questionType: { type: "AnswerOf", questionIndex: 0 },
      },
      {
        options: [
          { value: 0 },
          { value: 1 },
          { value: 2 },
          { value: 3 },
          { value: null },
        ],
        questionType: { type: "ClosestAfter", afterIndex: 0, answer: "C" },
      },
      {
        options: [
          { value: 0 },
          { value: 1 },
          { value: 2 },
          { value: 3 },
          { value: 4 },
        ],
        questionType: { type: "LetterDist", questionIndex: 0 },
      },
    ],
  };
  const fp = flattenPuzzle(puzzle);

  // count_answer: [C, B, C, A] → count(B) = 1, option B = "1" ✓
  const answers: AnswerLetter[] = ["C", "B", "C", "A"];
  assert(
    evaluate(fp.questions[0], 0, "C", answers, fp) === false,
    "count_answer C: count(B)=1, optC='2', should be false",
  );
  assert(
    evaluate(fp.questions[0], 0, "B", answers, fp) === true,
    "count_answer B: count(B)=1, optB='1', should be true",
  );

  // answer_of_question: Q2 should match Q1's answer
  assert(
    evaluate(fp.questions[1], 1, "C", answers, fp) === true,
    "answer_of_question: Q1=C, selecting C → optC='C' matches",
  );
  assert(
    evaluate(fp.questions[1], 1, "A", answers, fp) === false,
    "answer_of_question: Q1=C, selecting A → optA='A' ≠ C",
  );

  // closest_after: closest C after Q1 (index 0) → Q3 (index 2, display 3)
  assert(
    evaluate(fp.questions[2], 2, "C", answers, fp) === true,
    "closest_after: closest C after #1 is Q3, optC='3' ✓",
  );
  assert(
    evaluate(fp.questions[2], 2, "A", answers, fp) === false,
    "closest_after: optA='1' but Q1 isn't C",
  );

  // letter_distance: Q4's selected answer vs Q1's answer (C)
  // If Q4=A: |A-C| = |0-2| = 2, optA='0' → 2≠0 ✗
  // If Q4=C: |C-C| = 0, optC='2' → 0≠2 ✗
  // If Q4=E: |E-C| = |4-2| = 2, optE='4' → 2≠4 ✗
  // If Q4=D: |D-C| = |3-2| = 1, optD='3' → 1≠3 ✗
  // If Q4=B: |B-C| = |1-2| = 1, optB='1' → 1=1 ✓
  assert(
    evaluate(fp.questions[3], 3, "B", answers, fp) === true,
    "letter_distance: |B-C| = 1, optB='1' ✓",
  );
  assert(
    evaluate(fp.questions[3], 3, "A", answers, fp) === false,
    "letter_distance: |A-C| = 2, optA='0' ✗",
  );

  // Test with partial answers (nulls)
  const partial: (AnswerLetter | null)[] = ["C", null, "C", null];
  assert(
    evaluate(fp.questions[0], 0, "A", partial, fp) === true,
    "count_answer partial: count(B)=0, optA='0' ✓",
  );

  // Test least_common_answer
  const lcPuzzle: Puzzle = {
    id: "lc",
    title: "LC",
    difficulty: "1",
    questions: [
      {
        options: [
          { value: 0 },
          { value: 1 },
          { value: 2 },
          { value: 3 },
          { value: 4 },
        ],
        questionType: { type: "LeastCommon" },
      },
      {
        options: [
          { value: 0 },
          { value: 1 },
          { value: 2 },
          { value: 3 },
          { value: 4 },
        ],
        questionType: { type: "AnswerIsSelf" },
      },
      {
        options: [
          { value: 0 },
          { value: 1 },
          { value: 2 },
          { value: 3 },
          { value: 4 },
        ],
        questionType: { type: "AnswerIsSelf" },
      },
    ],
  };
  const lcFp = flattenPuzzle(lcPuzzle);
  const lcAnswers: AnswerLetter[] = ["A", "B", "B"];
  // A=1, B=2, C=D=E=0. Three tied at 0 → no unique least → all false
  assert(
    evaluate(lcFp.questions[0], 0, "C", lcAnswers, lcFp) === false,
    "least_common: C=D=E tied at 0, no unique least → false",
  );
  assert(
    evaluate(lcFp.questions[0], 0, "A", lcAnswers, lcFp) === false,
    "least_common: A(1) > min(0), selecting A ✗",
  );
  assert(
    evaluate(lcFp.questions[0], 0, "B", lcAnswers, lcFp) === false,
    "least_common: B(2) > min(0), selecting B ✗",
  );

  // answer_is_self: always true
  assert(
    evaluate(lcFp.questions[1], 1, "A", lcAnswers, lcFp) === true,
    "answer_is_self: always true for A",
  );
  assert(
    evaluate(lcFp.questions[1], 1, "E", lcAnswers, lcFp) === true,
    "answer_is_self: always true for E",
  );

  // Test evaluateClaim
  const claimAnswers: AnswerLetter[] = ["A", "B", "C", "B", "A"];
  assert(
    evaluateClaim(
      { type: "CountAnswer", answer: "B", value: 2 },
      claimAnswers,
    ) === true,
    "claim count_answer B=2 ✓",
  );
  assert(
    evaluateClaim(
      { type: "CountAnswer", answer: "B", value: 3 },
      claimAnswers,
    ) === false,
    "claim count_answer B=3 ✗",
  );
  assert(
    evaluateClaim({ type: "CountVowel", value: 2 }, claimAnswers) ===
      true,
    "claim vowels=2 (A,A) ✓",
  );
  assert(
    evaluateClaim(
      { type: "CountConsonant", value: 3 },
      claimAnswers,
    ) === true,
    "claim consonants=3 (B,C,B) ✓",
  );
}

// ════════════════════════════════════════════════
// Solver tests
// ════════════════════════════════════════════════

function testSolver() {
  console.log("Solver tests...");

  // Simple puzzle with known unique solution
  const simple: Puzzle = {
    id: "s",
    title: "S",
    difficulty: "1",
    questions: [
      {
        options: [
          { value: 0 },
          { value: 1 },
          { value: 2 },
          { value: 3 },
          { value: 4 },
        ],
        questionType: { type: "AnswerOf", questionIndex: 1 },
      },
      {
        options: [
          { value: 0 },
          { value: 1 },
          { value: 2 },
          { value: 3 },
          { value: 4 },
        ],
        questionType: { type: "AnswerOf", questionIndex: 0 },
      },
      {
        options: [
          { value: 3 },
          { value: 0 },
          { value: 1 },
          { value: 2 },
          { value: 4 },
        ],
        questionType: { type: "CountAnswer", answer: "A" },
      },
    ],
  };
  // Q1=Q2 (mirror). Q3 counts A's.
  // If Q1=Q2=A: count(A)=2, opt A='3' → 2≠3 ✗
  // If Q1=Q2=C: count(A)=0, opt C='1' → 0≠1 ✗
  // If Q1=Q2=D: count(A)=0, opt D='2' → 0≠2 ✗
  // Need to check all combos...
  const solutions = solve(simple, undefined, 10);
  assert(solutions.length > 0, "simple puzzle has at least 1 solution");

  // Verify each solution is actually valid
  const fp = flattenPuzzle(simple);
  for (const sol of solutions) {
    const allValid = fp.questions.every((q, i) =>
      evaluate(q, i, sol[i], sol, fp),
    );
    assert(allValid, `solver solution [${sol.join(",")}] validates correctly`);
  }

  // Test with fixed answers
  if (solutions.length > 0) {
    const sol = solutions[0];
    const fixed: (AnswerLetter | null)[] = [sol[0], null, null];
    const constrained = solve(simple, fixed, 10);
    assert(
      constrained.length >= 1 && constrained.some((s) => s[0] === sol[0]),
      "solver with fixed answer includes the expected solution",
    );
  }
}

// ════════════════════════════════════════════════
// Naive brute-force solver (no pruning, for cross-validation)
// ════════════════════════════════════════════════

function bruteForce(puzzle: Puzzle, maxN = 8): AnswerLetter[][] {
  const n = puzzle.questions.length;
  if (n > maxN) return []; // too large for brute force
  const fp = flattenPuzzle(puzzle);
  const solutions: AnswerLetter[][] = [];
  const current: AnswerLetter[] = new Array(n).fill("A");

  function recurse(depth: number) {
    if (depth === n) {
      const valid = fp.questions.every((q, i) =>
        evaluate(q, i, current[i], current, fp),
      );
      if (valid) solutions.push([...current]);
      return;
    }
    for (const letter of LETTERS) {
      current[depth] = letter;
      recurse(depth + 1);
    }
  }

  recurse(0);
  return solutions;
}

// ════════════════════════════════════════════════
// Generated puzzle cross-validation
// ════════════════════════════════════════════════

function testGeneratedPuzzles() {
  console.log("Generated puzzle tests...");

  const puzzles = allPuzzles.map((p) => ({
    name: p.id,
    puzzle: p,
  }));

  let bruteCount = 0;
  for (let pi = 0; pi < puzzles.length; pi++) {
    const { name, puzzle } = puzzles[pi];
    // Solver finds exactly 1 solution
    const solutions = solve(puzzle, undefined, 2);
    assert(
      solutions.length === 1,
      `${name}: solver finds exactly 1 solution (found ${solutions.length})`,
    );

    if (solutions.length !== 1) continue;
    const sol = solutions[0];

    // Solution validates correctly
    const fp = flattenPuzzle(puzzle);
    const allValid = fp.questions.every((q, i) =>
      evaluate(q, i, sol[i], sol, fp),
    );
    assert(allValid, `${name}: solution [${sol.join(",")}] validates`);

    // Cross-validate with brute force for a sample of small puzzles
    if (puzzle.questions.length <= 8 && pi % 50 === 0) {
      bruteCount++;
      const t0 = performance.now();
      const bruteSolutions = bruteForce(puzzle);
      const elapsed = (performance.now() - t0).toFixed(0);
      assertEq(
        bruteSolutions.length,
        1,
        `${name}: brute force finds exactly 1 solution (${elapsed}ms)`,
      );
      if (bruteSolutions.length === 1) {
        assertEq(
          bruteSolutions[0],
          sol,
          `${name}: brute force solution matches solver`,
        );
      }
    }

    // Every question has unique rule
    const ruleKeys = new Set(
      puzzle.questions.map((q) => JSON.stringify(q.questionType)),
    );
    assert(
      ruleKeys.size === puzzle.questions.length,
      `${name}: all question rules are unique`,
    );

    // Every question has exactly 5 options
    for (let i = 0; i < puzzle.questions.length; i++) {
      assert(
        puzzle.questions[i].options.length === 5,
        `${name} Q${i + 1}: has 5 options`,
      );
    }

    // Options within each question are distinct (skip claim-based questions)
    for (let i = 0; i < puzzle.questions.length; i++) {
      if (puzzle.questions[i].questionType.type === "TrueStmt")
        continue;
      const values = puzzle.questions[i].options.map((o) =>
        JSON.stringify(o.value),
      );
      const unique = new Set(values);
      assert(
        unique.size === 5,
        `${name} Q${i + 1}: all option values are distinct (${values.join(", ")})`,
      );
    }
  }
  console.log(`  brute-forced ${bruteCount} of ${puzzles.length} puzzles`);
}

// ════════════════════════════════════════════════
// Solver edge cases
// ════════════════════════════════════════════════

function testSolverEdgeCases() {
  console.log("Solver edge cases...");

  // Puzzle with no solution
  const impossible: Puzzle = {
    id: "imp",
    title: "Imp",
    difficulty: "1",
    questions: [
      {
        options: [
          { value: 0 },
          { value: 1 },
          { value: 2 },
          { value: 3 },
          { value: 4 },
        ],
        questionType: { type: "AnswerOf", questionIndex: 1 },
      },
      {
        options: [
          { value: 1 },
          { value: 0 },
          { value: 3 },
          { value: 4 },
          { value: 2 },
        ],
        // Q2 mirrors Q1, but options are swapped so Q1=Q2 is impossible
        // Q1=A → optA='A' → Q2 must be A → Q2=A → optA='B' → Q1 must be B → contradiction
        questionType: { type: "AnswerOf", questionIndex: 0 },
      },
    ],
  };
  const impSol = solve(impossible, undefined, 5);
  // Check via brute force too
  const impBrute = bruteForce(impossible);
  assertEq(
    impSol.length,
    impBrute.length,
    "impossible puzzle: solver agrees with brute force",
  );

  // Puzzle with multiple solutions: two answer_is_self questions (any combo works)
  const multi: Puzzle = {
    id: "multi",
    title: "Multi",
    difficulty: "1",
    questions: [
      {
        options: [
          { value: 0 },
          { value: 1 },
          { value: 2 },
          { value: 3 },
          { value: 4 },
        ],
        questionType: { type: "AnswerIsSelf" },
      },
      {
        options: [
          { value: 0 },
          { value: 1 },
          { value: 2 },
          { value: 3 },
          { value: 4 },
        ],
        questionType: { type: "AnswerIsSelf" },
      },
    ],
  };
  const multiSol = solve(multi, undefined, 30);
  const multiBrute = bruteForce(multi);
  assert(
    multiBrute.length === 25,
    `answer_is_self x2: brute force finds 25 solutions (5x5)`,
  );
  assertEq(multiSol.length, 25, "multi-solution: solver finds all 25");
}

// ════════════════════════════════════════════════
// Hint engine tests
// ════════════════════════════════════════════════

const L2I: Record<string, number> = { A: 0, B: 1, C: 2, D: 3, E: 4 };

function blankState(n: number): {
  answers: (AnswerLetter | null)[];
  eliminated: number[];
} {
  return { answers: new Array(n).fill(null), eliminated: new Array(n).fill(0) };
}

function setCorrect(
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  qi: number,
  letter: AnswerLetter,
) {
  const oi = L2I[letter];
  eliminated[qi] = 0b11111 ^ (1 << oi);
  answers[qi] = letter;
}

function setEliminated(eliminated: number[], qi: number, letter: AnswerLetter) {
  eliminated[qi] |= 1 << L2I[letter];
}

function applyAction(
  action: import("../src/engine/deduce.ts").DeduceAction,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
) {
  if (action.type === "force") {
    const oi = L2I[action.letter];
    eliminated[action.questionIndex] = 0b11111 ^ (1 << oi);
    answers[action.questionIndex] = action.letter;
  } else if (action.type === "eliminate") {
    eliminated[action.questionIndex] |= 1 << action.optionIndex;
  } else if (action.type === "eliminateMulti") {
    for (let qi = 0; qi < answers.length; qi++) {
      if (action.questionMask & (1 << qi)) {
        eliminated[qi] |= action.optionMask;
      }
    }
  }
}

function testHints() {
  console.log("Hint engine tests...");

  // ── Contradiction: answer_of_question says Q1=B but Q1 is marked C ──
  const contradictionPuzzle: Puzzle = {
    id: "h1",
    title: "H1",
    difficulty: "1",
    questions: [
      {
        options: [
          { value: 0 },
          { value: 1 },
          { value: 2 },
          { value: 3 },
          { value: 4 },
        ],
        questionType: { type: "AnswerIsSelf" },
      },
      {
        options: [
          { value: 0 },
          { value: 1 },
          { value: 2 },
          { value: 3 },
          { value: 4 },
        ],
        questionType: { type: "AnswerOf", questionIndex: 0 },
      },
    ],
  };
  {
    const { answers, eliminated } = blankState(2);
    const fp = flattenPuzzle(contradictionPuzzle);
    setCorrect(answers, eliminated, 0, "C");
    setCorrect(answers, eliminated, 1, "B"); // claims Q1=B, but Q1=C
    const v = checkAnswerValidity(fp, answers, eliminated, 1);
    assert(v === "invalid", `contradiction: Q2 should be invalid (got ${v})`);
  }

  // ── Forced: answer_of_question when target is known ──
  {
    const { answers, eliminated } = blankState(2);
    const fp = flattenPuzzle(contradictionPuzzle);
    setCorrect(answers, eliminated, 0, "C"); // Q1 = C, so Q2 must be C
    const dr = deduce(fp, answers, eliminated);
    assert(dr.length > 0, "forced hint: deduce returns a result");
    assert(
      dr[0].action.type === "force",
      `forced hint: action type is force (got ${dr[0].action.type})`,
    );
    assert(
      dr[0].action.type === "force" && dr[0].action.letter === "C",
      "forced hint: forces letter C",
    );
  }

  // ── Forced by elimination: only one option left ──
  {
    const { answers, eliminated } = blankState(2);
    const fp = flattenPuzzle(contradictionPuzzle);
    setEliminated(eliminated, 0, "A");
    setEliminated(eliminated, 0, "B");
    setEliminated(eliminated, 0, "C");
    setEliminated(eliminated, 0, "D");
    // Only E remains for Q1
    const dr = deduce(fp, answers, eliminated);
    assert(dr.length > 0, "forced-by-elim: deduce returns a result");
    assert(
      dr[0].action.type === "force" && dr[0].action.letter === "E",
      "forced-by-elim: forces letter E",
    );
  }

  // ── Elimination: count_answer bounds ──
  const countPuzzle: Puzzle = {
    id: "h2",
    title: "H2",
    difficulty: "1",
    questions: [
      {
        options: [
          { value: 0 },
          { value: 1 },
          { value: 2 },
          { value: 3 },
          { value: 4 },
        ],
        questionType: { type: "CountAnswer", answer: "A" },
      },
      {
        options: [
          { value: 0 },
          { value: 1 },
          { value: 2 },
          { value: 3 },
          { value: 4 },
        ],
        questionType: { type: "AnswerIsSelf" },
      },
      {
        options: [
          { value: 0 },
          { value: 1 },
          { value: 2 },
          { value: 3 },
          { value: 4 },
        ],
        questionType: { type: "AnswerIsSelf" },
      },
    ],
  };
  {
    const { answers, eliminated } = blankState(3);
    const fp = flattenPuzzle(countPuzzle);
    setCorrect(answers, eliminated, 1, "A");
    setCorrect(answers, eliminated, 2, "A");
    const dr = deduce(fp, answers, eliminated);
    assert(dr.length > 0, "elimination: deduce returns a result");
    assert(
      dr[0].action.type === "eliminate" || dr[0].action.type === "force",
      `elimination: action is eliminate or force (got ${dr[0].action.type})`,
    );
  }

  // ── Forced counting: all questions answered → count is determined ──
  {
    const { answers, eliminated } = blankState(3);
    const fp = flattenPuzzle(countPuzzle);
    setCorrect(answers, eliminated, 1, "B");
    setCorrect(answers, eliminated, 2, "C");
    const dr = deduce(fp, answers, eliminated);
    assert(dr.length > 0, "count forced: deduce returns a result");
  }

  // ── Look-ahead: assumption leads to contradiction ──
  const lookaheadPuzzle: Puzzle = {
    id: "h3",
    title: "H3",
    difficulty: "1",
    questions: [
      {
        options: [
          { value: 0 },
          { value: 1 },
          { value: 2 },
          { value: 3 },
          { value: 4 },
        ],
        questionType: { type: "AnswerOf", questionIndex: 1 },
      },
      {
        options: [
          { value: 1 },
          { value: 0 },
          { value: 2 },
          { value: 3 },
          { value: 4 },
        ],
        questionType: { type: "AnswerOf", questionIndex: 0 },
      },
    ],
  };
  {
    const { answers, eliminated } = blankState(2);
    const fp = flattenPuzzle(lookaheadPuzzle);
    // No direct deduction possible — should need lookahead
    const dr = deduce(fp, answers, eliminated);
    if (dr.length > 0) {
      assert(true, "lookahead puzzle: deduce found something directly");
    } else {
      const lr = lookahead(fp, answers, eliminated);
      assert(lr != null, "lookahead: lookahead returns a result");
    }
  }

  // ── Solved puzzle: all valid, nothing to do ──
  {
    const allSelfPuzzle: Puzzle = {
      id: "h4",
      title: "H4",
      difficulty: "1",
      questions: [
        {
          options: [
            { value: 0 },
            { value: 1 },
            { value: 2 },
            { value: 3 },
            { value: 4 },
          ],
          questionType: { type: "AnswerIsSelf" },
        },
        {
          options: [
            { value: 0 },
            { value: 1 },
            { value: 2 },
            { value: 3 },
            { value: 4 },
          ],
          questionType: { type: "AnswerIsSelf" },
        },
      ],
    };
    const { answers, eliminated } = blankState(2);
    const fp = flattenPuzzle(allSelfPuzzle);
    setCorrect(answers, eliminated, 0, "C");
    setCorrect(answers, eliminated, 1, "C");
    const dr = deduce(fp, answers, eliminated);
    assert(dr.length === 0, "solved puzzle: deduce returns empty");
    const lr = lookahead(fp, answers, eliminated);
    assert(lr == null, "solved puzzle: lookahead returns null");
  }

  // ── Solvability: verify generated puzzles are solvable from blank ──
  for (const puzzle of allPuzzles.slice(0, 3)) {
    const n = puzzle.questions.length;
    const fp = flattenPuzzle(puzzle);
    const answers: (AnswerLetter | null)[] = new Array(n).fill(null);
    const eliminated: number[] = new Array(n).fill(0);
    let steps = 0;
    let stuck = false;

    while (!answers.every((a) => a != null) && steps < n * 15) {
      const dr = deduce(fp, answers, eliminated);
      if (dr.length > 0) {
        applyAction(dr[0].action, answers, eliminated);
        steps++;
        continue;
      }
      const lr = lookahead(fp, answers, eliminated);
      if (lr) {
        eliminated[lr.eliminateQi] |= 1 << lr.eliminateOi;
        steps++;
        continue;
      }
      stuck = true;
      break;
    }

    if (!stuck && answers.every((a) => a != null)) {
      const solutions = solve(puzzle, undefined, 2);
      assert(
        solutions.length === 1 &&
          JSON.stringify(answers) === JSON.stringify(solutions[0]),
        `${puzzle.id}: hint engine solves to the unique solution`,
      );
    } else {
      assert(false, `${puzzle.id}: hint engine got stuck after ${steps} steps`);
    }
  }
}

// ════════════════════════════════════════════════
// Share encode/decode roundtrip tests
// ════════════════════════════════════════════════

function mkState(steps: Marks[]): SavedState {
  const history: { marks: Marks }[][] = [
    steps.map(() => ({
      marks: [
        "unmarked",
        "unmarked",
        "unmarked",
        "unmarked",
        "unmarked",
      ] as Marks,
    })),
  ];
  let current = history[0].map((q) => ({ marks: [...q.marks] as Marks }));
  for (let qi = 0; qi < steps.length; qi++) {
    for (let oi = 0; oi < 5; oi++) {
      if (steps[qi][oi] === "unmarked") continue;
      current = current.map((q) => ({ marks: [...q.marks] as Marks }));
      current[qi].marks[oi] = steps[qi][oi];
      history.push(current.map((q) => ({ marks: [...q.marks] as Marks })));
    }
  }
  const last = history[history.length - 1];
  return {
    questions: last,
    completed: false,
    history,
    historyIdx: history.length - 1,
    hints: new Map(),
  };
}

function testShare() {
  console.log("Share encode/decode tests...");

  const marks: Marks[] = [
    ["correct", "incorrect", "unmarked", "unmarked", "unmarked"],
    ["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"],
    ["incorrect", "incorrect", "incorrect", "incorrect", "correct"],
  ];

  const state = mkState(marks);
  const encoded = encodeHistory(state);
  const decoded = decodeHistory(encoded, 3);
  assert(decoded != null, "decode: returns non-null");
  assertEq(
    decoded!.questions.map((q) => q.marks),
    state.questions.map((q) => q.marks),
    "decode: roundtrip marks match",
  );

  // All unmarked — single-step history, encode/decode should roundtrip
  const blankState = mkState([
    ["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"],
  ]);
  const blankEncoded = encodeHistory(blankState);
  const blankDecoded = decodeHistory(blankEncoded, 1);
  assert(blankDecoded != null, "decode: blank returns non-null");
  assertEq(
    blankDecoded!.questions.map((q) => q.marks),
    blankState.questions.map((q) => q.marks),
    "decode: all-blank roundtrip",
  );

  // All correct
  const correctState = mkState([
    ["correct", "correct", "correct", "correct", "correct"],
  ]);
  const correctEncoded = encodeHistory(correctState);
  const correctDecoded = decodeHistory(correctEncoded, 1);
  assert(correctDecoded != null, "decode: all-correct returns non-null");
  assertEq(
    correctDecoded!.questions.map((q) => q.marks),
    correctState.questions.map((q) => q.marks),
    "decode: all-correct roundtrip",
  );

  // Empty input — decodes to a blank starting state
  const emptyDecoded = decodeHistory("", 1);
  assert(emptyDecoded != null, "decode: empty string returns a blank state");
  assertEq(
    emptyDecoded!.questions.map((q) => q.marks),
    [["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"]],
    "decode: empty string produces all-unmarked marks",
  );
}

// ════════════════════════════════════════════════
// Shared check-validity cross-validation (TS ↔ Rust)
// ════════════════════════════════════════════════

function testSharedCheckValidity() {
  console.log("Shared check-validity tests (TS side)...");

  const suiteJson = JSON.parse(
    readFileSync(resolve(__dirname, "../tests/check-validity.json"), "utf8"),
  );
  const tests = suiteJson.tests as {
    section?: string;
    name?: string;
    qi?: number;
    puzzle?: {
      q: {
        t: Record<string, unknown>;
        o?: (number | null)[];
        c?: (Record<string, unknown> | null)[];
      }[];
    };
    state?: string[];
    expect?: string;
  }[];

  for (const t of tests) {
    if (t.section) continue;
    const {
      name,
      qi,
      puzzle: puz,
      state,
      expect,
    } = t as {
      name: string;
      qi: number;
      puzzle: {
        q: {
          t: Record<string, unknown>;
          o?: (number | null)[];
          c?: (Record<string, unknown> | null)[];
        }[];
      };
      state: string[];
      expect: string;
    };
    const n = puz.q.length;

    const questions = puz.q.map(
      (q): import("../src/engine/types.ts").QuestionDef => {
        const r = q.t as Record<string, unknown>;
        const type = r.t as string;
        const a = typeof r.a === "number" ? LETTERS[r.a as number] : undefined;
        const qIdx = r.q as number | undefined;

        let questionType: import("../src/engine/types.ts").QuestionTypeDef;
        switch (type) {
          case "CountAnswer":
            questionType = { type, answer: a! };
            break;
          case "CountAnswerBefore":
            questionType = { type, answer: a!, beforeIndex: qIdx! };
            break;
          case "CountAnswerAfter":
            questionType = { type, answer: a!, afterIndex: qIdx! };
            break;
          case "CountVowel":
          case "CountConsonant":
          case "MostCommonCount":
            questionType = {
              type,
            } as import("../src/engine/types.ts").QuestionTypeDef;
            break;
          case "ClosestAfter":
            questionType = { type, afterIndex: qIdx!, answer: a! };
            break;
          case "ClosestBefore":
            questionType = { type, beforeIndex: qIdx!, answer: a! };
            break;
          case "FirstWith":
          case "LastWith":
            questionType = { type, answer: a! };
            break;
          case "OnlyOdd":
          case "OnlyEven":
            questionType = {
              type,
              answer: a!,
            } as import("../src/engine/types.ts").QuestionTypeDef;
            break;
          case "AnswerOf":
            questionType = { type, questionIndex: qIdx! };
            break;
          case "LetterDist":
            questionType = { type, questionIndex: qIdx! };
            break;
          case "EqualCount":
            questionType = { type, answer: a! };
            break;
          default:
            questionType = {
              type,
            } as import("../src/engine/types.ts").QuestionTypeDef;
            break;
        }

        const options: import("../src/engine/types.ts").OptionDef[] = [];
        if (q.c) {
          for (const c of q.c) {
            if (c == null) {
              options.push({
                value: null,
                claim: { type: "CountAnswer", answer: "A", value: 0 },
              });
            } else {
              const ct = c.t as string;
              const ca =
                typeof c.a === "number" ? LETTERS[c.a as number] : undefined;
              const cv = c.v as number;
              let claim: import("../src/engine/types.ts").Claim;
              switch (ct) {
                case "CountAnswer":
                  claim = { type: ct, answer: ca!, value: cv };
                  break;
                case "CountConsonant":
                  claim = { type: ct, value: cv };
                  break;
                case "CountVowel":
                  claim = { type: ct, value: cv };
                  break;
                case "CountAnswerAfter":
                  claim = {
                    type: ct,
                    answer: ca!,
                    afterIndex: c.q as number,
                    value: cv,
                  };
                  break;
                case "CountAnswerBefore":
                  claim = {
                    type: ct,
                    answer: ca!,
                    beforeIndex: c.q as number,
                    value: cv,
                  };
                  break;
                case "AnswerOf":
                  claim = { type: ct, questionIndex: c.q as number, value: cv };
                  break;
                case "FirstWith":
                  claim = { type: ct, answer: ca!, value: cv };
                  break;
                case "LastWith":
                  claim = { type: ct, answer: ca!, value: cv };
                  break;
                case "MostCommon":
                  claim = { type: ct, value: cv };
                  break;
                default:
                  claim = { type: "CountAnswer", answer: "A", value: 0 };
                  break;
              }
              options.push({ value: null, claim });
            }
          }
        } else if (q.o) {
          for (const v of q.o) {
            options.push({ value: v });
          }
        }

        return { options, questionType };
      },
    );

    const puzzle: import("../src/engine/types.ts").Puzzle = {
      id: "cv",
      title: "CV",
      difficulty: "1",
      questions,
    };
    const fp = flattenPuzzle(puzzle);

    const answers: (import("../src/engine/types.ts").AnswerLetter | null)[] =
      new Array(n).fill(null);
    const eliminated: number[] = new Array(n).fill(0);
    for (let i = 0; i < n; i++) {
      const s = state[i];
      for (const ch of s) {
        if (ch >= "A" && ch <= "E") {
          const oi = ch.charCodeAt(0) - 65;
          answers[i] = LETTERS[oi];
          eliminated[i] = 0b11111 ^ (1 << oi);
        } else if (ch >= "a" && ch <= "e") {
          const oi = ch.charCodeAt(0) - 97;
          eliminated[i] |= 1 << oi;
        }
      }
    }

    const got = checkAnswerValidity(fp, answers, eliminated, qi);
    assert(
      got === expect,
      `shared check-validity: ${name}: expected ${expect}, got ${got}`,
    );
  }
}

// ════════════════════════════════════════════════
// Run all
// ════════════════════════════════════════════════

testEvaluators();
testSolver();
testSolverEdgeCases();
testGeneratedPuzzles();
testHints();
testShare();
testSharedCheckValidity();

console.log(`\n${passed} passed, ${failed} failed`);
if (failed > 0) process.exit(1);
