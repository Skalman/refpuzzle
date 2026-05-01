export type AnswerLetter = "A" | "B" | "C" | "D" | "E";
export const VOWELS: ReadonlySet<AnswerLetter> = new Set<AnswerLetter>(["A", "E"]);
export type OptionMark = "unmarked" | "incorrect" | "correct";
export type Marks = [OptionMark, OptionMark, OptionMark, OptionMark, OptionMark];

export const LETTERS: readonly AnswerLetter[] = ["A", "B", "C", "D", "E"];
export const L2I: Record<string, number> = { A: 0, B: 1, C: 2, D: 3, E: 4 };

// Fast char-code based lookup: 'A'.charCodeAt(0)=65, so L2I_FAST[65]=0, etc.
const _l2i = new Int8Array(70);
_l2i[65] = 0;
_l2i[66] = 1;
_l2i[67] = 2;
_l2i[68] = 3;
_l2i[69] = 4;
export function letterIdx(s: string): number {
  return _l2i[s.charCodeAt(0)];
}

export interface Puzzle {
  id: string;
  title: string;
  difficulty: 1 | 2 | 3 | 4 | 5;
  questions: QuestionDef[];
}

export interface QuestionDef {
  options: OptionDef[];
  questionType: QuestionTypeDef;
}

export type OptionDef = SimpleOption | StatementOption;

export interface SimpleOption {
  value: number | null;
}

export interface StatementOption {
  value: number | null;
  claim: Claim;
}

export type Claim =
  | { type: "count_answer"; answer: AnswerLetter; value: number }
  | { type: "count_consonant_answers"; value: number }
  | { type: "count_vowel_answers"; value: number }
  | {
      type: "count_answer_after";
      answer: AnswerLetter;
      afterIndex: number;
      value: number;
    }
  | {
      type: "count_answer_before";
      answer: AnswerLetter;
      beforeIndex: number;
      value: number;
    }
  | { type: "answer_of_question"; questionIndex: number; value: number }
  | { type: "first_with_answer"; answer: AnswerLetter; value: number }
  | { type: "last_with_answer"; answer: AnswerLetter; value: number }
  | { type: "most_common_answer"; value: number };

export type QuestionTypeDef =
  // ── Counting ──
  | { type: "count_answer"; answer: AnswerLetter }
  | { type: "count_answer_before"; answer: AnswerLetter; beforeIndex: number }
  | { type: "count_answer_after"; answer: AnswerLetter; afterIndex: number }
  | { type: "count_vowel_answers" }
  | { type: "count_consonant_answers" }
  | { type: "most_common_count" }

  // ── Positional ──
  | { type: "closest_after"; afterIndex: number; answer: AnswerLetter }
  | { type: "closest_before"; beforeIndex: number; answer: AnswerLetter }
  | { type: "first_with_answer"; answer: AnswerLetter }
  | { type: "last_with_answer"; answer: AnswerLetter }
  | { type: "previous_same_answer" }
  | { type: "next_same_answer" }
  | { type: "only_same_answer" }
  | { type: "same_answer_as" }
  | { type: "only_odd_with_answer"; answer: AnswerLetter }
  | { type: "consecutive_identical" }

  // ── Constrained (options always A-E, answer determined by solution) ──
  | { type: "answer_of_question"; questionIndex: number }
  | { type: "least_common_answer" }
  | { type: "most_common_answer" }
  | { type: "unique_answer" }
  | { type: "equal_count_as"; answer: AnswerLetter }
  | { type: "answer_is_self" }

  // ── Relationship ──
  | { type: "letter_distance"; questionIndex: number }

  // ── Compound ──
  | { type: "only_true_statement" };

// Numeric question type IDs — top-level constants for V8 inlining
export const RT_COUNT_ANSWER = 0;
export const RT_COUNT_ANSWER_BEFORE = 1;
export const RT_COUNT_ANSWER_AFTER = 2;
export const RT_COUNT_VOWEL = 3;
export const RT_COUNT_CONSONANT = 4;
export const RT_MOST_COMMON_COUNT = 5;
export const RT_CLOSEST_AFTER = 6;
export const RT_CLOSEST_BEFORE = 7;
export const RT_FIRST_WITH = 8;
export const RT_LAST_WITH = 9;
export const RT_PREV_SAME = 10;
export const RT_NEXT_SAME = 11;
export const RT_ONLY_SAME = 12;
export const RT_SAME_AS = 13;
export const RT_ONLY_ODD = 14;
export const RT_CONSEC_IDENT = 15;
export const RT_ANSWER_OF = 16;
export const RT_LEAST_COMMON = 17;
export const RT_MOST_COMMON = 18;
export const RT_UNIQUE = 19;
export const RT_EQUAL_COUNT = 20;
export const RT_SELF = 21;
export const RT_LETTER_DIST = 22;
export const RT_TRUE_STMT = 23;

export type QuestionTypeId = number;

