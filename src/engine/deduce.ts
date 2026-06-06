import type { Answer, FlatPuzzle, State } from "./types.ts";
import {
  LETTERS,
  VOWELS,
  letterIdx,
  L2I,
  QT_COUNT_ANSWER,
  QT_COUNT_ANSWER_BEFORE,
  QT_COUNT_ANSWER_AFTER,
  QT_COUNT_VOWEL,
  QT_COUNT_CONSONANT,
  QT_CLOSEST_AFTER,
  QT_CLOSEST_BEFORE,
  QT_FIRST_WITH,
  QT_LAST_WITH,
  QT_PREV_SAME,
  QT_NEXT_SAME,
  QT_ONLY_SAME,
  QT_SAME_AS,
  QT_ONLY_ODD,
  QT_ONLY_EVEN,
  QT_CONSEC_IDENT,
  QT_EQUAL_COUNT,
  QT_ANSWER_OF,
  QT_LEAST_COMMON,
  QT_MOST_COMMON,
  QT_MOST_COMMON_COUNT,
  QT_LETTER_DIST,
  QT_TRUE_STMT,
  QT_SAME_AS_WHICH,
  claimAt,
} from "./types.ts";
import { checkClaim } from "./check-answer.ts";
import { V_INVALID, V_VALID } from "./state.ts";

const ALL_DEDUCE_RULES_INTERNAL = [
  "CountSaturated",
  "CountMustMatchForce",
  "CountMustMatchElim",
  "OnlyOptionLeft",
  "AnswerOfForward",
  "AnswerOfReverse",
  "SameAsReverse",
  "PrevNextOnlySameReverse",
  "LetterDistForward",
  "LetterDistReverseForce",
  "LetterDistReverseElim",
  "CountAllAnswered",
  "PositionalRangeAnswered",
  "PositionalRangeUnanswered",
  "VowelCrossElim",
  "ConsonantCrossElim",
  "CountExceeded",
  "CountImpossible",
  "MostCommonCountElim",
  "AnswerOfTargetRuledOut",
  "LetterDistImpossible",
  "LetterDistWrong",
  "LetterDistNoMatch",
  "FirstClosestAfterOutOfRange",
  "FirstClosestAfterWrongAnswer",
  "FirstClosestAfterRuledOut",
  "FirstClosestAfterEarlierMatch",
  "FirstClosestAfterSelfRef",
  "FirstClosestAfterNoneMatch",
  "LastClosestBeforeOutOfRange",
  "LastClosestBeforeWrongAnswer",
  "LastClosestBeforeRuledOut",
  "LastClosestBeforeLaterMatch",
  "LastClosestBeforeSelfRef",
  "LastClosestBeforeNoneMatch",
  "OnlyOddEvenWrongParity",
  "OnlyOddEvenWrongAnswer",
  "OnlyOddEvenRuledOut",
  "OnlyOddEvenNoneMatch",
  "ConsecIdentOutOfRange",
  "ConsecIdentSelfRef",
  "ConsecIdentNoCommon",
  "ConsecIdentNonePair",
  "EqualCountSelfRef",
  "PrevSameNotBefore",
  "PrevSameRuledOut",
  "PrevSameCloser",
  "NextSameNotAfter",
  "NextSameRuledOut",
  "NextSameCloser",
  "OnlySameSelfRef",
  "OnlySameRuledOut",
  "UniqueAlreadyUsed",
  "LeastCommonElim",
  "LeastCommonForce",
  "TrueStatementForward",
  "OnlyOddEvenRangeElim",
  "MostCommonElim",
  "MostCommonForce",
  "ConsecIdentReverse",
  "ConsecIdentForwardForce",
  "ConsecIdentForwardElim",
  "ConsecIdentForwardBothForce",
  "EqualCountRangeElim",
  "OnlySameOtherMatch",
  "PrevSameNoneMatch",
  "NextSameNoneMatch",
  "OnlySameNoneMatch",
  "OnlySameNoneForward",
  "SameAsNegative",
  "TrueStatementSelfRef",
  "TrueStatementClaimInvalid",
  "TrueStatementClaimValid",
  "TrueStatementClaimKnownTrue",
  "SameAsWhichForward",
  "SameAsWhichReverse",
] as const;
export type DeduceRule = (typeof ALL_DEDUCE_RULES_INTERNAL)[number];
export const ALL_DEDUCE_RULES: readonly DeduceRule[] = ALL_DEDUCE_RULES_INTERNAL;

/** Position of each rule in the canonical order — mirrors the discriminant of
 *  Rust's `enum DeduceRule` so `sortDeduceResults` produces output that matches
 *  Rust's `sort_by_key(|dr| dr.rule as u8)`. */
const RULE_ORDER = new Map<DeduceRule, number>(ALL_DEDUCE_RULES_INTERNAL.map((r, i) => [r, i]));

/** Stable sort by rule order in place. */
export function sortDeduceResults(drs: DeduceResult[]): void {
  drs.sort((a, b) => (RULE_ORDER.get(a.rule) ?? 0) - (RULE_ORDER.get(b.rule) ?? 0));
}

export type DeduceAction =
  | { type: "force"; qi: number; answer: Answer }
  | { type: "eliminate"; qi: number; oi: number }
  | { type: "eliminateMulti"; questionMask: number; optionMask: number };

export interface DeduceResult {
  action: DeduceAction;
  rule: DeduceRule;
}

// ── Helpers ──

type Pred = (a: Answer) => boolean;

function isElim(eliminated: number[], qi: number, oi: number): boolean {
  return ((eliminated[qi] >> oi) & 1) === 1;
}

function remainingCount(eliminated: number): number {
  let c = 0;
  for (let i = 0; i < 5; i++) {
    if (((eliminated >> i) & 1) === 0) c++;
  }
  return c;
}

interface CountResult {
  count: number;
  guaranteed: number;
  possible: number;
}

function crMin(cr: CountResult): number {
  return cr.count + cr.guaranteed;
}
function crMax(cr: CountResult): number {
  return cr.count + cr.guaranteed + cr.possible;
}

function countMatching(
  answers: (Answer | null)[],
  eliminated: number[],
  pred: Pred,
  matchMask: number,
  from: number,
  to: number,
): CountResult {
  const nonMask = ~matchMask & 0b11111;
  let count = 0;
  let guaranteed = 0;
  let possible = 0;
  for (let i = from; i < to; i++) {
    const a = answers[i];
    if (a != null) {
      if (pred(a)) count++;
    } else {
      const remaining = ~eliminated[i] & 0b11111;
      const matching = remaining & matchMask;
      if (matching === 0) continue;
      if ((remaining & nonMask) === 0) guaranteed++;
      else possible++;
    }
  }
  return { count, guaranteed, possible };
}

function countPred(q: { t: number; answer: string | null }): { pred: Pred; mask: number } | null {
  switch (q.t) {
    case QT_COUNT_ANSWER:
    case QT_COUNT_ANSWER_BEFORE:
    case QT_COUNT_ANSWER_AFTER: {
      const answer = q.answer!;
      return { pred: (a) => a === answer, mask: 1 << letterIdx(answer) };
    }
    case QT_COUNT_VOWEL:
      return { pred: (a) => VOWELS.has(a), mask: 0b10001 };
    case QT_COUNT_CONSONANT:
      return { pred: (a) => !VOWELS.has(a), mask: 0b01110 };
    default:
      return null;
  }
}

