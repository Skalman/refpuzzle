export const MAX_N = 16;

export type Answer = "A" | "B" | "C" | "D" | "E";
export const VOWELS: ReadonlySet<Answer> = new Set<Answer>(["A", "E"]);
export type OptionMark = "unmarked" | "incorrect" | "correct";
export type Marks = [OptionMark, OptionMark, OptionMark, OptionMark, OptionMark];

export const LETTERS: readonly Answer[] = ["A", "B", "C", "D", "E"];
export const FRESH_MARKS: Marks = ["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"];
export const L2I: Record<string, number> = { A: 0, B: 1, C: 2, D: 3, E: 4 };

// Fast char-code based lookup: 'A'.charCodeAt(0)=65, so L2I_FAST[65]=0, etc.
const l2i = new Int8Array(70);
l2i[65] = 0;
l2i[66] = 1;
l2i[67] = 2;
l2i[68] = 3;
l2i[69] = 4;
export function letterIdx(s: string): number {
  return l2i[s.charCodeAt(0)];
}

export interface Puzzle {
  id: string;
  title: string;
  difficulty: string;
  questions: QuestionDef[];
  optionCount?: number;
}

export interface QuestionDef {
  options: OptionDef[];
  questionType: QuestionType;
}

export type OptionDef = SimpleOption | StatementOption;

export interface SimpleOption {
  value: number | null;
}

export interface StatementOption {
  value: number | null;
  claim: Claim;
}

export interface Claim {
  questionType: QuestionType;
  value: number;
}

export interface State {
  answers: (Answer | null)[];
  eliminated: number[];
}

export interface OptionPos {
  qi: number;
  oi: number;
}

export type QuestionType =
  // ── Counting ──
  | { type: "CountAnswer"; answer: Answer }
  | { type: "CountAnswerBefore"; answer: Answer; beforeIndex: number }
  | { type: "CountAnswerAfter"; answer: Answer; afterIndex: number }
  | { type: "CountVowel" }
  | { type: "CountConsonant" }
  | { type: "MostCommonCount" }

  // ── Positional ──
  | { type: "ClosestAfter"; afterIndex: number; answer: Answer }
  | { type: "ClosestBefore"; beforeIndex: number; answer: Answer }
  | { type: "FirstWith"; answer: Answer }
  | { type: "LastWith"; answer: Answer }
  | { type: "PrevSame" }
  | { type: "NextSame" }
  | { type: "OnlySame" }
  | { type: "SameAs" }
  | { type: "OnlyOdd"; answer: Answer }
  | { type: "OnlyEven"; answer: Answer }
  | { type: "ConsecIdent" }

  // ── Letter-valued (option values are letter indices) ──
  | { type: "AnswerOf"; questionIndex: number }
  | { type: "LeastCommon" }
  | { type: "MostCommon" }
  | { type: "NoOtherHasAnswer" } // identity options: option A=0, B=1, ...
  | { type: "EqualCount"; answer: Answer }
  | { type: "AnswerIsSelf" } // identity options: option A=0, B=1, ...

  // ── Relationship ──
  | { type: "LetterDist"; questionIndex: number }

  // ── Compound ──
  | { type: "TrueStmt" }

  // ── Global relationship ──
  | { type: "SameAsWhich"; questionIndex: number };

// Numeric question type IDs — top-level constants for V8 inlining
let rtCounter = 0;
export const QT_COUNT_ANSWER = rtCounter++;
export const QT_COUNT_ANSWER_BEFORE = rtCounter++;
export const QT_COUNT_ANSWER_AFTER = rtCounter++;
export const QT_COUNT_VOWEL = rtCounter++;
export const QT_COUNT_CONSONANT = rtCounter++;
export const QT_MOST_COMMON_COUNT = rtCounter++;
export const QT_CLOSEST_AFTER = rtCounter++;
export const QT_CLOSEST_BEFORE = rtCounter++;
export const QT_FIRST_WITH = rtCounter++;
export const QT_LAST_WITH = rtCounter++;
export const QT_PREV_SAME = rtCounter++;
export const QT_NEXT_SAME = rtCounter++;
export const QT_ONLY_SAME = rtCounter++;
export const QT_SAME_AS = rtCounter++;
export const QT_ONLY_ODD = rtCounter++;
export const QT_ONLY_EVEN = rtCounter++;
export const QT_CONSEC_IDENT = rtCounter++;
export const QT_ANSWER_OF = rtCounter++;
export const QT_LEAST_COMMON = rtCounter++;
export const QT_MOST_COMMON = rtCounter++;
export const QT_NO_OTHER_HAS_ANSWER = rtCounter++;
export const QT_EQUAL_COUNT = rtCounter++;
export const QT_ANSWER_IS_SELF = rtCounter++;
export const QT_LETTER_DIST = rtCounter++;
export const QT_TRUE_STMT = rtCounter++;
export const QT_SAME_AS_WHICH = rtCounter++;

export type QuestionTypeId = number;

export type QuestionTypeName = QuestionType["type"];

