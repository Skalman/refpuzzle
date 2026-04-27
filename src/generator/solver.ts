import type { AnswerLetter, Puzzle, FlatPuzzle, FlatRule, RuleTypeId } from "../engine/types.ts";
import {
  LETTERS,
  letterIdx,
  flattenPuzzle,
  RT_COUNT_ANSWER,
  RT_COUNT_ANSWER_BEFORE,
  RT_COUNT_ANSWER_AFTER,
  RT_COUNT_VOWEL,
  RT_COUNT_CONSONANT,
  RT_MOST_COMMON_COUNT,
  RT_CLOSEST_AFTER,
  RT_CLOSEST_BEFORE,
  RT_PREV_SAME,
  RT_NEXT_SAME,
  RT_ONLY_SAME,
  RT_CONSEC_IDENT,
  RT_ONLY_ODD,
  RT_ANSWER_OF,
  RT_LEAST_COMMON,
  RT_MOST_COMMON,
  RT_UNIQUE,
  RT_EQUAL_COUNT,
  RT_SELF,
  RT_LETTER_DIST,
  RT_TRUE_STMT,
} from "../engine/types.ts";
import { evaluate } from "../engine/evaluators.ts";

export function solve(
  puzzle: Puzzle,
  fixedAnswers?: (AnswerLetter | null)[],
  maxSolutions = 2,
): AnswerLetter[][] {
  return solveFp(flattenPuzzle(puzzle), fixedAnswers, maxSolutions);
}

const SOLVER_GLOBAL_IDS: Set<RuleTypeId> = new Set([
  RT_COUNT_ANSWER,
  RT_COUNT_VOWEL,
  RT_COUNT_CONSONANT,
  RT_LEAST_COMMON,
  RT_MOST_COMMON,
  RT_MOST_COMMON_COUNT,
  RT_UNIQUE,
  RT_EQUAL_COUNT,
  RT_TRUE_STMT,
  RT_ONLY_SAME,
  RT_CONSEC_IDENT,
  RT_ONLY_ODD,
]);

function computeSearchOrder(fp: FlatPuzzle): number[] {
  const n = fp.n;

  // Count how many answer_of_question rules point to each question
  const refCount = new Array<number>(n).fill(0);
  for (const r of fp.rules) {
    if (r.t === RT_ANSWER_OF && r.questionIndex >= 0) {
      refCount[r.questionIndex]++;
    }
  }

  const indices = Array.from({ length: n }, (_, i) => i);

  indices.sort((a, b) => {
    // Most-referenced questions first (they unlock answer_of_question chains)
    if (refCount[b] !== refCount[a]) return refCount[b] - refCount[a];
    // Non-global rules before global (global needs all answers)
    const aGlobal = SOLVER_GLOBAL_IDS.has(fp.rules[a].t) ? 1 : 0;
    const bGlobal = SOLVER_GLOBAL_IDS.has(fp.rules[b].t) ? 1 : 0;
    return aGlobal - bGlobal;
  });

  return indices;
}

function solveFp(
  fp: FlatPuzzle,
  fixedAnswers?: (AnswerLetter | null)[],
  maxSolutions = 2,
): AnswerLetter[][] {
  const n = fp.n;
  const fixed = fixedAnswers ?? new Array<AnswerLetter | null>(n).fill(null);
  const solutions: AnswerLetter[][] = [];
  const current = new Array<AnswerLetter | null>(n).fill(null);
  const order = computeSearchOrder(fp);
  const allBits = (1 << n) - 1;
  let assignedBits = 0;

  // Pre-compute bitmasks for range checks in canFullyEvaluateLocal
  const rangeMasks: number[] = new Array(n);
  for (let i = 0; i < n; i++) {
    const r = fp.rules[i];
    if (r.t === RT_NEXT_SAME) {
      let m = 0;
      for (let j = i + 1; j < n; j++) m |= 1 << j;
      rangeMasks[i] = m;
    } else if (r.t === RT_CLOSEST_AFTER) {
      let m = 0;
      for (let j = r.afterIndex + 1; j < n; j++) m |= 1 << j;
      rangeMasks[i] = m;
    } else if (r.t === RT_CLOSEST_BEFORE || r.t === RT_COUNT_ANSWER_BEFORE) {
      let m = 0;
      for (let j = 0; j < r.beforeIndex; j++) m |= 1 << j;
      rangeMasks[i] = m;
    } else if (r.t === RT_COUNT_ANSWER_AFTER) {
      let m = 0;
      for (let j = r.afterIndex + 1; j < n; j++) m |= 1 << j;
      rangeMasks[i] = m;
    } else {
      rangeMasks[i] = 0;
    }
  }

  function search(depth: number) {
    if (solutions.length >= maxSolutions) return;

    if (depth === n) {
      let valid = true;
      for (let i = 0; i < n; i++) {
        if (!evaluate(fp.rules[i], i, current[i]!, current, fp)) {
          valid = false;
          break;
        }
      }
      if (valid) {
        const copy: AnswerLetter[] = [];
        for (let i = 0; i < n; i++) copy.push(current[i]!);
        solutions.push(copy);
      }
      return;
    }

    const qi = order[depth];
    const bit = 1 << qi;

    if (fixed[qi] != null) {
      current[qi] = fixed[qi];
      assignedBits |= bit;
      if (!hasContradiction(fp, current, n, qi, assignedBits, allBits, rangeMasks)) {
        search(depth + 1);
      }
      current[qi] = null;
      assignedBits &= ~bit;
      return;
    }

    for (const letter of LETTERS) {
      current[qi] = letter;
      assignedBits |= bit;
      if (!hasContradiction(fp, current, n, qi, assignedBits, allBits, rangeMasks)) {
        search(depth + 1);
        if (solutions.length >= maxSolutions) {
          current[qi] = null;
          assignedBits &= ~bit;
          return;
        }
      }
    }
    current[qi] = null;
    assignedBits &= ~bit;
  }

  search(0);
  return solutions;
}

