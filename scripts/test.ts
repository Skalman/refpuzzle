import type { Answer, Puzzle, Marks, QuestionTypeName } from "../src/engine/types.ts";
import { ALL_QUESTION_TYPE_NAMES, LETTERS, L2I, flattenPuzzle } from "../src/engine/types.ts";
import { checkAnswer, checkAnswers } from "../src/engine/check-answer.ts";
import { isValid } from "../src/engine/state.ts";
import { deduceAssumingUnique, deduceWithRule, ALL_DEDUCE_RULES } from "../src/engine/deduce.ts";
import type { DeduceResult, DeduceRule } from "../src/engine/deduce.ts";
import { explainDeduce, explainLookahead } from "../src/engine/explain.ts";
import { lookahead } from "../src/engine/lookahead.ts";
import { solve } from "../src/generator/solve-brute.ts";
import { solvePuzzle } from "../src/engine/solve-deduce.ts";
import type { SolveOutcome } from "../src/engine/solve-deduce.ts";
import { parseCompactYear, expandQuestion } from "../src/puzzles/daily.ts";
import { checkForm } from "../src/engine/check-form.ts";
import { fillOptions, validValues } from "../src/generator/construct.ts";
import type { ConstructResult } from "../src/generator/construct.ts";
import { RNG } from "../src/generator/rng.ts";
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
    readFileSync(resolve(import.meta.dirname, "../tests/evaluate.json"), "utf8"),
  );
  for (const test of suite.tests) {
    if ("section" in test) continue;
    const compact = test.puzzle;
    const wrapped = { "0101": { "1": compact } };
    const parsed = parseCompactYear(wrapped as Parameters<typeof parseCompactYear>[0]);
    const puzzle = parsed["0101"]["1"];
    const fp = flattenPuzzle(puzzle);
    const qi: number = test.qi;
    const answers: (Answer | null)[] = test.answers;
    const got = isValid(checkAnswer(fp, { answers, eliminated: new Array(fp.n).fill(0) }, qi));
    assert(got === test.expect, `${test.name}: expected ${test.expect}, got ${got}`);
  }
}

// ════════════════════════════════════════════════
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

function testGeneratedUniqueSolution() {
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
    const allValid = checkAnswers(fp, sol);
    assert(allValid, `${name}: solution [${sol.join(",")}] validates`);

    const isPast = puzzleIsPast(name);
    const formErrors = checkForm(puzzle, sol);
    for (const e of formErrors) {
      if (e.severity === "warning" && isPast) continue;
      assert(false, `${name} Q${e.qi + 1}: ${e.severity}: ${e.message}`);
    }

    const ruleKeys = new Set(puzzle.questions.map((q) => JSON.stringify(q.questionType)));
    assert(ruleKeys.size === puzzle.questions.length, `${name}: all question rules are unique`);

    const oc = puzzle.optionCount ?? 5;
    for (let i = 0; i < puzzle.questions.length; i++) {
      assert(puzzle.questions[i].options.length === oc, `${name} Q${i + 1}: has ${oc} options`);
    }

    for (let i = 0; i < puzzle.questions.length; i++) {
      if (puzzle.questions[i].questionType.type === "TrueStmt") continue;
      const values = puzzle.questions[i].options.map((o) => JSON.stringify(o.value));
      const unique = new Set(values);
      assert(
        unique.size === oc,
        `${name} Q${i + 1}: all option values are distinct (${values.join(", ")})`,
      );
    }
  }
  console.log(`  ${count}/${allPuzzles.length} puzzles`);
}

const today = (() => {
  const now = new Date();
  return now.getFullYear() * 10000 + (now.getMonth() + 1) * 100 + now.getDate();
})();

function puzzleIsPast(id: string): boolean {
  // id format: "YYYY-MMDD-level". Returns false (treat as not-past) when
  // the year isn't a 4-digit number.
  const parts = id.split("-");
  if (parts.length !== 3 || !/^\d{4}$/.test(parts[0])) return false;
  return parseInt(parts[0], 10) * 10000 + parseInt(parts[1], 10) < today;
}

function testGeneratedWellformed() {
  for (const puzzle of allPuzzles) {
    const isPast = puzzleIsPast(puzzle.id);
    const errors = checkForm(puzzle);
    for (const e of errors) {
      if (e.severity === "warning" && isPast) continue;
      assert(false, `${puzzle.id} Q${e.qi + 1}: ${e.severity}: ${e.message}`);
    }
  }
  console.log(`  ${allPuzzles.length} puzzles`);
}

