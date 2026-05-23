import type { Answer, Puzzle, FlatPuzzle, FlatQuestion, QuestionTypeId } from "../engine/types.ts";
import {
  LETTERS,
  MAX_N,
  letterIdx,
  flattenPuzzle,
  QT_COUNT_ANSWER,
  QT_COUNT_ANSWER_BEFORE,
  QT_COUNT_ANSWER_AFTER,
  QT_COUNT_VOWEL,
  QT_COUNT_CONSONANT,
  QT_MOST_COMMON_COUNT,
  QT_CLOSEST_AFTER,
  QT_CLOSEST_BEFORE,
  QT_PREV_SAME,
  QT_NEXT_SAME,
  QT_ONLY_SAME,
  QT_CONSEC_IDENT,
  QT_ONLY_ODD,
  QT_ANSWER_OF,
  QT_LEAST_COMMON,
  QT_MOST_COMMON,
  QT_NO_OTHER_HAS_ANSWER,
  QT_EQUAL_COUNT,
  QT_ANSWER_IS_SELF,
  QT_LETTER_DIST,
  QT_TRUE_STMT,
} from "../engine/types.ts";
import { checkAnswer, checkAnswers } from "../engine/check-answer.ts";
import { isValid } from "../engine/state.ts";

const EMPTY_ELIMINATED: number[] = Array(MAX_N).fill(0);

export function solve(
  puzzle: Puzzle,
  fixedAnswers?: (Answer | null)[],
  maxSolutions = 2,
): Answer[][] {
  return solveFp(flattenPuzzle(puzzle), fixedAnswers, maxSolutions);
}

const SOLVER_GLOBAL_IDS: Set<QuestionTypeId> = new Set([
  QT_COUNT_ANSWER,
  QT_COUNT_VOWEL,
  QT_COUNT_CONSONANT,
  QT_LEAST_COMMON,
  QT_MOST_COMMON,
  QT_MOST_COMMON_COUNT,
  QT_NO_OTHER_HAS_ANSWER,
  QT_EQUAL_COUNT,
  QT_TRUE_STMT,
  QT_ONLY_SAME,
  QT_CONSEC_IDENT,
  QT_ONLY_ODD,
]);

function computeSearchOrder(fp: FlatPuzzle): number[] {
  const n = fp.n;

  // Count how many answer_of_question rules point to each question
  const refCount = new Array<number>(n).fill(0);
  for (const q of fp.questions) {
    if (q.t === QT_ANSWER_OF && q.questionIndex >= 0) {
      refCount[q.questionIndex]++;
    }
  }

  const indices = Array.from({ length: n }, (_, i) => i);

  indices.sort((a, b) => {
    // Most-referenced questions first (they unlock answer_of_question chains)
    if (refCount[b] !== refCount[a]) return refCount[b] - refCount[a];
    // Non-global rules before global (global needs all answers)
    const aGlobal = SOLVER_GLOBAL_IDS.has(fp.questions[a].t) ? 1 : 0;
    const bGlobal = SOLVER_GLOBAL_IDS.has(fp.questions[b].t) ? 1 : 0;
    return aGlobal - bGlobal;
  });

  return indices;
}