function countRange(
  q: { t: number; afterIndex: number; beforeIndex: number },
  n: number,
): [number, number] {
  if (q.t === QT_COUNT_ANSWER_BEFORE) return [0, q.beforeIndex];
  if (q.t === QT_COUNT_ANSWER_AFTER) return [q.afterIndex + 1, n];
  return [0, n];
}

function canStillMatch(pred: Pred, eliminated: number): boolean {
  for (let oi = 0; oi < 5; oi++) {
    if (((eliminated >> oi) & 1) === 0 && pred(LETTERS[oi])) return true;
  }
  return false;
}

function res(action: DeduceAction, rule: DeduceRule): DeduceResult {
  return { action, rule };
}

// ── Main functions ──

/**
 * Sound-only deduction. Safe to use during generation: every conclusion is
 * true in any valid extension of the current state, regardless of whether the
 * puzzle has a unique solution.
 */
export function deduce(fp: FlatPuzzle, state: State): DeduceResult[] {
  return deduceImpl(fp, state, null, null, false, false);
}

/**
 * Deduction that may apply uniqueness-assuming rules (e.g. "TrueStmt has
 * exactly one true claim, so the known-true one must be it"). Only sound
 * when the puzzle is known to have a unique solution — use for play, check,
 * or tests; NOT during generation.
 */
export function deduceAssumingUnique(fp: FlatPuzzle, state: State): DeduceResult[] {
  return deduceImpl(fp, state, null, null, false, true);
}

/**
 * Fast-path variant of `deduce`: skips expensive non-fast rules. Sound-only
 * (does NOT apply uniqueness-assuming rules); used by lookahead's
 * hypothesis-testing where the hypothesis may be inconsistent.
 */
export function deduceFast(fp: FlatPuzzle, state: State): DeduceResult[] {
  return deduceImpl(fp, state, null, null, true, false);
}

export function deduceWithRule(
  fp: FlatPuzzle,
  state: State,
  rule: DeduceRule | null,
  exclude: DeduceRule | null = null,
): DeduceResult[] {
  return deduceImpl(fp, state, rule, exclude, false, true);
}