export const ALL_QUESTION_TYPE_NAMES = [
  "CountAnswer",
  "CountAnswerBefore",
  "CountAnswerAfter",
  "CountVowel",
  "CountConsonant",
  "MostCommonCount",
  "ClosestAfter",
  "ClosestBefore",
  "FirstWith",
  "LastWith",
  "PrevSame",
  "NextSame",
  "OnlySame",
  "SameAs",
  "OnlyOdd",
  "OnlyEven",
  "ConsecIdent",
  "AnswerOf",
  "LeastCommon",
  "MostCommon",
  "NoOtherHasAnswer",
  "EqualCount",
  "AnswerIsSelf",
  "LetterDist",
  "TrueStmt",
  "SameAsWhich",
] as const satisfies readonly QuestionTypeName[];

// Compile-time exhaustiveness: if QuestionType grows a new variant, this errors
// until the new name is added to ALL_QUESTION_TYPE_NAMES.
type ExhaustiveQuestionTypeNames =
  Exclude<QuestionTypeName, (typeof ALL_QUESTION_TYPE_NAMES)[number]> extends never ? true : false;
// @ts-expect-error
// oxlint-disable-next-line no-unused-vars
const ALL_QUESTION_TYPE_NAMES_EXHAUSTIVE: ExhaustiveQuestionTypeNames = true;

export function isQuestionTypeWithIdentityOptions(t: QuestionTypeId): boolean {
  return t === QT_NO_OTHER_HAS_ANSWER || t === QT_ANSWER_IS_SELF;
}

// Mapping from string type names to numeric IDs (used in flattenRule)
const QT_MAP: Record<string, QuestionTypeId> = {
  CountAnswer: QT_COUNT_ANSWER,
  CountAnswerBefore: QT_COUNT_ANSWER_BEFORE,
  CountAnswerAfter: QT_COUNT_ANSWER_AFTER,
  CountVowel: QT_COUNT_VOWEL,
  CountConsonant: QT_COUNT_CONSONANT,
  MostCommonCount: QT_MOST_COMMON_COUNT,
  ClosestAfter: QT_CLOSEST_AFTER,
  ClosestBefore: QT_CLOSEST_BEFORE,
  FirstWith: QT_FIRST_WITH,
  LastWith: QT_LAST_WITH,
  PrevSame: QT_PREV_SAME,
  NextSame: QT_NEXT_SAME,
  OnlySame: QT_ONLY_SAME,
  SameAs: QT_SAME_AS,
  OnlyOdd: QT_ONLY_ODD,
  OnlyEven: QT_ONLY_EVEN,
  ConsecIdent: QT_CONSEC_IDENT,
  AnswerOf: QT_ANSWER_OF,
  LeastCommon: QT_LEAST_COMMON,
  MostCommon: QT_MOST_COMMON,
  NoOtherHasAnswer: QT_NO_OTHER_HAS_ANSWER,
  EqualCount: QT_EQUAL_COUNT,
  AnswerIsSelf: QT_ANSWER_IS_SELF,
  LetterDist: QT_LETTER_DIST,
  TrueStmt: QT_TRUE_STMT,
  SameAsWhich: QT_SAME_AS_WHICH,
};

// Flat representation for hot-path performance (single V8 hidden class)
export interface FlatQuestion {
  t: QuestionTypeId;
  answer: string | null;
  questionIndex: number;
  afterIndex: number;
  beforeIndex: number;
}

export function flattenQuestion(t: QuestionType): FlatQuestion {
  return {
    t: QT_MAP[t.type],
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
  optionCount: number;
}

const GLOBAL_RULE_IDS = new Set<QuestionTypeId>([
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
  QT_ONLY_EVEN,
  QT_FIRST_WITH,
  QT_LAST_WITH,
  QT_SAME_AS,
  QT_SAME_AS_WHICH,
]);

let fpCache: { puzzle: Puzzle; fp: FlatPuzzle } | null = null;

export function getFlatPuzzle(puzzle: Puzzle): FlatPuzzle {
  if (fpCache && fpCache.puzzle === puzzle) return fpCache.fp;
  const fp = flattenPuzzle(puzzle);
  fpCache = { puzzle, fp };
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
    } else if (q.t === QT_ANSWER_OF) {
      affectedBy[q.questionIndex].push(i);
    } else if (q.t === QT_LETTER_DIST) {
      affectedBy[q.questionIndex].push(i);
    } else if (q.t === QT_CLOSEST_AFTER || q.t === QT_COUNT_ANSWER_AFTER) {
      for (let j = q.afterIndex + 1; j < n; j++) affectedBy[j].push(i);
    } else if (q.t === QT_CLOSEST_BEFORE || q.t === QT_COUNT_ANSWER_BEFORE) {
      for (let j = 0; j < q.beforeIndex; j++) affectedBy[j].push(i);
    } else if (q.t === QT_PREV_SAME) {
      for (let j = 0; j < i; j++) affectedBy[j].push(i);
    } else if (q.t === QT_NEXT_SAME) {
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
    optionCount: puzzle.optionCount ?? 5,
  };
}