function solveFp(fp: FlatPuzzle, fixedAnswers?: (Answer | null)[], maxSolutions = 2): Answer[][] {
  const n = fp.n;
  const fixed = fixedAnswers ?? new Array<Answer | null>(n).fill(null);
  const solutions: Answer[][] = [];
  const current = new Array<Answer | null>(n).fill(null);
  const order = computeSearchOrder(fp);
  const allBits = (1 << n) - 1;
  let assignedBits = 0;

  // Pre-compute bitmasks for range checks in canFullyEvaluateLocal
  const rangeMasks: number[] = new Array(n);
  for (let i = 0; i < n; i++) {
    const q = fp.questions[i];
    if (q.t === QT_NEXT_SAME) {
      let m = 0;
      for (let j = i + 1; j < n; j++) m |= 1 << j;
      rangeMasks[i] = m;
    } else if (q.t === QT_CLOSEST_AFTER) {
      let m = 0;
      for (let j = q.afterIndex + 1; j < n; j++) m |= 1 << j;
      rangeMasks[i] = m;
    } else if (q.t === QT_CLOSEST_BEFORE || q.t === QT_COUNT_ANSWER_BEFORE) {
      let m = 0;
      for (let j = 0; j < q.beforeIndex; j++) m |= 1 << j;
      rangeMasks[i] = m;
    } else if (q.t === QT_COUNT_ANSWER_AFTER) {
      let m = 0;
      for (let j = q.afterIndex + 1; j < n; j++) m |= 1 << j;
      rangeMasks[i] = m;
    } else {
      rangeMasks[i] = 0;
    }
  }

  function search(depth: number) {
    if (solutions.length >= maxSolutions) return;

    if (depth === n) {
      if (checkAnswers(fp, current)) {
        const copy: Answer[] = [];
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

    for (let li = 0; li < fp.optionCount; li++) {
      const letter = LETTERS[li];
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
  answers: (Answer | null)[],
  n: number,
  justAssigned: number,
  assigned: number,
  allBits: number,
  rangeMasks: number[],
): boolean {
  const allAnswered = assigned === allBits;

  // When all answered, full check (correctness guarantee)
  if (allAnswered) {
    return !checkAnswers(fp, answers);
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
  answers: (Answer | null)[],
  n: number,
  i: number,
  allAnswered: boolean,
  assigned: number,
  rangeMasks: number[],
): boolean {
  const q = fp.questions[i];

  if (allAnswered || canFullyEvaluateLocal(q, answers, assigned, rangeMasks, i)) {
    if (!isValid(checkAnswer(fp, { answers, eliminated: EMPTY_ELIMINATED }, i))) return true;
  }

  // Forward checking for counting rules
  if (q.t === QT_COUNT_ANSWER || q.t === QT_COUNT_ANSWER_BEFORE || q.t === QT_COUNT_ANSWER_AFTER) {
    const optVal = fp.optionValues[i][letterIdx(answers[i]!)];
    if (optVal == null) return false;

    let rangeStart: number;
    let rangeEnd: number;

    if (q.t === QT_COUNT_ANSWER) {
      rangeStart = 0;
      rangeEnd = n;
    } else if (q.t === QT_COUNT_ANSWER_BEFORE) {
      rangeStart = 0;
      rangeEnd = q.beforeIndex;
    } else {
      rangeStart = q.afterIndex + 1;
      rangeEnd = n;
    }

    let count = 0;
    let remaining = 0;
    for (let j = rangeStart; j < rangeEnd; j++) {
      if (answers[j] === q.answer) count++;
      else if (answers[j] == null) remaining++;
    }
    if (count > optVal || count + remaining < optVal) return true;
  }

  if (q.t === QT_COUNT_VOWEL || q.t === QT_COUNT_CONSONANT) {
    const optVal = fp.optionValues[i][letterIdx(answers[i]!)];
    if (optVal == null) return false;
    const isVowel = q.t === QT_COUNT_VOWEL;
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
  q: FlatQuestion,
  _answers: (Answer | null)[],
  assigned: number,
  rangeMasks: number[],
  questionIdx: number,
): boolean {
  switch (q.t) {
    case QT_ANSWER_IS_SELF:
      return true;
    case QT_PREV_SAME: {
      let mask = 0;
      for (let j = 0; j < questionIdx; j++) mask |= 1 << j;
      return (assigned & mask) === mask;
    }
    case QT_ANSWER_OF:
      return (assigned & (1 << q.questionIndex)) !== 0;
    case QT_LETTER_DIST:
      return (assigned & (1 << q.questionIndex)) !== 0;
    case QT_NEXT_SAME:
    case QT_CLOSEST_AFTER:
    case QT_CLOSEST_BEFORE:
    case QT_COUNT_ANSWER_BEFORE:
    case QT_COUNT_ANSWER_AFTER: {
      const mask = rangeMasks[questionIdx];
      return (assigned & mask) === mask;
    }
    default:
      return false;
  }
}
