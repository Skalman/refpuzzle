import type { AnswerLetter, Puzzle, Marks } from "../src/engine/types.ts";
import { LETTERS, L2I, flattenPuzzle } from "../src/engine/types.ts";
import { checkQuestionAgainstSolution as evaluate } from "../src/engine/evaluators.ts";
import { checkAnswerValidity } from "../src/engine/check-validity.ts";
import { deduce, deduceWithRule, ALL_DEDUCE_RULES } from "../src/engine/deduce.ts";
import type { DeduceResult, DeduceRule } from "../src/engine/deduce.ts";
import { explainDeduce } from "../src/engine/explain.ts";
import { lookahead } from "../src/engine/lookahead.ts";
import { solve } from "../src/generator/solver.ts";
import { checkSolvable } from "../src/engine/solve.ts";
import type { SolveOutcome } from "../src/engine/solve.ts";
import { parseCompactYear } from "../src/puzzles/daily.ts";
import { readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";

const dailyDir = resolve(import.meta.dirname, "../public/puzzles/daily");

const allPuzzles: Puzzle[] = [];
for (const file of readdirSync(dailyDir).filter((f: string) => f.endsWith(".json"))) {
  const yearData = parseCompactYear(JSON.parse(readFileSync(resolve(dailyDir, file), "utf8")));
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

function testSharedEvaluators() {
  const suite = JSON.parse(
    readFileSync(resolve(import.meta.dirname, "../tests/evaluators.json"), "utf8"),
  );
  for (const test of suite.tests) {
    if ("section" in test) continue;
    const compact = test.puzzle;
    const wrapped = { "0101": { "1": compact } };
    const parsed = parseCompactYear(wrapped as Parameters<typeof parseCompactYear>[0]);
    const puzzle = parsed["0101"]["1"];
    const fp = flattenPuzzle(puzzle);
    const qi: number = test.qi;
    const answers: (AnswerLetter | null)[] = test.answers;
    const selected = answers[qi] as AnswerLetter;
    const got = evaluate(fp.questions[qi], qi, selected, answers, fp);
    assert(got === test.expect, `${test.name}: expected ${test.expect}, got ${got}`);
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
      const valid = fp.questions.every((q, i) => evaluate(q, i, current[i], current, fp));
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

function shuffle<T>(arr: T[]): T[] {
  const a = arr.slice();
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [a[i], a[j]] = [a[j], a[i]];
  }
  return a;
}

function testGeneratedSolver() {
  const shuffled = shuffle(allPuzzles);
  const deadline = performance.now() + 10_000;
  let count = 0;

  for (const puzzle of shuffled) {
    if (performance.now() > deadline) break;
    count++;

    const name = puzzle.id;
    const solutions = solve(puzzle, undefined, 2);
    assert(
      solutions.length === 1,
      `${name}: solver finds exactly 1 solution (found ${solutions.length})`,
    );

    if (solutions.length !== 1) continue;
    const sol = solutions[0];

    const fp = flattenPuzzle(puzzle);
    const allValid = fp.questions.every((q, i) => evaluate(q, i, sol[i], sol, fp));
    assert(allValid, `${name}: solution [${sol.join(",")}] validates`);

    const ruleKeys = new Set(puzzle.questions.map((q) => JSON.stringify(q.questionType)));
    assert(ruleKeys.size === puzzle.questions.length, `${name}: all question rules are unique`);

    for (let i = 0; i < puzzle.questions.length; i++) {
      assert(puzzle.questions[i].options.length === 5, `${name} Q${i + 1}: has 5 options`);
    }

    for (let i = 0; i < puzzle.questions.length; i++) {
      if (puzzle.questions[i].questionType.type === "TrueStmt") continue;
      const values = puzzle.questions[i].options.map((o) => JSON.stringify(o.value));
      const unique = new Set(values);
      assert(
        unique.size === 5,
        `${name} Q${i + 1}: all option values are distinct (${values.join(", ")})`,
      );
    }
  }
  console.log(`  ${count}/${allPuzzles.length} puzzles`);
}

function testGeneratedBruteForce() {
  const small = shuffle(allPuzzles.filter((p) => p.questions.length <= 8));
  const deadline = performance.now() + 10_000;
  let count = 0;

  for (const puzzle of small) {
    if (performance.now() > deadline) break;
    count++;

    const name = puzzle.id;
    const solutions = solve(puzzle, undefined, 2);
    if (solutions.length !== 1) continue;
    const sol = solutions[0];

    const bruteSolutions = bruteForce(puzzle);
    assertEq(bruteSolutions.length, 1, `${name}: brute force finds exactly 1 solution`);
    if (bruteSolutions.length === 1) {
      assertEq(bruteSolutions[0], sol, `${name}: brute force matches solver`);
    }
  }
  console.log(`  ${count}/${small.length} small puzzles`);
}

// ════════════════════════════════════════════════
// Solver edge cases
// ════════════════════════════════════════════════

function testSolverEdgeCases() {
  // Puzzle with no solution
  const impossible: Puzzle = {
    id: "imp",
    title: "Imp",
    difficulty: "1",
    questions: [
      {
        options: [{ value: 0 }, { value: 1 }, { value: 2 }, { value: 3 }, { value: 4 }],
        questionType: { type: "AnswerOf", questionIndex: 1 },
      },
      {
        options: [{ value: 1 }, { value: 0 }, { value: 3 }, { value: 4 }, { value: 2 }],
        // Q2 mirrors Q1, but options are swapped so Q1=Q2 is impossible
        // Q1=A → optA='A' → Q2 must be A → Q2=A → optA='B' → Q1 must be B → contradiction
        questionType: { type: "AnswerOf", questionIndex: 0 },
      },
    ],
  };
  const impSol = solve(impossible, undefined, 5);
  // Check via brute force too
  const impBrute = bruteForce(impossible);
  assertEq(impSol.length, impBrute.length, "impossible puzzle: solver agrees with brute force");

  // Puzzle with multiple solutions: two answer_is_self questions (any combo works)
  const multi: Puzzle = {
    id: "multi",
    title: "Multi",
    difficulty: "1",
    questions: [
      {
        options: [{ value: 0 }, { value: 1 }, { value: 2 }, { value: 3 }, { value: 4 }],
        questionType: { type: "AnswerIsSelf" },
      },
      {
        options: [{ value: 0 }, { value: 1 }, { value: 2 }, { value: 3 }, { value: 4 }],
        questionType: { type: "AnswerIsSelf" },
      },
    ],
  };
  const multiSol = solve(multi, undefined, 30);
  const multiBrute = bruteForce(multi);
  assert(multiBrute.length === 25, `answer_is_self x2: brute force finds 25 solutions (5x5)`);
  assertEq(multiSol.length, 25, "multi-solution: solver finds all 25");
}

// ════════════════════════════════════════════════
// Hint engine tests
// ════════════════════════════════════════════════

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
  // ── Contradiction: answer_of_question says Q1=B but Q1 is marked C ──
  const contradictionPuzzle: Puzzle = {
    id: "h1",
    title: "H1",
    difficulty: "1",
    questions: [
      {
        options: [{ value: 0 }, { value: 1 }, { value: 2 }, { value: 3 }, { value: 4 }],
        questionType: { type: "AnswerIsSelf" },
      },
      {
        options: [{ value: 0 }, { value: 1 }, { value: 2 }, { value: 3 }, { value: 4 }],
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
        options: [{ value: 0 }, { value: 1 }, { value: 2 }, { value: 3 }, { value: 4 }],
        questionType: { type: "CountAnswer", answer: "A" },
      },
      {
        options: [{ value: 0 }, { value: 1 }, { value: 2 }, { value: 3 }, { value: 4 }],
        questionType: { type: "AnswerIsSelf" },
      },
      {
        options: [{ value: 0 }, { value: 1 }, { value: 2 }, { value: 3 }, { value: 4 }],
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
        options: [{ value: 0 }, { value: 1 }, { value: 2 }, { value: 3 }, { value: 4 }],
        questionType: { type: "AnswerOf", questionIndex: 1 },
      },
      {
        options: [{ value: 1 }, { value: 0 }, { value: 2 }, { value: 3 }, { value: 4 }],
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
          options: [{ value: 0 }, { value: 1 }, { value: 2 }, { value: 3 }, { value: 4 }],
          questionType: { type: "AnswerIsSelf" },
        },
        {
          options: [{ value: 0 }, { value: 1 }, { value: 2 }, { value: 3 }, { value: 4 }],
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
    const oc = puzzle.optionCount ?? 5;
    const phantomMask = 0b11111 & ~((1 << oc) - 1);
    const answers: (AnswerLetter | null)[] = new Array(n).fill(null);
    const eliminated: number[] = new Array(n).fill(phantomMask);
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
        solutions.length === 1 && JSON.stringify(answers) === JSON.stringify(solutions[0]),
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
      marks: ["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"] as Marks,
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
  const blankState = mkState([["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"]]);
  const blankEncoded = encodeHistory(blankState);
  const blankDecoded = decodeHistory(blankEncoded, 1);
  assert(blankDecoded != null, "decode: blank returns non-null");
  assertEq(
    blankDecoded!.questions.map((q) => q.marks),
    blankState.questions.map((q) => q.marks),
    "decode: all-blank roundtrip",
  );

  // All correct
  const correctState = mkState([["correct", "correct", "correct", "correct", "correct"]]);
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
  const suiteJson = JSON.parse(
    readFileSync(resolve(import.meta.dirname, "../tests/check-validity.json"), "utf8"),
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

    const questions = puz.q.map((q): import("../src/engine/types.ts").QuestionDef => {
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
        case "SameAsWhich":
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
              claim: {
                questionType: { type: "CountAnswer", answer: "A" },
                value: 0,
              },
            });
          } else {
            const ct = c.t as string;
            const ca = typeof c.a === "number" ? LETTERS[c.a as number] : undefined;
            const cv = c.v as number;
            let qt: import("../src/engine/types.ts").QuestionTypeDef;
            switch (ct) {
              case "CountAnswer":
                qt = { type: ct, answer: ca! };
                break;
              case "CountConsonant":
              case "CountVowel":
              case "MostCommon":
                qt = { type: ct };
                break;
              case "CountAnswerAfter":
                qt = { type: ct, answer: ca!, afterIndex: c.q as number };
                break;
              case "CountAnswerBefore":
                qt = { type: ct, answer: ca!, beforeIndex: c.q as number };
                break;
              case "AnswerOf":
                qt = { type: ct, questionIndex: c.q as number };
                break;
              case "FirstWith":
              case "LastWith":
                qt = { type: ct, answer: ca! };
                break;
              default:
                qt = { type: "CountAnswer", answer: "A" };
                break;
            }
            options.push({ value: null, claim: { questionType: qt, value: cv } });
          }
        }
      } else if (q.o) {
        for (const v of q.o) {
          options.push({ value: v });
        }
      }

      return { options, questionType };
    });

    const puzzle: import("../src/engine/types.ts").Puzzle = {
      id: "cv",
      title: "CV",
      difficulty: "1",
      questions,
    };
    const fp = flattenPuzzle(puzzle);

    const answers: (import("../src/engine/types.ts").AnswerLetter | null)[] = new Array(n).fill(
      null,
    );
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
    assert(got === expect, `shared check-validity: ${name}: expected ${expect}, got ${got}`);
  }
}