function hasContradiction(
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  n: number,
  justAssigned: number,
  assigned: number,
  allBits: number,
  rangeMasks: number[],
): boolean {
  const allAnswered = assigned === allBits;

  // When all answered, full check (correctness guarantee)
  if (allAnswered) {
    for (let i = 0; i < n; i++) {
      if (!evaluate(fp.rules[i], i, answers[i]!, answers, fp)) return true;
    }
    return false;
  }

  // Incremental: check questions affected by the just-assigned question
  const affected = fp.affectedBy[justAssigned];
  for (let k = 0; k < affected.length; k++) {
    const i = affected[k];
    if (answers[i] == null) continue;
    if (checkRule(fp, answers, n, i, allAnswered, assigned, rangeMasks)) return true;
  }

  // Check global rules for forward-checking bounds
  const globals = fp.globalIndices;
  for (let k = 0; k < globals.length; k++) {
    const i = globals[k];
    if (answers[i] == null) continue;
    if (i === justAssigned) continue;
    if (checkRule(fp, answers, n, i, allAnswered, assigned, rangeMasks)) return true;
  }

  return false;
}

function checkRule(
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  n: number,
  i: number,
  allAnswered: boolean,
  assigned: number,
  rangeMasks: number[],
): boolean {
  const r = fp.rules[i];

  if (allAnswered || canFullyEvaluateLocal(r, answers, assigned, rangeMasks, i)) {
    if (!evaluate(r, i, answers[i]!, answers, fp)) return true;
  }

  // Forward checking for counting rules
  if (r.t === RT_COUNT_ANSWER || r.t === RT_COUNT_ANSWER_BEFORE || r.t === RT_COUNT_ANSWER_AFTER) {
    const optVal = fp.optionValues[i][letterIdx(answers[i]!)];
    if (optVal == null) return false;

    let rangeStart: number;
    let rangeEnd: number;

    if (r.t === RT_COUNT_ANSWER) {
      rangeStart = 0;
      rangeEnd = n;
    } else if (r.t === RT_COUNT_ANSWER_BEFORE) {
      rangeStart = 0;
      rangeEnd = r.beforeIndex;
    } else {
      rangeStart = r.afterIndex + 1;
      rangeEnd = n;
    }

    let count = 0;
    let remaining = 0;
    for (let j = rangeStart; j < rangeEnd; j++) {
      if (answers[j] === r.answer) count++;
      else if (answers[j] == null) remaining++;
    }
    if (count > optVal || count + remaining < optVal) return true;
  }

  if (r.t === RT_COUNT_VOWEL || r.t === RT_COUNT_CONSONANT) {
    const optVal = fp.optionValues[i][letterIdx(answers[i]!)];
    if (optVal == null) return false;
    const isVowel = r.t === RT_COUNT_VOWEL;
    let count = 0;
    let remaining = 0;
    for (let j = 0; j < n; j++) {
      if (answers[j] == null) {
        remaining++;
      } else if (
        isVowel
          ? answers[j] === "A" || answers[j] === "E"
          : answers[j] !== "A" && answers[j] !== "E"
      ) {
        count++;
      }
    }
    if (count > optVal || count + remaining < optVal) return true;
  }

  return false;
}

// Lightweight canFullyEvaluate for non-global rules only
function canFullyEvaluateLocal(
  r: FlatRule,
  _answers: (AnswerLetter | null)[],
  assigned: number,
  rangeMasks: number[],
  questionIdx: number,
): boolean {
  switch (r.t) {
    case RT_SELF:
      return true;
    case RT_PREV_SAME: {
      let mask = 0;
      for (let j = 0; j < questionIdx; j++) mask |= 1 << j;
      return (assigned & mask) === mask;
    }
    case RT_ANSWER_OF:
      return (assigned & (1 << r.questionIndex)) !== 0;
    case RT_LETTER_DIST:
      return (assigned & (1 << r.questionIndex)) !== 0;
    case RT_NEXT_SAME:
    case RT_CLOSEST_AFTER:
    case RT_CLOSEST_BEFORE:
    case RT_COUNT_ANSWER_BEFORE:
    case RT_COUNT_ANSWER_AFTER: {
      const mask = rangeMasks[questionIdx];
      return (assigned & mask) === mask;
    }
    default:
      return false;
  }
}