function testGeneratedHintSolvable() {
  const shuffled = shuffle(allPuzzles);
  const deadline = performance.now() + 10_000;
  let count = 0;

  for (const puzzle of shuffled) {
    if (performance.now() > deadline) break;
    count++;

    const fp = flattenPuzzle(puzzle);
    const { answers } = solvePuzzle(fp);
    const ok = answers.slice(0, fp.n).every((a) => a != null);
    assert(ok, `${puzzle.id}: hint engine solves`);
  }
  console.log(`  ${count}/${allPuzzles.length} puzzles`);
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
  assertEq(impSol.length, 0, "impossible puzzle: solver finds 0 solutions");

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
  assertEq(multiSol.length, 25, "multi-solution: solver finds all 25 (5x5)");
}

// ════════════════════════════════════════════════
// Hint engine tests
// ════════════════════════════════════════════════

function blankState(n: number): {
  answers: (Answer | null)[];
  eliminated: number[];
} {
  return { answers: new Array(n).fill(null), eliminated: new Array(n).fill(0) };
}

function setCorrect(answers: (Answer | null)[], eliminated: number[], qi: number, letter: Answer) {
  const oi = L2I[letter];
  eliminated[qi] = 0b11111 ^ (1 << oi);
  answers[qi] = letter;
}

function setEliminated(eliminated: number[], qi: number, letter: Answer) {
  eliminated[qi] |= 1 << L2I[letter];
}

