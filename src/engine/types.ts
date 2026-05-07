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
  difficulty: string; // "1" | "2" | "3" | "4" | "5"
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
  | { type: "CountAnswer"; answer: AnswerLetter; value: number }
  | { type: "CountConsonant"; value: number }
  | { type: "CountVowel"; value: number }
  | {
      type: "CountAnswerAfter";
      answer: AnswerLetter;
      afterIndex: number;
      value: number;
    }
  | {
      type: "CountAnswerBefore";
      answer: AnswerLetter;
      beforeIndex: number;
      value: number;
    }
  | { type: "AnswerOf"; questionIndex: number; value: number }
  | { type: "FirstWith"; answer: AnswerLetter; value: number }
  | { type: "LastWith"; answer: AnswerLetter; value: number }
  | { type: "MostCommon"; value: number };

export type QuestionTypeDef =
  // ── Counting ──
  | { type: "CountAnswer"; answer: AnswerLetter }
  | { type: "CountAnswerBefore"; answer: AnswerLetter; beforeIndex: number }
  | { type: "CountAnswerAfter"; answer: AnswerLetter; afterIndex: number }
  | { type: "CountVowel" }
  | { type: "CountConsonant" }
  | { type: "MostCommonCount" }

  // ── Positional ──
  | { type: "ClosestAfter"; afterIndex: number; answer: AnswerLetter }
  | { type: "ClosestBefore"; beforeIndex: number; answer: AnswerLetter }
  | { type: "FirstWith"; answer: AnswerLetter }
  | { type: "LastWith"; answer: AnswerLetter }
  | { type: "PrevSame" }
  | { type: "NextSame" }
  | { type: "OnlySame" }
  | { type: "SameAs" }
  | { type: "OnlyOdd"; answer: AnswerLetter }
  | { type: "OnlyEven"; answer: AnswerLetter }
  | { type: "ConsecIdent" }

  // ── Constrained (options always A-E, answer determined by solution) ──
  | { type: "AnswerOf"; questionIndex: number }
  | { type: "LeastCommon" }
  | { type: "MostCommon" }
  | { type: "Unique" }
  | { type: "EqualCount"; answer: AnswerLetter }
  | { type: "AnswerIsSelf" }

  // ── Relationship ──
  | { type: "LetterDist"; questionIndex: number }

  // ── Compound ──
  | { type: "TrueStmt" }

  // ── Global relationship ──
  | { type: "SameAsWhich"; questionIndex: number };

// Numeric question type IDs — top-level constants for V8 inlining
let rtCounter = 0;
export const RT_COUNT_ANSWER = rtCounter++;
export const RT_COUNT_ANSWER_BEFORE = rtCounter++;
export const RT_COUNT_ANSWER_AFTER = rtCounter++;
export const RT_COUNT_VOWEL = rtCounter++;
export const RT_COUNT_CONSONANT = rtCounter++;
export const RT_MOST_COMMON_COUNT = rtCounter++;
export const RT_CLOSEST_AFTER = rtCounter++;
export const RT_CLOSEST_BEFORE = rtCounter++;
export const RT_FIRST_WITH = rtCounter++;
export const RT_LAST_WITH = rtCounter++;
export const RT_PREV_SAME = rtCounter++;
export const RT_NEXT_SAME = rtCounter++;
export const RT_ONLY_SAME = rtCounter++;
export const RT_SAME_AS = rtCounter++;
export const RT_ONLY_ODD = rtCounter++;
export const RT_ONLY_EVEN = rtCounter++;
export const RT_CONSEC_IDENT = rtCounter++;
export const RT_ANSWER_OF = rtCounter++;
export const RT_LEAST_COMMON = rtCounter++;
export const RT_MOST_COMMON = rtCounter++;
export const RT_UNIQUE = rtCounter++;
export const RT_EQUAL_COUNT = rtCounter++;
export const RT_SELF = rtCounter++;
export const RT_LETTER_DIST = rtCounter++;
export const RT_TRUE_STMT = rtCounter++;
export const RT_SAME_AS_WHICH = rtCounter++;

export type QuestionTypeId = number;

// Mapping from string type names to numeric IDs (used in flattenRule)
const RT_MAP: Record<string, QuestionTypeId> = {
  CountAnswer: RT_COUNT_ANSWER,
  CountAnswerBefore: RT_COUNT_ANSWER_BEFORE,
  CountAnswerAfter: RT_COUNT_ANSWER_AFTER,
  CountVowel: RT_COUNT_VOWEL,
  CountConsonant: RT_COUNT_CONSONANT,
  MostCommonCount: RT_MOST_COMMON_COUNT,
  ClosestAfter: RT_CLOSEST_AFTER,
  ClosestBefore: RT_CLOSEST_BEFORE,
  FirstWith: RT_FIRST_WITH,
  LastWith: RT_LAST_WITH,
  PrevSame: RT_PREV_SAME,
  NextSame: RT_NEXT_SAME,
  OnlySame: RT_ONLY_SAME,
  SameAs: RT_SAME_AS,
  OnlyOdd: RT_ONLY_ODD,
  OnlyEven: RT_ONLY_EVEN,
  ConsecIdent: RT_CONSEC_IDENT,
  AnswerOf: RT_ANSWER_OF,
  LeastCommon: RT_LEAST_COMMON,
  MostCommon: RT_MOST_COMMON,
  Unique: RT_UNIQUE,
  EqualCount: RT_EQUAL_COUNT,
  AnswerIsSelf: RT_SELF,
  LetterDist: RT_LETTER_DIST,
  TrueStmt: RT_TRUE_STMT,
  SameAsWhich: RT_SAME_AS_WHICH,
};

// Flat representation for hot-path performance (single V8 hidden class)
export interface FlatQuestion {
  t: QuestionTypeId;
  answer: string | null;
  questionIndex: number;
  afterIndex: number;
  beforeIndex: number;
}

function flattenQuestion(t: QuestionTypeDef): FlatQuestion {
  return {
    t: RT_MAP[t.type],
    answer: "answer" in t ? t.answer : null,
    questionIndex: "questionIndex" in t ? t.questionIndex : -1,
    afterIndex: "afterIndex" in t ? t.afterIndex : -1,
    beforeIndex: "beforeIndex" in t ? t.beforeIndex : -1,
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
  RT_ONLY_EVEN,
  RT_FIRST_WITH,
  RT_LAST_WITH,
  RT_SAME_AS,
  RT_SAME_AS_WHICH,
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
    const q = questions[i];
    if (GLOBAL_RULE_IDS.has(q.t)) {
      globalIndices.push(i);
    } else if (q.t === RT_ANSWER_OF) {
      affectedBy[q.questionIndex].push(i);
    } else if (q.t === RT_LETTER_DIST) {
      affectedBy[q.questionIndex].push(i);
    } else if (q.t === RT_CLOSEST_AFTER || q.t === RT_COUNT_ANSWER_AFTER) {
      for (let j = q.afterIndex + 1; j < n; j++) affectedBy[j].push(i);
    } else if (q.t === RT_CLOSEST_BEFORE || q.t === RT_COUNT_ANSWER_BEFORE) {
      for (let j = 0; j < q.beforeIndex; j++) affectedBy[j].push(i);
    } else if (q.t === RT_PREV_SAME) {
      for (let j = 0; j < i; j++) affectedBy[j].push(i);
    } else if (q.t === RT_NEXT_SAME) {
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