// Mapping from string type names to numeric IDs (used in flattenRule)
const RT_MAP: Record<string, QuestionTypeId> = {
  count_answer: RT_COUNT_ANSWER,
  count_answer_before: RT_COUNT_ANSWER_BEFORE,
  count_answer_after: RT_COUNT_ANSWER_AFTER,
  count_vowel_answers: RT_COUNT_VOWEL,
  count_consonant_answers: RT_COUNT_CONSONANT,
  most_common_count: RT_MOST_COMMON_COUNT,
  closest_after: RT_CLOSEST_AFTER,
  closest_before: RT_CLOSEST_BEFORE,
  first_with_answer: RT_FIRST_WITH,
  last_with_answer: RT_LAST_WITH,
  previous_same_answer: RT_PREV_SAME,
  next_same_answer: RT_NEXT_SAME,
  only_same_answer: RT_ONLY_SAME,
  same_answer_as: RT_SAME_AS,
  only_odd_with_answer: RT_ONLY_ODD,
  consecutive_identical: RT_CONSEC_IDENT,
  answer_of_question: RT_ANSWER_OF,
  least_common_answer: RT_LEAST_COMMON,
  most_common_answer: RT_MOST_COMMON,
  unique_answer: RT_UNIQUE,
  equal_count_as: RT_EQUAL_COUNT,
  answer_is_self: RT_SELF,
  letter_distance: RT_LETTER_DIST,
  only_true_statement: RT_TRUE_STMT,
};

// Flat representation for hot-path performance (single V8 hidden class)
export interface FlatQuestion {
  t: QuestionTypeId;
  answer: string | null;
  questionIndex: number;
  afterIndex: number;
  beforeIndex: number;
}

function flattenQuestion(r: QuestionTypeDef): FlatQuestion {
  return {
    t: RT_MAP[r.type],
    answer: "answer" in r ? r.answer : null,
    questionIndex: "questionIndex" in r ? r.questionIndex : -1,
    afterIndex: "afterIndex" in r ? r.afterIndex : -1,
    beforeIndex: "beforeIndex" in r ? r.beforeIndex : -1,
  };
}

// Pre-flattened puzzle for solver/evaluator
export interface FlatPuzzle {
  questions: FlatQuestion[];
  optionValues: (number | null)[][]; // [questionIdx][optionIdx] → semantic value
  optionClaims: (Claim | null)[][]; // for only_true_statement
  affectedBy: number[][]; // affectedBy[j] = question indices to re-check when Q_j changes
  globalIndices: number[]; // indices of questions with global rules (need all answers)
  n: number;
}

const GLOBAL_RULE_IDS = new Set<QuestionTypeId>([
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
  RT_FIRST_WITH,
  RT_LAST_WITH,
  RT_SAME_AS,
]);

let _fpCache: { puzzle: Puzzle; fp: FlatPuzzle } | null = null;

export function getFlatPuzzle(puzzle: Puzzle): FlatPuzzle {
  if (_fpCache && _fpCache.puzzle === puzzle) return _fpCache.fp;
  const fp = flattenPuzzle(puzzle);
  _fpCache = { puzzle, fp };
  return fp;
}

export function flattenPuzzle(puzzle: Puzzle): FlatPuzzle {
  const n = puzzle.questions.length;
  const questions = puzzle.questions.map((q) => flattenQuestion(q.questionType));

  // Build dependency map: affectedBy[j] = local rules to re-check when Q_j changes
  const affectedBy: number[][] = Array.from({ length: n }, () => []);
  const globalIndices: number[] = [];

  for (let i = 0; i < n; i++) {
    const r = questions[i];
    if (GLOBAL_RULE_IDS.has(r.t)) {
      globalIndices.push(i);
    } else if (r.t === RT_ANSWER_OF) {
      affectedBy[r.questionIndex].push(i);
    } else if (r.t === RT_LETTER_DIST) {
      affectedBy[r.questionIndex].push(i);
    } else if (r.t === RT_CLOSEST_AFTER || r.t === RT_COUNT_ANSWER_AFTER) {
      for (let j = r.afterIndex + 1; j < n; j++) affectedBy[j].push(i);
    } else if (r.t === RT_CLOSEST_BEFORE || r.t === RT_COUNT_ANSWER_BEFORE) {
      for (let j = 0; j < r.beforeIndex; j++) affectedBy[j].push(i);
    } else if (r.t === RT_PREV_SAME) {
      for (let j = 0; j < i; j++) affectedBy[j].push(i);
    } else if (r.t === RT_NEXT_SAME) {
      for (let j = i + 1; j < n; j++) affectedBy[j].push(i);
    } else {
      // answer_is_self or unknown — self only, no external deps
    }
    // Every question is affected by itself
    affectedBy[i].push(i);
  }

  return {
    questions,
    optionValues: puzzle.questions.map((q) => q.options.map((o) => o.value)),
    optionClaims: puzzle.questions.map((q) =>
      q.options.map((o) => ("claim" in o ? o.claim : null)),
    ),
    affectedBy,
    globalIndices,
    n,
  };
}