// ════════════════════════════════════════════════
// Shared lookahead cross-validation (TS ↔ Rust)
// ════════════════════════════════════════════════

function testSharedLookahead() {
  const suiteJson = JSON.parse(readFileSync(resolve(import.meta.dirname, "../tests/lookahead.json"), "utf8"));
  const tests = suiteJson.tests as (
    | { section: string }
    | {
        name: string;
        puzzle: Record<string, unknown>;
        state: string[];
        expect: string | null;
      }
  )[];

  for (const t of tests) {
    if ("section" in t) continue;
    const { name, puzzle: compact, state, expect } = t;

    // eslint-disable-next-line @typescript-eslint/no-explicit-any -- JSON fixture
    const wrapped = { "0101": { "1": compact } } as any;
    const parsed = parseCompactYear(wrapped);
    const puzzle = parsed["0101"]["1"];
    const fp = flattenPuzzle(puzzle);
    const n = puzzle.questions.length;

    const answers: (AnswerLetter | null)[] = new Array(n).fill(null);
    const eliminated: number[] = new Array(n).fill(0);
    for (let i = 0; i < n; i++) {
      const s = state[i] || "";
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

    const result = lookahead(fp, answers, eliminated);
    const got = result ? `${result.eliminateQi + 1}${"abcde"[result.eliminateOi]}` : null;
    const gotStr = got === null ? "null" : got;
    const expectStr = expect === null ? "null" : expect;
    assert(gotStr === expectStr, `shared lookahead: ${name}: expected ${expectStr}, got ${gotStr}`);
  }
}

// ════════════════════════════════════════════════
// Shared solve cross-validation (TS ↔ Rust)
// ════════════════════════════════════════════════

function testSharedSolve() {
  const suiteJson = JSON.parse(readFileSync(resolve(import.meta.dirname, "../tests/solve.json"), "utf8"));
  const tests = suiteJson.tests as (
    | { section: string }
    | {
        name: string;
        puzzle: Record<string, unknown>;
        expect: SolveOutcome;
      }
  )[];

  for (const t of tests) {
    if ("section" in t) continue;
    const { name, puzzle: compact, expect } = t;

    // eslint-disable-next-line @typescript-eslint/no-explicit-any -- JSON fixture
    const wrapped = { "0101": { "1": compact } } as any;
    const parsed = parseCompactYear(wrapped);
    const puzzle = parsed["0101"]["1"];
    const fp = flattenPuzzle(puzzle);

    const got = checkSolvable(fp);
    assert(got === expect, `shared solve: ${name}: expected ${expect}, got ${got}`);
  }
}

// ════════════════════════════════════════════════
// Shared deduce tests (correctness + explanations + DRY + coverage)
// ════════════════════════════════════════════════

function testSharedDeduce() {
  const suite = JSON.parse(readFileSync(resolve(import.meta.dirname, "../tests/deduce.json"), "utf8"));
  const coveredRules = new Set<string>();

  function formatAction(dr: DeduceResult | undefined): string {
    if (!dr) return "null";
    const a = dr.action;
    if (a.type === "force") return `${a.questionIndex + 1}${a.letter}`;
    if (a.type === "eliminate") return `${a.questionIndex + 1}${"abcde"[a.optionIndex]}`;
    if (a.type === "eliminateMulti")
      return `qm${a.questionMask.toString(2)}o${a.optionMask.toString(2).padStart(5, "0")}`;
    return "null";
  }

  function parsePuzzle(compact: Record<string, unknown>) {
    const wrapped = { "0101": { "1": compact } } as unknown as Parameters<
      typeof parseCompactYear
    >[0];
    return parseCompactYear(wrapped)["0101"]["1"];
  }

  function applyState(
    n: number,
    state: string[],
  ): { answers: (AnswerLetter | null)[]; eliminated: number[] } {
    const answers: (AnswerLetter | null)[] = new Array(n).fill(null);
    const eliminated: number[] = new Array(n).fill(0);
    for (let qi = 0; qi < n; qi++) {
      const s = state[qi] || "";
      for (const ch of s) {
        if (ch >= "A" && ch <= "E") {
          answers[qi] = ch as AnswerLetter;
          eliminated[qi] = 0b11111 ^ (1 << L2I[ch]);
        } else if (ch >= "a" && ch <= "e") {
          eliminated[qi] |= 1 << L2I[ch.toUpperCase()];
        }
      }
    }
    return { answers, eliminated };
  }

  for (const test of suite.tests) {
    if ("section" in test) continue;
    const { name, state, expect, rule: ruleStr } = test;
    const puzzle = parsePuzzle(test.puzzle);
    const fp = flattenPuzzle(puzzle);
    const n = puzzle.questions.length;
    const { answers, eliminated } = applyState(n, state);

    const parsedRule: DeduceRule | null =
      ruleStr && ALL_DEDUCE_RULES.includes(ruleStr) ? ruleStr : null;
    if (parsedRule) coveredRules.add(parsedRule);

    const results = parsedRule
      ? deduceWithRule(fp, answers, eliminated, parsedRule)
      : deduce(fp, answers, eliminated);
    const got = formatAction(results[0]);
    const expected = expect ?? "null";
    assert(got === expected, `deduce: ${name}: expected ${expected}, got ${got}`);

    // Explain check
    if (results[0] && got === expected) {
      try {
        const steps = explainDeduce(puzzle, fp, answers, eliminated, results[0]);
        const hasFallback = steps.some(
          (s) =>
            s.type === "simple" &&
            (/^Q\d+ can't be [A-E]\.$/.test(s.text) ||
              /^Q\d+ options? [A-E, ]+ can be ruled out\.$/.test(s.text)),
        );
        assert(!hasFallback, `deduce explain fallback: ${name}`);
      } catch (e) {
        assert(false, `deduce explain threw: ${name}: ${String(e)}`);
      }
    }

    // DRY check
    if (parsedRule && results[0] && got === expected) {
      const without = deduceWithRule(fp, answers, eliminated, null, parsedRule);
      const withoutGot = formatAction(without[0]);
      assert(
        withoutGot !== got,
        `deduce DRY: ${name}: excluding "${parsedRule}" still produces ${got}`,
      );
    }
  }

  const uncovered = ALL_DEDUCE_RULES.filter((r) => !coveredRules.has(r));
  assert(uncovered.length === 0, `deduce: missing test coverage for: ${uncovered.join(", ")}`);
}

// ════════════════════════════════════════════════
// Run all
// ════════════════════════════════════════════════

const slow = process.argv.includes("--all");

function timed(name: string, fn: () => void) {
  const t0 = performance.now();
  fn();
  console.log(`  [${name}] ${(performance.now() - t0).toFixed(0)}ms`);
}

timed("Shared evaluator tests", testSharedEvaluators);
timed("Shared deduce tests", testSharedDeduce);
timed("Shared check-validity tests", testSharedCheckValidity);
timed("Shared lookahead tests", testSharedLookahead);
timed("Shared solve tests", testSharedSolve);
timed("Solver edge cases", testSolverEdgeCases);
timed("Share encode/decode tests", testShare);

timed("Hint engine tests", testHints);
if (slow) {
  timed("Generated puzzles: solver", testGeneratedSolver);
  timed("Generated puzzles: brute-force", testGeneratedBruteForce);
} else {
  console.log("Skipping generated puzzle tests (use --all to include)");
}

console.log(`\n${passed} passed, ${failed} failed`);
if (failed > 0) process.exit(1);