function deduceImpl(
  fp: FlatPuzzle,
  state: State,
  rule: DeduceRule | null,
  exclude: DeduceRule | null,
  fast: boolean,
  assumeUnique: boolean,
): DeduceResult[] {
  const { answers, eliminated } = state;
  const n = fp.n;
  const oc = fp.optionCount;
  const run = (r: DeduceRule) => (rule === null || rule === r) && exclude !== r;
  const results: DeduceResult[] = [];

  // Precompute countMatching once per count-typed qi. Used by CountSaturated
  // (answered qis), CountAllAnswered, and the Eliminations count-family arms
  // (both unanswered) — the latter two previously called countMatching
  // independently with identical args for the same qi.
  const countResults: (CountResult | null)[] = new Array(n).fill(null);
  for (let qi = 0; qi < n; qi++) {
    const cp = countPred(fp.questions[qi]);
    if (cp) {
      const [from, to] = countRange(fp.questions[qi], n);
      countResults[qi] = countMatching(answers, eliminated, cp.pred, cp.mask, from, to);
    }
  }

  // ── Count saturation ──
  for (let qi = 0; qi < n; qi++) {
    if (answers[qi] == null) continue;
    const q = fp.questions[qi];
    const cp = countPred(q);
    if (!cp) continue;
    const ai = letterIdx(answers[qi]!);
    const v = fp.optionValues[qi][ai];
    if (v == null) continue;
    const [from, to] = countRange(q, n);
    const cr = countResults[qi]!;

    if (run("CountSaturated")) {
      if (crMin(cr) === v && cr.possible > 0) {
        for (let j = from; j < to; j++) {
          if (answers[j] != null) continue;
          const remBits = ~eliminated[j] & 0b11111;
          if ((remBits & (~cp.mask & 0b11111)) === 0) continue;
          for (let oi = 0; oi < 5; oi++) {
            if (!isElim(eliminated, j, oi) && cp.pred(LETTERS[oi])) {
              results.push(res({ type: "eliminate", qi: j, oi }, "CountSaturated"));
            }
          }
        }
      }
    }

    if (crMax(cr) === v && cr.possible > 0) {
      if (run("CountMustMatchForce")) {
        if (cr.possible === 1) {
          for (let j = from; j < to; j++) {
            if (answers[j] != null || !canStillMatch(cp.pred, eliminated[j])) continue;
            let matchCount = 0;
            let matchOi = 0;
            for (let oi = 0; oi < 5; oi++) {
              if (!isElim(eliminated, j, oi) && cp.pred(LETTERS[oi])) {
                matchCount++;
                matchOi = oi;
              }
            }
            if (matchCount === 1) {
              results.push(
                res({ type: "force", qi: j, answer: LETTERS[matchOi] }, "CountMustMatchForce"),
              );
            }
          }
        }
      }

      if (run("CountMustMatchElim")) {
        for (let j = from; j < to; j++) {
          if (answers[j] != null || !canStillMatch(cp.pred, eliminated[j])) continue;
          for (let oi = 0; oi < 5; oi++) {
            if (!isElim(eliminated, j, oi) && !cp.pred(LETTERS[oi])) {
              results.push(res({ type: "eliminate", qi: j, oi }, "CountMustMatchElim"));
            }
          }
        }
      }
    }
  }

  // ── Forced values ──
  for (let qi = 0; qi < n; qi++) {
    if (answers[qi] != null) continue;
    const q = fp.questions[qi];

    if (run("OnlyOptionLeft")) {
      if (remainingCount(eliminated[qi]) === 1) {
        for (let oi = 0; oi < 5; oi++) {
          if (!isElim(eliminated, qi, oi)) {
            results.push(res({ type: "force", qi, answer: LETTERS[oi] }, "OnlyOptionLeft"));
          }
        }
      }
    }

    if (run("AnswerOfForward")) {
      if (q.t === QT_ANSWER_OF && answers[q.questionIndex] != null) {
        const target = answers[q.questionIndex]!;
        const targetIdx = letterIdx(target);
        let best = -1;
        for (let oi = 0; oi < 5; oi++) {
          if (fp.optionValues[qi][oi] === targetIdx) {
            if (!isElim(eliminated, qi, oi)) {
              best = oi;
              break;
            }
            if (best < 0) best = oi;
          }
        }
        if (best >= 0) {
          results.push(res({ type: "force", qi, answer: LETTERS[best] }, "AnswerOfForward"));
        }
      }
    }

    for (let other = 0; other < n; other++) {
      const otherAns = answers[other];
      if (otherAns == null) continue;
      const otherR = fp.questions[other];

      if (run("AnswerOfReverse")) {
        if (otherR.t === QT_ANSWER_OF && otherR.questionIndex === qi) {
          const impliedIdx = fp.optionValues[other][letterIdx(otherAns)];
          if (impliedIdx != null && impliedIdx >= 0 && impliedIdx < 5) {
            results.push(
              res(
                {
                  type: "force",
                  qi,
                  answer: LETTERS[impliedIdx],
                },
                "AnswerOfReverse",
              ),
            );
          }
        }
      }

      if (run("SameAsReverse")) {
        if (otherR.t === QT_SAME_AS) {
          const targetQ = fp.optionValues[other][letterIdx(otherAns)];
          if (targetQ != null && targetQ >= 0 && targetQ === qi) {
            results.push(res({ type: "force", qi, answer: otherAns }, "SameAsReverse"));
          }
        }
      }

      if (run("PrevNextOnlySameReverse")) {
        if (otherR.t === QT_PREV_SAME || otherR.t === QT_NEXT_SAME || otherR.t === QT_ONLY_SAME) {
          const targetQ = fp.optionValues[other][letterIdx(otherAns)];
          if (targetQ != null && targetQ >= 0 && targetQ === qi) {
            results.push(res({ type: "force", qi, answer: otherAns }, "PrevNextOnlySameReverse"));
          }
        }
      }
    }

    if (run("LetterDistForward")) {
      if (q.t === QT_LETTER_DIST) {
        const otherAns = answers[q.questionIndex];
        if (otherAns != null) {
          const otherIdx = letterIdx(otherAns);
          let validCount = 0;
          let validLetter: Answer = "A";
          for (let oi = 0; oi < 5; oi++) {
            if (isElim(eliminated, qi, oi)) continue;
            const dist = Math.abs(oi - otherIdx);
            if (dist === fp.optionValues[qi][oi]) {
              validCount++;
              validLetter = LETTERS[oi];
            }
          }
          if (validCount === 1) {
            results.push(res({ type: "force", qi, answer: validLetter }, "LetterDistForward"));
          }
        }
      }
    }

    // Reverse LetterDist: other questions' LetterDist rules constrain qi
    for (let src = 0; src < n; src++) {
      if (src === qi) continue;
      const srcR = fp.questions[src];
      if (srcR.t !== QT_LETTER_DIST || srcR.questionIndex !== qi) continue;
      let elimMask = 0;
      const srcAns = answers[src];
      if (srcAns != null) {
        const dist = fp.optionValues[src][letterIdx(srcAns)];
        if (dist == null) continue;
        let validCount = 0;
        let validOi = 0;
        for (let oi = 0; oi < 5; oi++) {
          if (isElim(eliminated, qi, oi)) continue;
          if (Math.abs(oi - letterIdx(srcAns)) === dist) {
            validCount++;
            validOi = oi;
          } else {
            elimMask |= 1 << oi;
          }
        }
        if (run("LetterDistReverseForce")) {
          if (validCount === 1 && elimMask !== 0) {
            results.push(
              res({ type: "force", qi, answer: LETTERS[validOi] }, "LetterDistReverseForce"),
            );
          }
        }
        if (run("LetterDistReverseElim")) {
          if (elimMask !== 0 && validCount !== 1) {
            results.push(
              res(
                {
                  type: "eliminateMulti",
                  questionMask: 1 << qi,
                  optionMask: elimMask,
                },
                "LetterDistReverseElim",
              ),
            );
          }
        }
      } else {
        for (let oi = 0; oi < 5; oi++) {
          if (isElim(eliminated, qi, oi)) continue;
          let compatible = false;
          for (let si = 0; si < 5; si++) {
            if (isElim(eliminated, src, si)) continue;
            const dist = fp.optionValues[src][si];
            if (dist != null && Math.abs(oi - si) === dist) {
              compatible = true;
              break;
            }
          }
          if (!compatible) elimMask |= 1 << oi;
        }
        if (run("LetterDistReverseElim")) {
          if (elimMask !== 0) {
            results.push(
              res(
                {
                  type: "eliminateMulti",
                  questionMask: 1 << qi,
                  optionMask: elimMask,
                },
                "LetterDistReverseElim",
              ),
            );
          }
        }
      }
    }

    if (!fast && run("CountAllAnswered")) {
      const cr = countResults[qi];
      if (cr != null && cr.possible === 0) {
        const target = crMin(cr);
        let matchOi: number | null = null;
        let matchCount = 0;
        for (let oi = 0; oi < oc; oi++) {
          if (isElim(eliminated, qi, oi)) continue;
          if (fp.optionValues[qi][oi] === target) {
            matchOi = oi;
            matchCount++;
          }
        }
        if (matchCount === 1 && matchOi !== null) {
          results.push(res({ type: "force", qi, answer: LETTERS[matchOi] }, "CountAllAnswered"));
        }
      }
    }
  }

  // ── Positional range elimination ──
  if (run("PositionalRangeAnswered")) {
    for (let src = 0; src < n; src++) {
      const srcAns = answers[src];
      if (srcAns == null) continue;
      const srcR = fp.questions[src];
      const v = fp.optionValues[src][letterIdx(srcAns)];
      if (v == null) continue;

      let letterOi: number;
      let rangeStart: number;
      let rangeEnd: number;

      if (srcR.t === QT_FIRST_WITH || srcR.t === QT_CLOSEST_AFTER) {
        letterOi = letterIdx(srcR.answer!);
        rangeStart = srcR.t === QT_CLOSEST_AFTER ? srcR.afterIndex + 1 : 0;
        rangeEnd = v;
      } else if (srcR.t === QT_LAST_WITH || srcR.t === QT_CLOSEST_BEFORE) {
        letterOi = letterIdx(srcR.answer!);
        rangeStart = v + 1;
        rangeEnd = srcR.t === QT_CLOSEST_BEFORE ? srcR.beforeIndex : n;
      } else if (srcR.t === QT_NEXT_SAME) {
        letterOi = letterIdx(srcAns);
        rangeStart = src + 1;
        rangeEnd = v;
      } else if (srcR.t === QT_PREV_SAME) {
        letterOi = letterIdx(srcAns);
        rangeStart = v + 1;
        rangeEnd = src;
      } else {
        continue;
      }
      let qMask = 0;
      for (let j = rangeStart; j < rangeEnd; j++) {
        if (answers[j] != null) continue;
        if (!isElim(eliminated, j, letterOi)) qMask |= 1 << j;
      }
      if (qMask !== 0) {
        results.push(
          res(
            {
              type: "eliminateMulti",
              questionMask: qMask,
              optionMask: 1 << letterOi,
            },
            "PositionalRangeAnswered",
          ),
        );
      }
    }
  }

  if (run("PositionalRangeUnanswered")) {
    // Unanswered positional rules: min/max of remaining options defines exclusion range
    for (let src = 0; src < n; src++) {
      if (answers[src] != null) continue;
      const srcR = fp.questions[src];

      if (srcR.t === QT_FIRST_WITH || srcR.t === QT_CLOSEST_AFTER) {
        const letterOi = letterIdx(srcR.answer!);
        const scanStart = srcR.t === QT_CLOSEST_AFTER ? srcR.afterIndex + 1 : 0;
        let minPos = n;
        for (let oi = 0; oi < 5; oi++) {
          if (isElim(eliminated, src, oi)) continue;
          const v = fp.optionValues[src][oi];
          if (v != null && v < minPos) minPos = v;
        }
        let qMask = 0;
        for (let j = scanStart; j < minPos; j++) {
          if (answers[j] != null) continue;
          if (!isElim(eliminated, j, letterOi)) qMask |= 1 << j;
        }
        if (qMask !== 0) {
          results.push(
            res(
              {
                type: "eliminateMulti",
                questionMask: qMask,
                optionMask: 1 << letterOi,
              },
              "PositionalRangeUnanswered",
            ),
          );
        }
      } else if (srcR.t === QT_LAST_WITH || srcR.t === QT_CLOSEST_BEFORE) {
        const letterOi = letterIdx(srcR.answer!);
        const scanEnd = srcR.t === QT_CLOSEST_BEFORE ? srcR.beforeIndex : n;
        let maxPos = -1;
        for (let oi = 0; oi < 5; oi++) {
          if (isElim(eliminated, src, oi)) continue;
          const v = fp.optionValues[src][oi];
          if (v != null && v > maxPos) maxPos = v;
        }
        let qMask = 0;
        for (let j = maxPos + 1; j < scanEnd; j++) {
          if (answers[j] != null) continue;
          if (!isElim(eliminated, j, letterOi)) qMask |= 1 << j;
        }
        if (qMask !== 0) {
          results.push(
            res(
              {
                type: "eliminateMulti",
                questionMask: qMask,
                optionMask: 1 << letterOi,
              },
              "PositionalRangeUnanswered",
            ),
          );
        }
      }
    }
  }

  // ── OnlyOdd/OnlyEven range elimination ──
  if (!fast && run("OnlyOddEvenRangeElim")) {
    for (let src = 0; src < n; src++) {
      if (answers[src] != null) continue;
      const srcR = fp.questions[src];
      if (srcR.t !== QT_ONLY_ODD && srcR.t !== QT_ONLY_EVEN) continue;
      const parity = srcR.t === QT_ONLY_ODD ? 1 : 0;
      const answerOi = letterIdx(srcR.answer!);

      // Collect all positions claimed by remaining options
      const claimedPositions = new Set<number>();
      for (let oi = 0; oi < 5; oi++) {
        if (isElim(eliminated, src, oi)) continue;
        const v = fp.optionValues[src][oi];
        if (v != null && v >= 0 && v < n) claimedPositions.add(v);
      }

      // Eliminate answer from all parity-matching questions NOT in claimed set
      let qMask = 0;
      for (let j = 0; j < n; j++) {
        if (j === src) continue;
        if ((j + 1) % 2 !== parity) continue;
        if (answers[j] != null) continue;
        if (claimedPositions.has(j)) continue;
        if (!isElim(eliminated, j, answerOi)) qMask |= 1 << j;
      }
      if (qMask !== 0) {
        results.push(
          res(
            {
              type: "eliminateMulti",
              questionMask: qMask,
              optionMask: 1 << answerOi,
            },
            "OnlyOddEvenRangeElim",
          ),
        );
      }
    }
  }

  // ── Vowel/consonant cross-elimination ──
  {
    let vowelQi = -1;
    let consonantQi = -1;
    for (let i = 0; i < n; i++) {
      if (answers[i] != null) continue;
      if (fp.questions[i].t === QT_COUNT_VOWEL) vowelQi = i;
      if (fp.questions[i].t === QT_COUNT_CONSONANT) consonantQi = i;
    }
    if (vowelQi >= 0 && consonantQi >= 0) {
      if (!fast && run("VowelCrossElim")) {
        for (let oi = 0; oi < 5; oi++) {
          if (isElim(eliminated, vowelQi, oi)) continue;
          const vv = fp.optionValues[vowelQi][oi];
          if (vv == null) continue;
          const need = n - vv;
          let has = false;
          for (let coi = 0; coi < 5; coi++) {
            if (
              !isElim(eliminated, consonantQi, coi) &&
              fp.optionValues[consonantQi][coi] === need
            ) {
              has = true;
              break;
            }
          }
          if (!has) results.push(res({ type: "eliminate", qi: vowelQi, oi }, "VowelCrossElim"));
        }
      }
      if (!fast && run("ConsonantCrossElim")) {
        for (let oi = 0; oi < 5; oi++) {
          if (isElim(eliminated, consonantQi, oi)) continue;
          const vv = fp.optionValues[consonantQi][oi];
          if (vv == null) continue;
          const need = n - vv;
          let has = false;
          for (let voi = 0; voi < 5; voi++) {
            if (!isElim(eliminated, vowelQi, voi) && fp.optionValues[vowelQi][voi] === need) {
              has = true;
              break;
            }
          }
          if (!has)
            results.push(
              res(
                {
                  type: "eliminate",
                  qi: consonantQi,
                  oi,
                },
                "ConsonantCrossElim",
              ),
            );
        }
      }
    }
  }

  // ── Eliminations ──
  for (let qi = 0; qi < n; qi++) {
    if (answers[qi] != null) continue;
    const q = fp.questions[qi];

    // Precompute per-question state used by multiple oi iterations.
    const countCr = countResults[qi];
    let mccBounds: { maxKnown: number; maxPossible: number } | null = null;
    if (q.t === QT_MOST_COMMON_COUNT && run("MostCommonCountElim")) {
      let maxKnown = 0;
      let maxPossible = 0;
      for (const letter of LETTERS.slice(0, fp.optionCount)) {
        const cr = countMatching(
          answers,
          eliminated,
          (a) => a === letter,
          1 << letterIdx(letter),
          0,
          n,
        );
        if (crMin(cr) > maxKnown) maxKnown = crMin(cr);
        if (crMax(cr) > maxPossible) maxPossible = crMax(cr);
      }
      mccBounds = { maxKnown, maxPossible };
    }

    for (let oi = 0; oi < 5; oi++) {
      if (isElim(eliminated, qi, oi)) continue;
      const v = fp.optionValues[qi][oi];

      if (countCr != null && q.t !== QT_MOST_COMMON_COUNT && v != null) {
        if (run("CountExceeded") && crMin(countCr) > v) {
          results.push(res({ type: "eliminate", qi, oi }, "CountExceeded"));
        }
        if (run("CountImpossible") && crMax(countCr) < v) {
          results.push(res({ type: "eliminate", qi, oi }, "CountImpossible"));
        }
      }

      if (q.t === QT_MOST_COMMON_COUNT && mccBounds != null && v != null) {
        if (v < mccBounds.maxKnown || v > mccBounds.maxPossible) {
          results.push(res({ type: "eliminate", qi, oi }, "MostCommonCountElim"));
        }
      }

      if (q.t === QT_ANSWER_OF) {
        if (run("AnswerOfTargetRuledOut")) {
          const target = answers[q.questionIndex];
          if (target != null && v != null && letterIdx(target) !== v) {
            results.push(res({ type: "eliminate", qi, oi }, "AnswerOfTargetRuledOut"));
          }
          if (target == null && v != null && v >= 0 && v < 5) {
            if (isElim(eliminated, q.questionIndex, v)) {
              results.push(res({ type: "eliminate", qi, oi }, "AnswerOfTargetRuledOut"));
            }
          }
        }
      }

      if (q.t === QT_SAME_AS_WHICH) {
        if (run("SameAsWhichForward")) {
          const refAns = answers[q.questionIndex];
          if (refAns != null && v != null && v >= 0 && v < n && v !== qi && v !== q.questionIndex) {
            const targetAns = answers[v];
            const wrong =
              targetAns != null ? targetAns !== refAns : isElim(eliminated, v, letterIdx(refAns));
            if (wrong) {
              results.push(res({ type: "eliminate", qi, oi }, "SameAsWhichForward"));
            }
          }
        }
      }

      if (q.t === QT_LETTER_DIST) {
        if (run("LetterDistImpossible")) {
          if (v != null && v > Math.max(oi, 4 - oi)) {
            results.push(res({ type: "eliminate", qi, oi }, "LetterDistImpossible"));
          }
        }
        if (run("LetterDistWrong")) {
          const other = answers[q.questionIndex];
          if (other != null && v != null && Math.abs(oi - letterIdx(other)) !== v) {
            results.push(res({ type: "eliminate", qi, oi }, "LetterDistWrong"));
          }
        }
        if (run("LetterDistNoMatch")) {
          const other = answers[q.questionIndex];
          if (other == null) {
            if (v == null) {
              results.push(res({ type: "eliminate", qi, oi }, "LetterDistNoMatch"));
            } else if (v <= Math.max(oi, 4 - oi)) {
              let anyPossible = false;
              for (let ti = 0; ti < 5; ti++) {
                if (!isElim(eliminated, q.questionIndex, ti) && Math.abs(oi - ti) === v) {
                  anyPossible = true;
                  break;
                }
              }
              if (!anyPossible) {
                results.push(res({ type: "eliminate", qi, oi }, "LetterDistNoMatch"));
              }
            }
          }
        }
      }

      if (q.t === QT_CLOSEST_AFTER || q.t === QT_FIRST_WITH) {
        const scanStart = q.t === QT_CLOSEST_AFTER ? q.afterIndex + 1 : 0;
        if (v != null) {
          if (run("FirstClosestAfterOutOfRange")) {
            if (v < scanStart || v >= n) {
              results.push(res({ type: "eliminate", qi, oi }, "FirstClosestAfterOutOfRange"));
            }
          }
          if (v >= scanStart && v < n) {
            if (run("FirstClosestAfterWrongAnswer")) {
              if (answers[v] != null && answers[v] !== q.answer) {
                results.push(res({ type: "eliminate", qi, oi }, "FirstClosestAfterWrongAnswer"));
              }
            }
            if (run("FirstClosestAfterRuledOut")) {
              if (answers[v] == null && isElim(eliminated, v, L2I[q.answer!])) {
                results.push(res({ type: "eliminate", qi, oi }, "FirstClosestAfterRuledOut"));
              }
            }
            if (run("FirstClosestAfterEarlierMatch")) {
              for (let j = scanStart; j < v; j++) {
                if (answers[j] === q.answer) {
                  results.push(res({ type: "eliminate", qi, oi }, "FirstClosestAfterEarlierMatch"));
                }
              }
            }
            if (run("FirstClosestAfterSelfRef")) {
              if (LETTERS[oi] === q.answer && qi >= scanStart && qi < v) {
                results.push(res({ type: "eliminate", qi, oi }, "FirstClosestAfterSelfRef"));
              }
            }
          }
        } else {
          if (run("FirstClosestAfterNoneMatch")) {
            for (let j = scanStart; j < n; j++) {
              if (answers[j] === q.answer) {
                results.push(res({ type: "eliminate", qi, oi }, "FirstClosestAfterNoneMatch"));
                break;
              }
            }
          }
        }
      }

      if (q.t === QT_CLOSEST_BEFORE || q.t === QT_LAST_WITH) {
        const beforeIdx = q.t === QT_CLOSEST_BEFORE ? q.beforeIndex : n;
        if (v != null) {
          if (run("LastClosestBeforeOutOfRange")) {
            if (v < 0 || v >= beforeIdx) {
              results.push(res({ type: "eliminate", qi, oi }, "LastClosestBeforeOutOfRange"));
            }
          }
          if (v >= 0 && v < beforeIdx) {
            if (run("LastClosestBeforeWrongAnswer")) {
              if (answers[v] != null && answers[v] !== q.answer) {
                results.push(res({ type: "eliminate", qi, oi }, "LastClosestBeforeWrongAnswer"));
              }
            }
            if (run("LastClosestBeforeRuledOut")) {
              if (answers[v] == null && isElim(eliminated, v, L2I[q.answer!])) {
                results.push(res({ type: "eliminate", qi, oi }, "LastClosestBeforeRuledOut"));
              }
            }
            if (run("LastClosestBeforeLaterMatch")) {
              for (let j = beforeIdx - 1; j > v; j--) {
                if (answers[j] === q.answer) {
                  results.push(res({ type: "eliminate", qi, oi }, "LastClosestBeforeLaterMatch"));
                  break;
                }
              }
            }
            if (run("LastClosestBeforeSelfRef")) {
              if (LETTERS[oi] === q.answer && qi > v && qi < beforeIdx) {
                results.push(res({ type: "eliminate", qi, oi }, "LastClosestBeforeSelfRef"));
              }
            }
          }
        } else {
          if (run("LastClosestBeforeNoneMatch")) {
            for (let j = 0; j < beforeIdx; j++) {
              if (answers[j] === q.answer) {
                results.push(res({ type: "eliminate", qi, oi }, "LastClosestBeforeNoneMatch"));
                break;
              }
            }
          }
        }
      }

      if (q.t === QT_ONLY_ODD || q.t === QT_ONLY_EVEN) {
        const parity = q.t === QT_ONLY_ODD ? 1 : 0;
        if (v != null) {
          if (run("OnlyOddEvenWrongParity")) {
            if ((v + 1) % 2 !== parity) {
              results.push(res({ type: "eliminate", qi, oi }, "OnlyOddEvenWrongParity"));
            }
          }
          if ((v + 1) % 2 === parity && v >= 0 && v < n) {
            if (run("OnlyOddEvenWrongAnswer")) {
              if (answers[v] != null && answers[v] !== q.answer) {
                results.push(res({ type: "eliminate", qi, oi }, "OnlyOddEvenWrongAnswer"));
              }
            }
            if (run("OnlyOddEvenRuledOut")) {
              if (answers[v] == null && isElim(eliminated, v, L2I[q.answer!])) {
                results.push(res({ type: "eliminate", qi, oi }, "OnlyOddEvenRuledOut"));
              }
            }
          }
        } else {
          if (run("OnlyOddEvenNoneMatch")) {
            for (let i = 0; i < n; i++) {
              if ((i + 1) % 2 === parity && answers[i] === q.answer) {
                results.push(res({ type: "eliminate", qi, oi }, "OnlyOddEvenNoneMatch"));
                break;
              }
            }
          }
        }
      }

      if (q.t === QT_CONSEC_IDENT) {
        if (v != null) {
          if (run("ConsecIdentOutOfRange")) {
            if (v < 0 || v + 1 >= n) {
              results.push(res({ type: "eliminate", qi, oi }, "ConsecIdentOutOfRange"));
            }
          }
          if (v >= 0 && v + 1 < n) {
            const possibleA = ~eliminated[v] & 0b11111;
            const possibleB = ~eliminated[v + 1] & 0b11111;
            if (run("ConsecIdentNoCommon")) {
              if ((possibleA & possibleB) === 0) {
                results.push(res({ type: "eliminate", qi, oi }, "ConsecIdentNoCommon"));
              }
            }
            if ((possibleA & possibleB) !== 0) {
              if (run("ConsecIdentSelfRef")) {
                if (v === qi || v + 1 === qi) {
                  const partner = v === qi ? v + 1 : v;
                  if (isElim(eliminated, partner, oi)) {
                    results.push(
                      res(
                        {
                          type: "eliminate",
                          qi,
                          oi,
                        },
                        "ConsecIdentSelfRef",
                      ),
                    );
                  }
                }
              }
            }
          }
        } else {
          if (run("ConsecIdentNonePair")) {
            for (let i = 0; i < n - 1; i++) {
              if (answers[i] != null && answers[i + 1] != null && answers[i] === answers[i + 1]) {
                results.push(res({ type: "eliminate", qi, oi }, "ConsecIdentNonePair"));
              }
            }
          }
        }
      }

      if (q.t === QT_EQUAL_COUNT) {
        if (run("EqualCountSelfRef")) {
          if (v != null && LETTERS[v] === q.answer) {
            results.push(res({ type: "eliminate", qi, oi }, "EqualCountSelfRef"));
          }
        }
        if (run("EqualCountRangeElim") && v != null && v >= 0 && v < 5) {
          const claimed = LETTERS[v];
          if (claimed !== q.answer) {
            const refMask = 1 << letterIdx(q.answer!);
            const claimedMask = 1 << v;
            let rc = 0,
              rr = 0,
              sc = 0,
              sr = 0;
            for (let j = 0; j < n; j++) {
              if (answers[j] != null) {
                if (answers[j] === q.answer) rc++;
                if (answers[j] === claimed) sc++;
              } else {
                if ((eliminated[j] & refMask) === 0) rr++;
                if ((eliminated[j] & claimedMask) === 0) sr++;
              }
            }
            if (rc + rr < sc || sc + sr < rc) {
              results.push(res({ type: "eliminate", qi, oi }, "EqualCountRangeElim"));
            }
          }
        }
      }

      if (q.t === QT_PREV_SAME && v == null) {
        if (run("PrevSameNoneMatch")) {
          for (let j = 0; j < qi; j++) {
            if (answers[j] === LETTERS[oi]) {
              results.push(res({ type: "eliminate", qi, oi }, "PrevSameNoneMatch"));
              break;
            }
          }
        }
      }
      if (q.t === QT_PREV_SAME && v != null) {
        if (run("PrevSameNotBefore")) {
          if (v >= qi) {
            results.push(res({ type: "eliminate", qi, oi }, "PrevSameNotBefore"));
          }
        }
        if (v < qi) {
          if (run("PrevSameRuledOut")) {
            if (isElim(eliminated, v, oi)) {
              results.push(res({ type: "eliminate", qi, oi }, "PrevSameRuledOut"));
            }
          }
          if (run("PrevSameCloser")) {
            for (let j = qi - 1; j > v; j--) {
              if (answers[j] === LETTERS[oi]) {
                results.push(res({ type: "eliminate", qi, oi }, "PrevSameCloser"));
                break;
              }
            }
          }
        }
      }

      if (q.t === QT_NEXT_SAME && v == null) {
        if (run("NextSameNoneMatch")) {
          for (let j = qi + 1; j < n; j++) {
            if (answers[j] === LETTERS[oi]) {
              results.push(res({ type: "eliminate", qi, oi }, "NextSameNoneMatch"));
              break;
            }
          }
        }
      }
      if (q.t === QT_NEXT_SAME && v != null) {
        if (run("NextSameNotAfter")) {
          if (v <= qi || v >= n) {
            results.push(res({ type: "eliminate", qi, oi }, "NextSameNotAfter"));
          }
        }
        if (v > qi && v < n) {
          if (run("NextSameRuledOut")) {
            if (isElim(eliminated, v, oi)) {
              results.push(res({ type: "eliminate", qi, oi }, "NextSameRuledOut"));
            }
          }
          if (run("NextSameCloser")) {
            for (let j = qi + 1; j < v; j++) {
              if (answers[j] === LETTERS[oi]) {
                results.push(res({ type: "eliminate", qi, oi }, "NextSameCloser"));
                break;
              }
            }
          }
        }
      }

      if ((q.t === QT_ONLY_SAME || q.t === QT_SAME_AS) && v == null) {
        if (run("OnlySameNoneMatch")) {
          for (let j = 0; j < n; j++) {
            if (j !== qi && answers[j] === LETTERS[oi]) {
              results.push(res({ type: "eliminate", qi, oi }, "OnlySameNoneMatch"));
              break;
            }
          }
        }
      }
      if ((q.t === QT_ONLY_SAME || q.t === QT_SAME_AS) && v != null) {
        if (run("OnlySameSelfRef")) {
          if (v === qi) {
            results.push(res({ type: "eliminate", qi, oi }, "OnlySameSelfRef"));
          }
        }
        if (run("OnlySameRuledOut")) {
          if (v >= 0 && v < n && isElim(eliminated, v, oi)) {
            results.push(res({ type: "eliminate", qi, oi }, "OnlySameRuledOut"));
          }
        }
        if (run("OnlySameOtherMatch") && q.t === QT_ONLY_SAME) {
          if (v >= 0 && v < n && v !== qi) {
            const letter = LETTERS[oi];
            for (let j = 0; j < n; j++) {
              if (j !== qi && j !== v && answers[j] === letter) {
                results.push(res({ type: "eliminate", qi, oi }, "OnlySameOtherMatch"));
                break;
              }
            }
          }
        }
      }
    }
  }

  // ── LeastCommon ──
  if (!fast)
    for (let qi = 0; qi < n; qi++) {
      if (answers[qi] != null) continue;
      const q = fp.questions[qi];
      if (q.t !== QT_LEAST_COMMON) continue;

      // Compute min/max possible count for each letter
      const minCount = [0, 0, 0, 0, 0];
      const maxCount = [0, 0, 0, 0, 0];
      for (let j = 0; j < n; j++) {
        if (j === qi) continue;
        if (answers[j] != null) {
          const li = letterIdx(answers[j]!);
          minCount[li]++;
          maxCount[li]++;
        } else {
          for (let li = 0; li < 5; li++) {
            if (!isElim(eliminated, j, li)) maxCount[li]++;
          }
        }
      }

      const canBeLeastOpt = [false, false, false, false, false];
      const mustBeLeastOpt = [false, false, false, false, false];

      for (let oi = 0; oi < 5; oi++) {
        if (isElim(eliminated, qi, oi)) continue;
        const v = fp.optionValues[qi][oi];
        if (v == null || v < 0 || v >= 5) continue;
        const claimed = v;
        const selfLetter = oi;

        const adjMin = [...minCount];
        const adjMax = [...maxCount];
        adjMin[selfLetter]++;
        adjMax[selfLetter]++;

        let canBeLeast = true;
        let mustBeLeast = true;
        for (let li = 0; li < oc; li++) {
          if (li === claimed) continue;
          if (adjMax[li] < adjMin[claimed]) canBeLeast = false;
          if (adjMin[li] <= adjMax[claimed]) mustBeLeast = false;
        }

        canBeLeastOpt[oi] = canBeLeast;
        mustBeLeastOpt[oi] = mustBeLeast;

        if (run("LeastCommonElim") && !canBeLeast) {
          results.push(res({ type: "eliminate", qi, oi }, "LeastCommonElim"));
        }
      }

      if (run("LeastCommonForce")) {
        for (let oi = 0; oi < 5; oi++) {
          if (!mustBeLeastOpt[oi]) continue;
          const onlyViable = [0, 1, 2, 3, 4].every(
            (oj) => oj === oi || isElim(eliminated, qi, oj) || !canBeLeastOpt[oj],
          );
          if (onlyViable) {
            results.push(res({ type: "force", qi, answer: LETTERS[oi] }, "LeastCommonForce"));
          }
        }
      }
    }

  // ── MostCommon ──
  if (!fast)
    for (let qi = 0; qi < n; qi++) {
      if (answers[qi] != null) continue;
      const q = fp.questions[qi];
      if (q.t !== QT_MOST_COMMON) continue;

      const minCount = [0, 0, 0, 0, 0];
      const maxCount = [0, 0, 0, 0, 0];
      for (let j = 0; j < n; j++) {
        if (j === qi) continue;
        if (answers[j] != null) {
          const li = letterIdx(answers[j]!);
          minCount[li]++;
          maxCount[li]++;
        } else {
          for (let li = 0; li < 5; li++) {
            if (!isElim(eliminated, j, li)) maxCount[li]++;
          }
        }
      }

      const canBeMostOpt = [false, false, false, false, false];
      const mustBeMostOpt = [false, false, false, false, false];

      for (let oi = 0; oi < 5; oi++) {
        if (isElim(eliminated, qi, oi)) continue;
        const v = fp.optionValues[qi][oi];
        if (v == null || v < 0 || v >= 5) continue;
        const claimed = v;
        const selfLetter = oi;

        const adjMin = [...minCount];
        const adjMax = [...maxCount];
        adjMin[selfLetter]++;
        adjMax[selfLetter]++;

        let canBeMost = true;
        let mustBeMost = true;
        for (let li = 0; li < oc; li++) {
          if (li === claimed) continue;
          if (adjMin[li] > adjMax[claimed]) canBeMost = false;
          if (adjMax[li] >= adjMin[claimed]) mustBeMost = false;
        }

        canBeMostOpt[oi] = canBeMost;
        mustBeMostOpt[oi] = mustBeMost;

        if (run("MostCommonElim") && !canBeMost) {
          results.push(res({ type: "eliminate", qi, oi }, "MostCommonElim"));
        }
      }

      if (run("MostCommonForce")) {
        for (let oi = 0; oi < 5; oi++) {
          if (!mustBeMostOpt[oi]) continue;
          let onlyViable = true;
          for (let oj = 0; oj < 5; oj++) {
            if (oj === oi || isElim(eliminated, qi, oj)) continue;
            if (canBeMostOpt[oj]) {
              onlyViable = false;
              break;
            }
          }
          if (onlyViable) {
            results.push(res({ type: "force", qi, answer: LETTERS[oi] }, "MostCommonForce"));
          }
        }
      }
    }

  // ── TrueStatement forward ──
  if (!fast && run("TrueStatementForward")) {
    for (let qi = 0; qi < n; qi++) {
      const a = answers[qi];
      if (a == null) continue;
      const q = fp.questions[qi];
      if (q.t !== QT_TRUE_STMT) continue;
      const claim = claimAt(fp, qi, letterIdx(a));
      if (!claim) continue;

      const cqt = claim.questionType;
      if (
        (cqt.type === "FirstWith" || cqt.type === "LastWith") &&
        claim.value >= 0 &&
        claim.value < n
      ) {
        const targetQi = claim.value;
        const targetLetter = cqt.answer;
        if (answers[targetQi] == null) {
          const targetOi = letterIdx(targetLetter);
          if (!isElim(eliminated, targetQi, targetOi)) {
            results.push(
              res(
                {
                  type: "force",
                  qi: targetQi,
                  answer: targetLetter,
                },
                "TrueStatementForward",
              ),
            );
          }
        }
      }

      if (cqt.type === "AnswerOf" && claim.value >= 0 && claim.value < 5) {
        const targetQi = cqt.questionIndex;
        const targetLetter = LETTERS[claim.value];
        if (answers[targetQi] == null) {
          const targetOi = claim.value;
          if (!isElim(eliminated, targetQi, targetOi)) {
            results.push(
              res(
                {
                  type: "force",
                  qi: targetQi,
                  answer: targetLetter,
                },
                "TrueStatementForward",
              ),
            );
          }
        }
      }
    }
  }

  // OnlySame/SameAs None forward: an answered None means qi's answer is unique, so no
  // other question can have that letter. Sound (follows from the answer), so ungated.
  if (!fast && run("OnlySameNoneForward")) {
    for (let qi = 0; qi < n; qi++) {
      if (fp.questions[qi].t !== QT_ONLY_SAME && fp.questions[qi].t !== QT_SAME_AS) continue;
      if (answers[qi] == null) continue;
      const ai = letterIdx(answers[qi]!);
      const v = fp.optionValues[qi][ai];
      if (v != null) continue;
      for (let j = 0; j < n; j++) {
        if (j === qi) continue;
        if (answers[j] == null && !isElim(eliminated, j, ai)) {
          results.push(res({ type: "eliminate", qi: j, oi: ai }, "OnlySameNoneForward"));
        }
      }
    }
  }

  // SameAs negative: non-selected option targets cannot share this question's answer.
  // Uniqueness-assuming: it relies on "exactly one option is correct" — another target
  // sharing qi's answer would be a second valid answer. Gated so it never fires during
  // generation (which must reject genuinely ambiguous puzzles).
  if (assumeUnique && run("SameAsNegative")) {
    for (let qi = 0; qi < n; qi++) {
      if (fp.questions[qi].t !== QT_SAME_AS) continue;
      if (answers[qi] == null) continue;
      const ai = letterIdx(answers[qi]!);
      const selected = fp.optionValues[qi][ai];
      // The "none" answer's sound inference (qi's letter is unique) is handled by
      // OnlySameNoneForward; this gated, uniqueness-assuming rule is for the index case.
      if (selected == null || selected < 0) continue;
      let qMask = 0;
      for (let oi = 0; oi < oc; oi++) {
        if (oi === ai) continue;
        const target = fp.optionValues[qi][oi];
        if (target == null || target < 0 || target >= n || target === qi) continue;
        if (target !== selected && answers[target] == null && !isElim(eliminated, target, ai)) {
          qMask |= 1 << target;
        }
      }
      if (qMask !== 0) {
        results.push(
          res(
            { type: "eliminateMulti", questionMask: qMask, optionMask: 1 << ai },
            "SameAsNegative",
          ),
        );
      }
    }
  }

  // ConsecIdent forward: answered ConsecIdent constrains the pair
  for (let qi = 0; qi < n; qi++) {
    if (fp.questions[qi].t !== QT_CONSEC_IDENT) continue;
    if (answers[qi] == null) continue;
    const v = fp.optionValues[qi][letterIdx(answers[qi]!)];
    if (v == null || v < 0 || v + 1 >= n) continue;
    const p = v;
    const possA = ~eliminated[p] & 0b11111;
    const possB = ~eliminated[p + 1] & 0b11111;

    if (!fast && run("ConsecIdentForwardForce")) {
      if (answers[p] != null && answers[p + 1] == null) {
        const oi = letterIdx(answers[p]);
        if (!isElim(eliminated, p + 1, oi))
          results.push(
            res({ type: "force", qi: p + 1, answer: answers[p] }, "ConsecIdentForwardForce"),
          );
      }
      if (answers[p + 1] != null && answers[p] == null) {
        const oi = letterIdx(answers[p + 1]!);
        if (!isElim(eliminated, p, oi))
          results.push(
            res({ type: "force", qi: p, answer: answers[p + 1]! }, "ConsecIdentForwardForce"),
          );
      }
    }

    if (!fast && run("ConsecIdentForwardElim")) {
      for (let oi = 0; oi < 5; oi++) {
        if (answers[p] == null && !isElim(eliminated, p, oi) && (possB & (1 << oi)) === 0)
          results.push(res({ type: "eliminate", qi: p, oi }, "ConsecIdentForwardElim"));
        if (answers[p + 1] == null && !isElim(eliminated, p + 1, oi) && (possA & (1 << oi)) === 0)
          results.push(res({ type: "eliminate", qi: p + 1, oi }, "ConsecIdentForwardElim"));
      }
    }

    if (!fast && run("ConsecIdentForwardBothForce")) {
      if (answers[p] == null && answers[p + 1] == null) {
        const common = possA & possB;
        if (common !== 0 && (common & (common - 1)) === 0) {
          const oi = Math.log2(common);
          results.push(
            res({ type: "force", qi: p, answer: LETTERS[oi] }, "ConsecIdentForwardBothForce"),
          );
          results.push(
            res({ type: "force", qi: p + 1, answer: LETTERS[oi] }, "ConsecIdentForwardBothForce"),
          );
        }
      }
    }
  }

  // SameAsWhich reverse: when answered, propagate to option target and ref question
  if (!fast && run("SameAsWhichReverse")) {
    for (let src = 0; src < n; src++) {
      const srcAns = answers[src];
      if (srcAns == null) continue;
      const srcR = fp.questions[src];
      if (srcR.t !== QT_SAME_AS_WHICH) continue;
      const on = fp.optionValues[src][letterIdx(srcAns)];
      if (on == null || on < 0 || on >= n) continue;
      const j = on;
      const qiRef = srcR.questionIndex;

      const refAns = answers[qiRef];
      if (refAns != null && answers[j] == null && !isElim(eliminated, j, letterIdx(refAns))) {
        results.push(res({ type: "force", qi: j, answer: refAns }, "SameAsWhichReverse"));
      }
      const jAns = answers[j];
      if (jAns != null && answers[qiRef] == null && !isElim(eliminated, qiRef, letterIdx(jAns))) {
        results.push(res({ type: "force", qi: qiRef, answer: jAns }, "SameAsWhichReverse"));
      }
    }
  }

  // ConsecIdent reverse: eliminate matching neighbors for impossible pairs
  if (run("ConsecIdentReverse")) {
    for (let qi = 0; qi < n; qi++) {
      if (fp.questions[qi].t !== QT_CONSEC_IDENT) continue;
      let possiblePairs = 0;
      for (let oi = 0; oi < 5; oi++) {
        if (isElim(eliminated, qi, oi)) continue;
        const v = fp.optionValues[qi][oi];
        if (v == null) continue;
        if (v + 1 < n) possiblePairs |= 1 << v;
      }
      for (let j = 0; j < n - 1; j++) {
        if (possiblePairs & (1 << j)) continue;
        if (answers[j] != null && answers[j + 1] == null) {
          const oi = letterIdx(answers[j]!);
          if (!isElim(eliminated, j + 1, oi))
            results.push(res({ type: "eliminate", qi: j + 1, oi }, "ConsecIdentReverse"));
        }
        if (answers[j + 1] != null && answers[j] == null) {
          const oi = letterIdx(answers[j + 1]!);
          if (!isElim(eliminated, j, oi))
            results.push(res({ type: "eliminate", qi: j, oi }, "ConsecIdentReverse"));
        }
      }
    }
  }

  // TrueStatement self-reference: claim contradicts the option's own letter
  if (!fast && run("TrueStatementSelfRef")) {
    for (let qi = 0; qi < n; qi++) {
      if (fp.questions[qi].t !== QT_TRUE_STMT) continue;
      for (let oi = 0; oi < 5; oi++) {
        if (isElim(eliminated, qi, oi)) continue;
        const claim = claimAt(fp, qi, oi);
        if (!claim) continue;
        const cqt = claim.questionType;
        let contradicts = false;
        if (
          (cqt.type === "FirstWith" || cqt.type === "LastWith") &&
          claim.value === qi &&
          cqt.answer !== LETTERS[oi]
        ) {
          contradicts = true;
        }
        if (
          cqt.type === "AnswerOf" &&
          cqt.questionIndex === qi &&
          LETTERS[claim.value] !== LETTERS[oi]
        ) {
          contradicts = true;
        }
        if (contradicts) {
          results.push(res({ type: "eliminate", qi, oi }, "TrueStatementSelfRef"));
        }
      }
    }
  }

  // TrueStatement claim invalid: claim contradicts known answers
  if (!fast && run("TrueStatementClaimInvalid")) {
    for (let qi = 0; qi < n; qi++) {
      if (fp.questions[qi].t !== QT_TRUE_STMT) continue;
      for (let oi = 0; oi < 5; oi++) {
        if (isElim(eliminated, qi, oi)) continue;
        const claim = claimAt(fp, qi, oi);
        if (!claim) continue;
        const v = checkClaim(fp, { answers, eliminated }, { qi, oi }, claim);
        if (v === V_INVALID) {
          results.push(res({ type: "eliminate", qi, oi }, "TrueStatementClaimInvalid"));
        }
      }
    }
  }

  // TrueStatement claim valid: if exactly one non-eliminated claim is not provably false, force it
  if (!fast && run("TrueStatementClaimValid")) {
    for (let qi = 0; qi < n; qi++) {
      if (fp.questions[qi].t !== QT_TRUE_STMT) continue;
      if (answers[qi] != null) continue;
      let survivingOi = -1;
      let survivingCount = 0;
      for (let oi = 0; oi < 5; oi++) {
        if (isElim(eliminated, qi, oi)) continue;
        const claim = claimAt(fp, qi, oi);
        if (!claim) continue;
        const hyp = { answers: answers.slice(), eliminated: eliminated.slice() };
        hyp.answers[qi] = LETTERS[oi];
        hyp.eliminated[qi] = 0b11111 ^ (1 << oi);
        const v = checkClaim(fp, hyp, { qi, oi }, claim);
        if (v !== V_INVALID) {
          survivingCount++;
          survivingOi = oi;
        }
      }
      if (survivingCount === 1 && survivingOi >= 0) {
        results.push(
          res({ type: "force", qi, answer: LETTERS[survivingOi] }, "TrueStatementClaimValid"),
        );
      }
    }
  }

  // TrueStatement claim known-true: if exactly one non-eliminated option's
  // claim is already provably true (in the current state, without hypothesizing
  // the answer), force the question's answer to that option.
  //
  // This is a uniqueness-assuming rule: it relies on "TrueStmt has exactly
  // one true claim" — but the current check_answer doesn't enforce that an
  // unselected option's claim must be false, so a puzzle can have multiple
  // simultaneously-Valid claims, in which case the brute-force solver will
  // find multiple solutions. Gated on assumeUnique so it never fires during
  // generation.
  if (!fast && assumeUnique && run("TrueStatementClaimKnownTrue")) {
    for (let qi = 0; qi < n; qi++) {
      if (fp.questions[qi].t !== QT_TRUE_STMT) continue;
      if (answers[qi] != null) continue;
      let validOi = -1;
      let validCount = 0;
      for (let oi = 0; oi < 5; oi++) {
        if (isElim(eliminated, qi, oi)) continue;
        const claim = claimAt(fp, qi, oi);
        if (!claim) continue;
        const v = checkClaim(fp, { answers, eliminated }, { qi, oi }, claim);
        if (v === V_VALID) {
          validCount++;
          validOi = oi;
        }
      }
      if (validCount === 1) {
        results.push(
          res({ type: "force", qi, answer: LETTERS[validOi] }, "TrueStatementClaimKnownTrue"),
        );
      }
    }
  }

  return results;
}