function applyAction(
  action: import("../src/engine/deduce.ts").DeduceAction,
  answers: (Answer | null)[],
  eliminated: number[],
) {
  if (action.type === "force") {
    const oi = L2I[action.answer];
    eliminated[action.qi] = 0b11111 ^ (1 << oi);
    answers[action.qi] = action.answer;
  } else if (action.type === "eliminate") {
    eliminated[action.qi] |= 1 << action.oi;
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
    const v = checkAnswer(fp, { answers, eliminated }, 1);
    assert(v === "invalid", `contradiction: Q2 should be invalid (got ${v})`);
  }

  // ── Forced: answer_of_question when target is known ──
  {
    const { answers, eliminated } = blankState(2);
    const fp = flattenPuzzle(contradictionPuzzle);
    setCorrect(answers, eliminated, 0, "C"); // Q1 = C, so Q2 must be C
    const dr = deduceAssumingUnique(fp, { answers, eliminated });
    assert(dr.length > 0, "forced hint: deduce returns a result");
    assert(
      dr[0].action.type === "force",
      `forced hint: action type is force (got ${dr[0].action.type})`,
    );
    assert(
      dr[0].action.type === "force" && dr[0].action.answer === "C",
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
    const dr = deduceAssumingUnique(fp, { answers, eliminated });
    assert(dr.length > 0, "forced-by-elim: deduce returns a result");
    assert(
      dr[0].action.type === "force" && dr[0].action.answer === "E",
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
    const dr = deduceAssumingUnique(fp, { answers, eliminated });
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
    const dr = deduceAssumingUnique(fp, { answers, eliminated });
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
    const dr = deduceAssumingUnique(fp, { answers, eliminated });
    if (dr.length > 0) {
      assert(true, "lookahead puzzle: deduce found something directly");
    } else {
      const lr = lookahead(fp, { answers, eliminated });
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
    const dr = deduceAssumingUnique(fp, { answers, eliminated });
    assert(dr.length === 0, "solved puzzle: deduce returns empty");
    const lr = lookahead(fp, { answers, eliminated });
    assert(lr == null, "solved puzzle: lookahead returns null");
  }

  // ── Solvability: verify generated puzzles are solvable from blank ──
  for (const puzzle of allPuzzles.slice(0, 3)) {
    const n = puzzle.questions.length;
    const fp = flattenPuzzle(puzzle);
    const oc = puzzle.optionCount ?? 5;
    const phantomMask = 0b11111 & ~((1 << oc) - 1);
    const answers: (Answer | null)[] = new Array(n).fill(null);
    const eliminated: number[] = new Array(n).fill(phantomMask);
    let steps = 0;
    let stuck = false;

    while (!answers.every((a) => a != null) && steps < n * 15) {
      const dr = deduceAssumingUnique(fp, { answers, eliminated });
      if (dr.length > 0) {
        applyAction(dr[0].action, answers, eliminated);
        steps++;
        continue;
      }
      const lr = lookahead(fp, { answers, eliminated });
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
    stale: false,
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
// Shared check-answer cross-validation (TS ↔ Rust)
// ════════════════════════════════════════════════

function testSharedCheckAnswer() {
  const suiteJson = JSON.parse(
    readFileSync(resolve(import.meta.dirname, "../tests/check-answer.json"), "utf8"),
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

      let questionType: import("../src/engine/types.ts").QuestionType;
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
          } as import("../src/engine/types.ts").QuestionType;
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
          } as import("../src/engine/types.ts").QuestionType;
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
          } as import("../src/engine/types.ts").QuestionType;
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
            let qt: import("../src/engine/types.ts").QuestionType;
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

    const answers: (import("../src/engine/types.ts").Answer | null)[] = new Array(n).fill(null);
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

    const got = checkAnswer(fp, { answers, eliminated }, qi);
    assert(got === expect, `shared check-answer: ${name}: expected ${expect}, got ${got}`);
  }
}

// ════════════════════════════════════════════════
// Shared lookahead cross-validation (TS ↔ Rust)
// ════════════════════════════════════════════════

function testSharedLookahead() {
  const suiteJson = JSON.parse(
    readFileSync(resolve(import.meta.dirname, "../tests/lookahead.json"), "utf8"),
  );
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

    const wrapped = { "0101": { "1": compact } } as any;
    const parsed = parseCompactYear(wrapped);
    const puzzle = parsed["0101"]["1"];
    const fp = flattenPuzzle(puzzle);
    const n = puzzle.questions.length;

    const answers: (Answer | null)[] = new Array(n).fill(null);
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

    const result = lookahead(fp, { answers, eliminated });
    const got = result ? `${result.eliminateQi + 1}${"abcde"[result.eliminateOi]}` : null;
    const gotStr = got === null ? "null" : got;
    const expectStr = expect === null ? "null" : expect;
    assert(gotStr === expectStr, `shared lookahead: ${name}: expected ${expectStr}, got ${gotStr}`);

    // Explain check: every lookahead result should produce a complete explanation
    if (result && gotStr === expectStr) {
      try {
        explainLookahead(puzzle, fp, answers, eliminated, result);
      } catch (e) {
        assert(false, `shared lookahead explain threw: ${name}: ${String(e)}`);
      }
    }
  }
}

// ════════════════════════════════════════════════
// Shared solve cross-validation (TS ↔ Rust)
// ════════════════════════════════════════════════

function testSharedSolve() {
  const suiteJson = JSON.parse(
    readFileSync(resolve(import.meta.dirname, "../tests/solve.json"), "utf8"),
  );
  const tests = suiteJson.tests as (
    | { section: string }
    | {
        name: string;
        puzzle: Record<string, unknown>;
        expect: SolveOutcome;
        solution?: string;
      }
  )[];

  for (const t of tests) {
    if ("section" in t) continue;
    const { name, puzzle: compact, expect, solution } = t;

    const wrapped = { "0101": { "1": compact } } as any;
    const parsed = parseCompactYear(wrapped);
    const puzzle = parsed["0101"]["1"];
    const fp = flattenPuzzle(puzzle);

    const result = solvePuzzle(fp);
    const got: SolveOutcome = result.answers.slice(0, fp.n).every((a) => a != null)
      ? "solved"
      : "stuck";
    assert(got === expect, `shared solve: ${name}: expected ${expect}, got ${got}`);

    if (solution) {
      const gotSol = result.answers.slice(0, fp.n).join("");
      assert(gotSol === solution, `shared solve: ${name}: expected ${solution}, got ${gotSol}`);
    }
  }
}

// ════════════════════════════════════════════════
// Shared deduce tests (correctness + explanations + DRY + coverage)
// ════════════════════════════════════════════════

function testSharedDeduce() {
  const suite = JSON.parse(
    readFileSync(resolve(import.meta.dirname, "../tests/deduce.json"), "utf8"),
  );
  const coveredRules = new Set<string>();

  function formatAction(dr: DeduceResult | undefined): string {
    if (!dr) return "null";
    const a = dr.action;
    if (a.type === "force") return `${a.qi + 1}${a.answer}`;
    if (a.type === "eliminate") return `${a.qi + 1}${"abcde"[a.oi]}`;
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
    optionCount: number,
    state: string[],
  ): { answers: (Answer | null)[]; eliminated: number[] } {
    const answers: (Answer | null)[] = new Array(n).fill(null);
    const phantomMask = 0b11111 & ~((1 << optionCount) - 1);
    const eliminated: number[] = new Array(n).fill(phantomMask);
    for (let qi = 0; qi < n; qi++) {
      const s = state[qi] || "";
      for (const ch of s) {
        if (ch >= "A" && ch <= "E") {
          answers[qi] = ch as Answer;
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
    const { answers, eliminated } = applyState(n, fp.optionCount, state);

    const parsedRule: DeduceRule | null =
      ruleStr && ALL_DEDUCE_RULES.includes(ruleStr) ? ruleStr : null;
    if (parsedRule) coveredRules.add(parsedRule);

    const results = parsedRule
      ? deduceWithRule(fp, { answers, eliminated }, parsedRule)
      : deduceAssumingUnique(fp, { answers, eliminated });
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
      const without = deduceWithRule(fp, { answers, eliminated }, null, parsedRule);
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
// Shared fill-options tests
// ════════════════════════════════════════════════

function testSharedFillOptions() {
  const suite = JSON.parse(
    readFileSync(resolve(import.meta.dirname, "../tests/fill-options.json"), "utf8"),
  );
  const SEEDS = 16;

  for (const t of suite.tests) {
    if (t.section) continue;
    const { name, n, oc, types, solution: solStr, expectedCorrect } = t;

    const parsedTypes = (types as { t: string; a?: number; q?: number }[]).map((ct) =>
      expandQuestion(ct),
    );
    const solution: Answer[] = [];
    for (const ch of solStr as string) solution.push(LETTERS[ch.charCodeAt(0) - 65]);

    const expected = expectedCorrect as (number | null)[];
    assert(
      Array.isArray(expected) && expected.length === n,
      `fill-options: ${name}: expectedCorrect missing or wrong length`,
    );

    for (let seed = 0; seed < SEEDS; seed++) {
      const rng = new RNG(Math.imul(seed, 2654435761));
      const cr: ConstructResult = {
        types: parsedTypes,
        solution,
        n,
        oc,
        level: 1,
        name: "test",
      };
      const puzzle = fillOptions(cr, rng, false);
      if (puzzle == null) {
        assert(false, `fill-options: ${name} (seed=${seed}): fillOptions returned null`);
        continue;
      }
      const fp = flattenPuzzle(puzzle);
      assert(
        checkAnswers(fp, solution),
        `fill-options: ${name} (seed=${seed}): checkAnswers rejected`,
      );

      // expectedCorrect cross-check: the value at the correct option must
      // match the hand-computed expectation.
      for (let qi = 0; qi < n; qi++) {
        const exp = expected[qi];
        if (exp == null) continue;
        const correctOi = L2I[solution[qi]];
        const stored = puzzle.questions[qi].options[correctOi].value;
        assert(
          stored === exp,
          `fill-options: ${name} (seed=${seed}) Q${qi + 1}: stored ${String(stored)} != expected ${String(exp)}`,
        );
      }

      // Distinctness: distractor option values must differ from the correct
      // value and from each other. Identity-option / TrueStmt types don't
      // store numeric values, so skip them.
      for (let qi = 0; qi < n; qi++) {
        const qt = puzzle.questions[qi].questionType;
        if (qt.type === "AnswerIsSelf" || qt.type === "NoOtherHasAnswer" || qt.type === "TrueStmt")
          continue;
        const seen = new Set<number>();
        const opts = puzzle.questions[qi].options;
        for (let oi = 0; oi < oc; oi++) {
          const v = opts[oi].value;
          if (v == null) continue;
          assert(
            !seen.has(v),
            `fill-options: ${name} (seed=${seed}) Q${qi + 1}: duplicate option value ${v}`,
          );
          seen.add(v);
        }
      }
    }
  }
}

// ════════════════════════════════════════════════
// Shared valid-values tests (validValues ⟷ checkForm cross-check)
// ════════════════════════════════════════════════

// Exempt: types whose option.value field isn't a user-chosen pool member.
// NoOtherHasAnswer / AnswerIsSelf use identity options (value is always the
// letter index); TrueStmt uses claims instead of values.
const VALUE_TYPED_EXEMPT: ReadonlySet<QuestionTypeName> = new Set([
  "NoOtherHasAnswer",
  "AnswerIsSelf",
  "TrueStmt",
]);
const VALUE_TYPED_QUESTION_TYPES: readonly QuestionTypeName[] = ALL_QUESTION_TYPE_NAMES.filter(
  (t) => !VALUE_TYPED_EXEMPT.has(t),
);

function buildSinglePuzzle(
  qt: ReturnType<typeof expandQuestion>,
  qi: number,
  n: number,
  oc: number,
  v: number | null,
): Puzzle {
  const questions: Puzzle["questions"] = [];
  for (let i = 0; i < n; i++) {
    const opts: { value: number | null }[] = [];
    if (i === qi) {
      opts.push({ value: v });
      for (let j = 1; j < oc; j++) opts.push({ value: null });
      questions.push({ options: opts, questionType: qt });
    } else {
      for (let j = 0; j < oc; j++) opts.push({ value: null });
      questions.push({ options: opts, questionType: { type: "AnswerIsSelf" } });
    }
  }
  return { id: "test", title: "T", difficulty: "1", optionCount: oc, questions };
}

function testSharedValidValues() {
  const suite = JSON.parse(
    readFileSync(resolve(import.meta.dirname, "../tests/valid-values.json"), "utf8"),
  );
  const covered = new Set<string>();

  for (const t of suite.tests) {
    if (t.section) continue;
    const { name, qi, n, oc } = t as { name: string; qi: number; n: number; oc: number };
    const type = expandQuestion(t.type);
    covered.add(type.type);

    // 1) validValues function output matches fixture spec
    const got = validValues(type, qi, n, oc);
    const gotSet = new Set(got.map((v) => (v === null ? "null" : String(v))));
    const expSet = new Set(
      (t.valid as (number | null)[]).map((v) => (v === null ? "null" : String(v))),
    );
    assertEq([...gotSet].sort(), [...expSet].sort(), `valid-values: ${name}: pool`);

    // 2) & 3) Cross-check checkForm: any message at qi mentioning "option 0"
    // (the slot we vary) must fire iff the value isn't in the pool. The "option 0"
    // scope filters out incidental errors on the other (null-filled) options.
    // Skip negatives: JSON -1 collides with Rust's NONE_VAL sentinel, so the
    // two parsers can't represent it portably.
    const maxV = Math.max(n, oc) + 1;
    const candidates: (number | null)[] = [];
    for (let i = 0; i <= maxV; i++) candidates.push(i);
    candidates.push(null);
    for (const v of candidates) {
      const inPool = gotSet.has(v === null ? "null" : String(v));
      const puzzle = buildSinglePuzzle(type, qi, n, oc, v);
      const errors = checkForm(puzzle);
      const flagged = errors.some((e) => e.qi === qi && /\boption 0\b/i.test(e.message));
      assert(
        flagged === !inPool,
        `valid-values: ${name} v=${v === null ? "null" : v}: pool=${inPool ? "in" : "out"}, checkForm=${flagged ? "flagged" : "ok"} (disagree)`,
      );
    }
  }

  // 4) Coverage: every value-typed QuestionType has at least one fixture entry
  for (const ty of VALUE_TYPED_QUESTION_TYPES) {
    assert(covered.has(ty), `valid-values: missing fixture coverage for ${ty}`);
  }
}

// ════════════════════════════════════════════════
// Check-form tests
// ════════════════════════════════════════════════

function testCheckForm() {
  const suite = JSON.parse(
    readFileSync(resolve(import.meta.dirname, "../tests/check-form.json"), "utf8"),
  );
  for (const t of suite.tests) {
    if (t.section) continue;
    const puzzle = parseCompactYear({ "0101": { "1": t.puzzle } })["0101"]["1"];
    const errors = checkForm(puzzle);
    const hasErrors = errors.length > 0;
    assert(
      hasErrors === t.expectError,
      `check-form: ${t.name}: expected ${t.expectError ? "errors" : "no errors"}, got ${hasErrors ? errors.map((e: { qi: number; message: string }) => e.message).join("; ") : "none"}`,
    );
  }
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
timed("Shared check-answer tests", testSharedCheckAnswer);
timed("Shared lookahead tests", testSharedLookahead);
timed("Shared solve tests", testSharedSolve);
timed("Shared fill-options tests", testSharedFillOptions);
timed("Shared valid-values tests", testSharedValidValues);
timed("Solver edge cases", testSolverEdgeCases);
timed("Share encode/decode tests", testShare);

timed("Check-form tests", testCheckForm);
timed("Generated puzzles: wellformed", testGeneratedWellformed);
timed("Hint engine tests", testHints);
if (slow) {
  timed("Generated puzzles: unique solution", testGeneratedUniqueSolution);
  timed("Generated puzzles: hint solvable", testGeneratedHintSolvable);
} else {
  console.log("Skipping generated puzzle tests (use --all to include)");
}

console.log(`\n${passed} passed, ${failed} failed`);
if (failed > 0) process.exit(1);
