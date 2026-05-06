import type { AnswerLetter, FlatPuzzle } from "./types.ts";
import {
  LETTERS,
  VOWELS,
  letterIdx,
  L2I,
  RT_COUNT_ANSWER,
  RT_COUNT_ANSWER_BEFORE,
  RT_COUNT_ANSWER_AFTER,
  RT_COUNT_VOWEL,
  RT_COUNT_CONSONANT,
  RT_CLOSEST_AFTER,
  RT_CLOSEST_BEFORE,
  RT_FIRST_WITH,
  RT_LAST_WITH,
  RT_PREV_SAME,
  RT_NEXT_SAME,
  RT_ONLY_SAME,
  RT_SAME_AS,
  RT_ONLY_ODD,
  RT_ONLY_EVEN,
  RT_CONSEC_IDENT,
  RT_EQUAL_COUNT,
  RT_ANSWER_OF,
  RT_LEAST_COMMON,
  RT_MOST_COMMON,
  RT_LETTER_DIST,
  RT_TRUE_STMT,
} from "./types.ts";

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
  "TrueStatementSelfRef",
  "TrueStatementClaimInvalid",
] as const;
export type DeduceRule = (typeof ALL_DEDUCE_RULES_INTERNAL)[number];
export const ALL_DEDUCE_RULES: readonly DeduceRule[] = ALL_DEDUCE_RULES_INTERNAL;

export type DeduceAction =
  | { type: "force"; questionIndex: number; letter: AnswerLetter }
  | { type: "eliminate"; questionIndex: number; optionIndex: number }
  | { type: "eliminateMulti"; questionMask: number; optionMask: number };

export interface DeduceResult {
  action: DeduceAction;
  rule: DeduceRule;
}

// ── Helpers ──

type Pred = (a: AnswerLetter) => boolean;

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

function crMin(cr: CountResult): number { return cr.count + cr.guaranteed; }
function crMax(cr: CountResult): number { return cr.count + cr.guaranteed + cr.possible; }

function countMatching(
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  pred: Pred,
  matchMask: number,
  from: number,
  to: number,
): CountResult {
  let count = 0;
  let guaranteed = 0;
  let possible = 0;
  for (let i = from; i < to; i++) {
    const a = answers[i];
    if (a != null) {
      if (pred(a)) count++;
    } else {
      const remaining = ~eliminated[i] & 0b11111;
      if (remaining === 0) continue;
      const matching = remaining & matchMask;
      const nonMatching = remaining & (~matchMask & 0b11111);
      if (matching !== 0 && nonMatching === 0) guaranteed++;
      else if (matching !== 0) possible++;
    }
  }
  return { count, guaranteed, possible };
}

function countPred(q: { t: number; answer: string | null }): { pred: Pred; mask: number } | null {
  switch (q.t) {
    case RT_COUNT_ANSWER:
    case RT_COUNT_ANSWER_BEFORE:
    case RT_COUNT_ANSWER_AFTER: {
      const answer = q.answer!;
      return { pred: (a) => a === answer, mask: 1 << letterIdx(answer) };
    }
    case RT_COUNT_VOWEL:
      return { pred: (a) => VOWELS.has(a), mask: 0b10001 };
    case RT_COUNT_CONSONANT:
      return { pred: (a) => !VOWELS.has(a), mask: 0b01110 };
    default:
      return null;
  }
}

function countRange(
  q: { t: number; afterIndex: number; beforeIndex: number },
  n: number,
): [number, number] {
  if (q.t === RT_COUNT_ANSWER_BEFORE) return [0, q.beforeIndex];
  if (q.t === RT_COUNT_ANSWER_AFTER) return [q.afterIndex + 1, n];
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

export function deduce(
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
): DeduceResult[] {
  return deduceWithRule(fp, answers, eliminated, null);
}

export function deduceWithRule(
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  rule: DeduceRule | null,
  exclude: DeduceRule | null = null,
): DeduceResult[] {
  const n = fp.n;
  const run = (r: DeduceRule) => (rule === null || rule === r) && exclude !== r;
  const results: DeduceResult[] = [];

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
    const cr = countMatching(answers, eliminated, cp.pred, cp.mask, from, to);

    if (run("CountSaturated")) {
      if (crMin(cr) === v && cr.possible > 0) {
        for (let j = from; j < to; j++) {
          if (answers[j] != null) continue;
          const remBits = ~eliminated[j] & 0b11111;
          if ((remBits & (~cp.mask & 0b11111)) === 0) continue;
          for (let oi = 0; oi < 5; oi++) {
            if (!isElim(eliminated, j, oi) && cp.pred(LETTERS[oi])) {
              results.push(
                res({ type: "eliminate", questionIndex: j, optionIndex: oi }, "CountSaturated"),
              );
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
                res(
                  { type: "force", questionIndex: j, letter: LETTERS[matchOi] },
                  "CountMustMatchForce",
                ),
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
              results.push(
                res({ type: "eliminate", questionIndex: j, optionIndex: oi }, "CountMustMatchElim"),
              );
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
            results.push(
              res({ type: "force", questionIndex: qi, letter: LETTERS[oi] }, "OnlyOptionLeft"),
            );
          }
        }
      }
    }

    if (run("AnswerOfForward")) {
      if (q.t === RT_ANSWER_OF && answers[q.questionIndex] != null) {
        const target = answers[q.questionIndex]!;
        const targetIdx = letterIdx(target);
        for (let oi = 0; oi < 5; oi++) {
          if (fp.optionValues[qi][oi] === targetIdx) {
            results.push(
              res({ type: "force", questionIndex: qi, letter: LETTERS[oi] }, "AnswerOfForward"),
            );
          }
        }
      }
    }

    for (let other = 0; other < n; other++) {
      const otherAns = answers[other];
      if (otherAns == null) continue;
      const otherR = fp.questions[other];

      if (run("AnswerOfReverse")) {
        if (otherR.t === RT_ANSWER_OF && otherR.questionIndex === qi) {
          const impliedIdx = fp.optionValues[other][letterIdx(otherAns)];
          if (impliedIdx != null && impliedIdx >= 0 && impliedIdx < 5) {
            results.push(
              res(
                {
                  type: "force",
                  questionIndex: qi,
                  letter: LETTERS[impliedIdx],
                },
                "AnswerOfReverse",
              ),
            );
          }
        }
      }

      if (run("SameAsReverse")) {
        if (otherR.t === RT_SAME_AS) {
          const targetQ = fp.optionValues[other][letterIdx(otherAns)];
          if (targetQ != null && targetQ >= 0 && targetQ === qi) {
            results.push(
              res({ type: "force", questionIndex: qi, letter: otherAns }, "SameAsReverse"),
            );
          }
        }
      }

      if (run("PrevNextOnlySameReverse")) {
        if (otherR.t === RT_PREV_SAME || otherR.t === RT_NEXT_SAME || otherR.t === RT_ONLY_SAME) {
          const targetQ = fp.optionValues[other][letterIdx(otherAns)];
          if (targetQ != null && targetQ >= 0 && targetQ === qi) {
            results.push(
              res(
                { type: "force", questionIndex: qi, letter: otherAns },
                "PrevNextOnlySameReverse",
              ),
            );
          }
        }
      }
    }

    if (run("LetterDistForward")) {
      if (q.t === RT_LETTER_DIST) {
        const otherAns = answers[q.questionIndex];
        if (otherAns != null) {
          const otherIdx = letterIdx(otherAns);
          let validCount = 0;
          let validLetter: AnswerLetter = "A";
          for (let oi = 0; oi < 5; oi++) {
            if (isElim(eliminated, qi, oi)) continue;
            const dist = Math.abs(oi - otherIdx);
            if (dist === fp.optionValues[qi][oi]) {
              validCount++;
              validLetter = LETTERS[oi];
            }
          }
          if (validCount === 1) {
            results.push(
              res({ type: "force", questionIndex: qi, letter: validLetter }, "LetterDistForward"),
            );
          }
        }
      }
    }

    // Reverse LetterDist: other questions' LetterDist rules constrain qi
    for (let src = 0; src < n; src++) {
      if (src === qi) continue;
      const srcR = fp.questions[src];
      if (srcR.t !== RT_LETTER_DIST || srcR.questionIndex !== qi) continue;
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
              res(
                { type: "force", questionIndex: qi, letter: LETTERS[validOi] },
                "LetterDistReverseForce",
              ),
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

    if (run("CountAllAnswered")) {
      const cp = countPred(q);
      if (cp) {
        const [from, to] = countRange(q, n);
        const cr = countMatching(answers, eliminated, cp.pred, cp.mask, from, to);
        if (cr.possible === 0) {
          for (let oi = 0; oi < 5; oi++) {
            if (isElim(eliminated, qi, oi)) continue;
            if (fp.optionValues[qi][oi] === crMin(cr)) {
              results.push(
                res({ type: "force", questionIndex: qi, letter: LETTERS[oi] }, "CountAllAnswered"),
              );
            }
          }
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

      if (srcR.t === RT_FIRST_WITH || srcR.t === RT_CLOSEST_AFTER) {
        letterOi = letterIdx(srcR.answer!);
        rangeStart = srcR.t === RT_CLOSEST_AFTER ? srcR.afterIndex + 1 : 0;
        rangeEnd = v;
      } else if (srcR.t === RT_LAST_WITH || srcR.t === RT_CLOSEST_BEFORE) {
        letterOi = letterIdx(srcR.answer!);
        rangeStart = v + 1;
        rangeEnd = srcR.t === RT_CLOSEST_BEFORE ? srcR.beforeIndex : n;
      } else if (srcR.t === RT_NEXT_SAME) {
        letterOi = letterIdx(srcAns);
        rangeStart = src + 1;
        rangeEnd = v;
      } else if (srcR.t === RT_PREV_SAME) {
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

      if (srcR.t === RT_FIRST_WITH || srcR.t === RT_CLOSEST_AFTER) {
        const letterOi = letterIdx(srcR.answer!);
        const scanStart = srcR.t === RT_CLOSEST_AFTER ? srcR.afterIndex + 1 : 0;
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
      } else if (srcR.t === RT_LAST_WITH || srcR.t === RT_CLOSEST_BEFORE) {
        const letterOi = letterIdx(srcR.answer!);
        const scanEnd = srcR.t === RT_CLOSEST_BEFORE ? srcR.beforeIndex : n;
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
  if (run("OnlyOddEvenRangeElim")) {
    for (let src = 0; src < n; src++) {
      if (answers[src] != null) continue;
      const srcR = fp.questions[src];
      if (srcR.t !== RT_ONLY_ODD && srcR.t !== RT_ONLY_EVEN) continue;
      const parity = srcR.t === RT_ONLY_ODD ? 1 : 0;
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
      if (fp.questions[i].t === RT_COUNT_VOWEL) vowelQi = i;
      if (fp.questions[i].t === RT_COUNT_CONSONANT) consonantQi = i;
    }
    if (vowelQi >= 0 && consonantQi >= 0) {
      if (run("VowelCrossElim")) {
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
          if (!has)
            results.push(
              res({ type: "eliminate", questionIndex: vowelQi, optionIndex: oi }, "VowelCrossElim"),
            );
        }
      }
      if (run("ConsonantCrossElim")) {
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
                  questionIndex: consonantQi,
                  optionIndex: oi,
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

    for (let oi = 0; oi < 5; oi++) {
      if (isElim(eliminated, qi, oi)) continue;
      const v = fp.optionValues[qi][oi];

      const cp = countPred(q);
      if (cp && q.t !== 5 /* RT_MOST_COMMON_COUNT */) {
        const [from, to] = countRange(q, n);
        const cr = countMatching(answers, eliminated, cp.pred, cp.mask, from, to);
        if (run("CountExceeded")) {
          if (v != null && crMin(cr) > v) {
            results.push(
              res({ type: "eliminate", questionIndex: qi, optionIndex: oi }, "CountExceeded"),
            );
          }
        }
        if (run("CountImpossible")) {
          if (v != null && crMax(cr) < v) {
            results.push(
              res({ type: "eliminate", questionIndex: qi, optionIndex: oi }, "CountImpossible"),
            );
          }
        }
      }

      if (q.t === RT_ANSWER_OF) {
        if (run("AnswerOfTargetRuledOut")) {
          const target = answers[q.questionIndex];
          if (target != null && v != null && letterIdx(target) !== v) {
            results.push(
              res(
                { type: "eliminate", questionIndex: qi, optionIndex: oi },
                "AnswerOfTargetRuledOut",
              ),
            );
          }
          if (target == null && v != null && v >= 0 && v < 5) {
            if (isElim(eliminated, q.questionIndex, v)) {
              results.push(
                res(
                  { type: "eliminate", questionIndex: qi, optionIndex: oi },
                  "AnswerOfTargetRuledOut",
                ),
              );
            }
          }
        }
      }

      if (q.t === RT_LETTER_DIST) {
        if (run("LetterDistImpossible")) {
          if (v != null && v > Math.max(oi, 4 - oi)) {
            results.push(
              res(
                { type: "eliminate", questionIndex: qi, optionIndex: oi },
                "LetterDistImpossible",
              ),
            );
          }
        }
        if (run("LetterDistWrong")) {
          const other = answers[q.questionIndex];
          if (other != null && v != null && Math.abs(oi - letterIdx(other)) !== v) {
            results.push(
              res({ type: "eliminate", questionIndex: qi, optionIndex: oi }, "LetterDistWrong"),
            );
          }
        }
        if (run("LetterDistNoMatch")) {
          const other = answers[q.questionIndex];
          if (other == null && v != null) {
            let anyPossible = false;
            for (let ti = 0; ti < 5; ti++) {
              if (!isElim(eliminated, q.questionIndex, ti) && Math.abs(oi - ti) === v) {
                anyPossible = true;
                break;
              }
            }
            if (!anyPossible) {
              results.push(
                res({ type: "eliminate", questionIndex: qi, optionIndex: oi }, "LetterDistNoMatch"),
              );
            }
          }
        }
      }

      if (q.t === RT_CLOSEST_AFTER || q.t === RT_FIRST_WITH) {
        const scanStart = q.t === RT_CLOSEST_AFTER ? q.afterIndex + 1 : 0;
        if (v != null) {
          if (run("FirstClosestAfterOutOfRange")) {
            if (v < scanStart || v >= n) {
              results.push(
                res(
                  { type: "eliminate", questionIndex: qi, optionIndex: oi },
                  "FirstClosestAfterOutOfRange",
                ),
              );
            }
          }
          if (v >= scanStart && v < n) {
            if (run("FirstClosestAfterWrongAnswer")) {
              if (answers[v] != null && answers[v] !== q.answer) {
                results.push(
                  res(
                    { type: "eliminate", questionIndex: qi, optionIndex: oi },
                    "FirstClosestAfterWrongAnswer",
                  ),
                );
              }
            }
            if (run("FirstClosestAfterRuledOut")) {
              if (answers[v] == null && isElim(eliminated, v, L2I[q.answer!])) {
                results.push(
                  res(
                    { type: "eliminate", questionIndex: qi, optionIndex: oi },
                    "FirstClosestAfterRuledOut",
                  ),
                );
              }
            }
            if (run("FirstClosestAfterEarlierMatch")) {
              for (let j = scanStart; j < v; j++) {
                if (answers[j] === q.answer) {
                  results.push(
                    res(
                      { type: "eliminate", questionIndex: qi, optionIndex: oi },
                      "FirstClosestAfterEarlierMatch",
                    ),
                  );
                }
              }
            }
            if (run("FirstClosestAfterSelfRef")) {
              if (LETTERS[oi] === q.answer && qi >= scanStart && qi < v) {
                results.push(
                  res(
                    { type: "eliminate", questionIndex: qi, optionIndex: oi },
                    "FirstClosestAfterSelfRef",
                  ),
                );
              }
            }
          }
        } else {
          if (run("FirstClosestAfterNoneMatch")) {
            for (let j = scanStart; j < n; j++) {
              if (answers[j] === q.answer) {
                results.push(
                  res(
                    { type: "eliminate", questionIndex: qi, optionIndex: oi },
                    "FirstClosestAfterNoneMatch",
                  ),
                );
              }
            }
          }
        }
      }

      if (q.t === RT_CLOSEST_BEFORE || q.t === RT_LAST_WITH) {
        const beforeIdx = q.t === RT_CLOSEST_BEFORE ? q.beforeIndex : n;
        if (v != null) {
          if (run("LastClosestBeforeOutOfRange")) {
            if (v < 0 || v >= beforeIdx) {
              results.push(
                res(
                  { type: "eliminate", questionIndex: qi, optionIndex: oi },
                  "LastClosestBeforeOutOfRange",
                ),
              );
            }
          }
          if (v >= 0 && v < beforeIdx) {
            if (run("LastClosestBeforeWrongAnswer")) {
              if (answers[v] != null && answers[v] !== q.answer) {
                results.push(
                  res(
                    { type: "eliminate", questionIndex: qi, optionIndex: oi },
                    "LastClosestBeforeWrongAnswer",
                  ),
                );
              }
            }
            if (run("LastClosestBeforeRuledOut")) {
              if (answers[v] == null && isElim(eliminated, v, L2I[q.answer!])) {
                results.push(
                  res(
                    { type: "eliminate", questionIndex: qi, optionIndex: oi },
                    "LastClosestBeforeRuledOut",
                  ),
                );
              }
            }
            if (run("LastClosestBeforeLaterMatch")) {
              for (let j = beforeIdx - 1; j > v; j--) {
                if (answers[j] === q.answer) {
                  results.push(
                    res(
                      { type: "eliminate", questionIndex: qi, optionIndex: oi },
                      "LastClosestBeforeLaterMatch",
                    ),
                  );
                }
              }
            }
            if (run("LastClosestBeforeSelfRef")) {
              if (LETTERS[oi] === q.answer && qi > v && qi < beforeIdx) {
                results.push(
                  res(
                    { type: "eliminate", questionIndex: qi, optionIndex: oi },
                    "LastClosestBeforeSelfRef",
                  ),
                );
              }
            }
          }
        } else {
          if (run("LastClosestBeforeNoneMatch")) {
            for (let j = 0; j < beforeIdx; j++) {
              if (answers[j] === q.answer) {
                results.push(
                  res(
                    { type: "eliminate", questionIndex: qi, optionIndex: oi },
                    "LastClosestBeforeNoneMatch",
                  ),
                );
              }
            }
          }
        }
      }

      if (q.t === RT_ONLY_ODD || q.t === RT_ONLY_EVEN) {
        const parity = q.t === RT_ONLY_ODD ? 1 : 0;
        if (v != null) {
          if (run("OnlyOddEvenWrongParity")) {
            if ((v + 1) % 2 !== parity) {
              results.push(
                res(
                  { type: "eliminate", questionIndex: qi, optionIndex: oi },
                  "OnlyOddEvenWrongParity",
                ),
              );
            }
          }
          if ((v + 1) % 2 === parity && v >= 0 && v < n) {
            if (run("OnlyOddEvenWrongAnswer")) {
              if (answers[v] != null && answers[v] !== q.answer) {
                results.push(
                  res(
                    { type: "eliminate", questionIndex: qi, optionIndex: oi },
                    "OnlyOddEvenWrongAnswer",
                  ),
                );
              }
            }
            if (run("OnlyOddEvenRuledOut")) {
              if (answers[v] == null && isElim(eliminated, v, L2I[q.answer!])) {
                results.push(
                  res(
                    { type: "eliminate", questionIndex: qi, optionIndex: oi },
                    "OnlyOddEvenRuledOut",
                  ),
                );
              }
            }
          }
        } else {
          if (run("OnlyOddEvenNoneMatch")) {
            for (let i = 0; i < n; i++) {
              if ((i + 1) % 2 === parity && answers[i] === q.answer) {
                results.push(
                  res(
                    { type: "eliminate", questionIndex: qi, optionIndex: oi },
                    "OnlyOddEvenNoneMatch",
                  ),
                );
              }
            }
          }
        }
      }

      if (q.t === RT_CONSEC_IDENT) {
        if (v != null) {
          if (run("ConsecIdentOutOfRange")) {
            if (v < 0 || v + 1 >= n) {
              results.push(
                res(
                  { type: "eliminate", questionIndex: qi, optionIndex: oi },
                  "ConsecIdentOutOfRange",
                ),
              );
            }
          }
          if (v >= 0 && v + 1 < n) {
            const possibleA = ~eliminated[v] & 0b11111;
            const possibleB = ~eliminated[v + 1] & 0b11111;
            if (run("ConsecIdentNoCommon")) {
              if ((possibleA & possibleB) === 0) {
                results.push(
                  res(
                    { type: "eliminate", questionIndex: qi, optionIndex: oi },
                    "ConsecIdentNoCommon",
                  ),
                );
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
                          questionIndex: qi,
                          optionIndex: oi,
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
                results.push(
                  res(
                    { type: "eliminate", questionIndex: qi, optionIndex: oi },
                    "ConsecIdentNonePair",
                  ),
                );
              }
            }
          }
        }
      }

      if (q.t === RT_EQUAL_COUNT) {
        if (run("EqualCountSelfRef")) {
          if (v != null && LETTERS[v] === q.answer) {
            results.push(
              res({ type: "eliminate", questionIndex: qi, optionIndex: oi }, "EqualCountSelfRef"),
            );
          }
        }
      }

      if (q.t === RT_PREV_SAME && v != null) {
        if (run("PrevSameNotBefore")) {
          if (v >= qi) {
            results.push(
              res({ type: "eliminate", questionIndex: qi, optionIndex: oi }, "PrevSameNotBefore"),
            );
          }
        }
        if (v < qi) {
          if (run("PrevSameRuledOut")) {
            if (isElim(eliminated, v, oi)) {
              results.push(
                res({ type: "eliminate", questionIndex: qi, optionIndex: oi }, "PrevSameRuledOut"),
              );
            }
          }
          if (run("PrevSameCloser")) {
            for (let j = qi - 1; j > v; j--) {
              if (answers[j] === LETTERS[oi]) {
                results.push(
                  res({ type: "eliminate", questionIndex: qi, optionIndex: oi }, "PrevSameCloser"),
                );
              }
            }
          }
        }
      }

      if (q.t === RT_NEXT_SAME && v != null) {
        if (run("NextSameNotAfter")) {
          if (v <= qi || v >= n) {
            results.push(
              res({ type: "eliminate", questionIndex: qi, optionIndex: oi }, "NextSameNotAfter"),
            );
          }
        }
        if (v > qi && v < n) {
          if (run("NextSameRuledOut")) {
            if (isElim(eliminated, v, oi)) {
              results.push(
                res({ type: "eliminate", questionIndex: qi, optionIndex: oi }, "NextSameRuledOut"),
              );
            }
          }
          if (run("NextSameCloser")) {
            for (let j = qi + 1; j < v; j++) {
              if (answers[j] === LETTERS[oi]) {
                results.push(
                  res({ type: "eliminate", questionIndex: qi, optionIndex: oi }, "NextSameCloser"),
                );
              }
            }
          }
        }
      }

      if ((q.t === RT_ONLY_SAME || q.t === RT_SAME_AS) && v != null) {
        if (run("OnlySameSelfRef")) {
          if (v === qi) {
            results.push(
              res({ type: "eliminate", questionIndex: qi, optionIndex: oi }, "OnlySameSelfRef"),
            );
          }
        }
        if (run("OnlySameRuledOut")) {
          if (v >= 0 && v < n && isElim(eliminated, v, oi)) {
            results.push(
              res({ type: "eliminate", questionIndex: qi, optionIndex: oi }, "OnlySameRuledOut"),
            );
          }
        }
      }
    }
  }

  // ── LeastCommon ──
  for (let qi = 0; qi < n; qi++) {
    if (answers[qi] != null) continue;
    const q = fp.questions[qi];
    if (q.t !== RT_LEAST_COMMON) continue;

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

      const canBeLeast = [0, 1, 2, 3, 4].every(
        (li) => li === claimed || adjMax[li] >= adjMin[claimed],
      );
      const mustBeLeast = [0, 1, 2, 3, 4].every(
        (li) => li === claimed || adjMin[li] > adjMax[claimed],
      );

      canBeLeastOpt[oi] = canBeLeast;
      mustBeLeastOpt[oi] = mustBeLeast;

      if (run("LeastCommonElim") && !canBeLeast) {
        results.push(
          res({ type: "eliminate", questionIndex: qi, optionIndex: oi }, "LeastCommonElim"),
        );
      }
    }

    if (run("LeastCommonForce")) {
      for (let oi = 0; oi < 5; oi++) {
        if (!mustBeLeastOpt[oi]) continue;
        const onlyViable = [0, 1, 2, 3, 4].every(
          (oj) => oj === oi || isElim(eliminated, qi, oj) || !canBeLeastOpt[oj],
        );
        if (onlyViable) {
          results.push(
            res({ type: "force", questionIndex: qi, letter: LETTERS[oi] }, "LeastCommonForce"),
          );
        }
      }
    }
  }

  // ── MostCommon ──
  for (let qi = 0; qi < n; qi++) {
    if (answers[qi] != null) continue;
    const q = fp.questions[qi];
    if (q.t !== RT_MOST_COMMON) continue;

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
      for (let li = 0; li < 5; li++) {
        if (li === claimed) continue;
        if (adjMin[li] > adjMax[claimed]) canBeMost = false;
        if (adjMax[li] >= adjMin[claimed]) mustBeMost = false;
      }

      canBeMostOpt[oi] = canBeMost;
      mustBeMostOpt[oi] = mustBeMost;

      if (run("MostCommonElim") && !canBeMost) {
        results.push(
          res({ type: "eliminate", questionIndex: qi, optionIndex: oi }, "MostCommonElim"),
        );
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
          results.push(
            res({ type: "force", questionIndex: qi, letter: LETTERS[oi] }, "MostCommonForce"),
          );
        }
      }
    }
  }

  // ── TrueStatement forward ──
  if (run("TrueStatementForward")) {
    for (let qi = 0; qi < n; qi++) {
      const a = answers[qi];
      if (a == null) continue;
      const q = fp.questions[qi];
      if (q.t !== RT_TRUE_STMT) continue;
      const claim = fp.optionClaims[qi][letterIdx(a)];
      if (!claim) continue;

      if (
        (claim.type === "FirstWith" || claim.type === "LastWith") &&
        claim.value >= 0 &&
        claim.value < n
      ) {
        const targetQi = claim.value;
        const targetLetter = claim.answer;
        if (answers[targetQi] == null) {
          const targetOi = letterIdx(targetLetter);
          if (!isElim(eliminated, targetQi, targetOi)) {
            results.push(
              res(
                {
                  type: "force",
                  questionIndex: targetQi,
                  letter: targetLetter,
                },
                "TrueStatementForward",
              ),
            );
          }
        }
      }

      if (claim.type === "AnswerOf" && claim.value >= 0 && claim.value < 5) {
        const targetQi = claim.questionIndex;
        const targetLetter = LETTERS[claim.value];
        if (answers[targetQi] == null) {
          const targetOi = claim.value;
          if (!isElim(eliminated, targetQi, targetOi)) {
            results.push(
              res(
                {
                  type: "force",
                  questionIndex: targetQi,
                  letter: targetLetter,
                },
                "TrueStatementForward",
              ),
            );
          }
        }
      }
    }
  }

  // ConsecIdent forward: answered ConsecIdent constrains the pair
  for (let qi = 0; qi < n; qi++) {
    if (fp.questions[qi].t !== RT_CONSEC_IDENT) continue;
    if (answers[qi] == null) continue;
    const v = fp.optionValues[qi][letterIdx(answers[qi]!)];
    if (v == null || v < 0 || v + 1 >= n) continue;
    const p = v;
    const possA = ~eliminated[p] & 0b11111;
    const possB = ~eliminated[p + 1] & 0b11111;

    if (run("ConsecIdentForwardForce")) {
      if (answers[p] != null && answers[p + 1] == null) {
        const oi = letterIdx(answers[p]);
        if (!isElim(eliminated, p + 1, oi))
          results.push(res({ type: "force", questionIndex: p + 1, letter: answers[p] }, "ConsecIdentForwardForce"));
      }
      if (answers[p + 1] != null && answers[p] == null) {
        const oi = letterIdx(answers[p + 1]!);
        if (!isElim(eliminated, p, oi))
          results.push(res({ type: "force", questionIndex: p, letter: answers[p + 1]! }, "ConsecIdentForwardForce"));
      }
    }

    if (run("ConsecIdentForwardElim")) {
      for (let oi = 0; oi < 5; oi++) {
        if (answers[p] == null && !isElim(eliminated, p, oi) && (possB & (1 << oi)) === 0)
          results.push(res({ type: "eliminate", questionIndex: p, optionIndex: oi }, "ConsecIdentForwardElim"));
        if (answers[p + 1] == null && !isElim(eliminated, p + 1, oi) && (possA & (1 << oi)) === 0)
          results.push(res({ type: "eliminate", questionIndex: p + 1, optionIndex: oi }, "ConsecIdentForwardElim"));
      }
    }

    if (run("ConsecIdentForwardBothForce")) {
      if (answers[p] == null && answers[p + 1] == null) {
        const common = possA & possB;
        if (common !== 0 && (common & (common - 1)) === 0) {
          const oi = Math.log2(common);
          results.push(res({ type: "force", questionIndex: p, letter: LETTERS[oi] }, "ConsecIdentForwardBothForce"));
          results.push(res({ type: "force", questionIndex: p + 1, letter: LETTERS[oi] }, "ConsecIdentForwardBothForce"));
        }
      }
    }
  }

  // ConsecIdent reverse: eliminate matching neighbors for impossible pairs
  if (run("ConsecIdentReverse")) {
    for (let qi = 0; qi < n; qi++) {
      if (fp.questions[qi].t !== RT_CONSEC_IDENT) continue;
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
            results.push(res({ type: "eliminate", questionIndex: j + 1, optionIndex: oi }, "ConsecIdentReverse"));
        }
        if (answers[j + 1] != null && answers[j] == null) {
          const oi = letterIdx(answers[j + 1]!);
          if (!isElim(eliminated, j, oi))
            results.push(res({ type: "eliminate", questionIndex: j, optionIndex: oi }, "ConsecIdentReverse"));
        }
      }
    }
  }

  // TrueStatement self-reference: claim contradicts the option's own letter
  if (run("TrueStatementSelfRef")) {
    for (let qi = 0; qi < n; qi++) {
      if (fp.questions[qi].t !== RT_TRUE_STMT) continue;
      for (let oi = 0; oi < 5; oi++) {
        if (isElim(eliminated, qi, oi)) continue;
        const claim = fp.optionClaims[qi][oi];
        if (!claim) continue;
        let contradicts = false;
        if ((claim.type === "FirstWith" || claim.type === "LastWith") && claim.value === qi && claim.answer !== LETTERS[oi]) {
          contradicts = true;
        }
        if (claim.type === "AnswerOf" && claim.questionIndex === qi && LETTERS[claim.value] !== LETTERS[oi]) {
          contradicts = true;
        }
        if (contradicts) {
          results.push(res({ type: "eliminate", questionIndex: qi, optionIndex: oi }, "TrueStatementSelfRef"));
        }
      }
    }
  }

  function predMask(pred: Pred): number {
    let m = 0;
    for (let i = 0; i < 5; i++) if (pred(LETTERS[i])) m |= 1 << i;
    return m;
  }

  // TrueStatement claim invalid: claim contradicts known answers
  if (run("TrueStatementClaimInvalid")) {
    for (let qi = 0; qi < n; qi++) {
      if (fp.questions[qi].t !== RT_TRUE_STMT) continue;
      for (let oi = 0; oi < 5; oi++) {
        if (isElim(eliminated, qi, oi)) continue;
        const claim = fp.optionClaims[qi][oi];
        if (!claim) continue;
        let invalid = false;
        if ((claim.type === "FirstWith" || claim.type === "LastWith") && claim.value < n) {
          const tqi = claim.value;
          if (answers[tqi] != null && answers[tqi] !== claim.answer) invalid = true;
        }
        if (claim.type === "AnswerOf" && claim.questionIndex < n) {
          if (answers[claim.questionIndex] != null && letterIdx(answers[claim.questionIndex]!) !== claim.value) invalid = true;
        }
        if (claim.type === "CountAnswer" || claim.type === "CountVowel" || claim.type === "CountConsonant" || claim.type === "CountAnswerAfter" || claim.type === "CountAnswerBefore") {
          const pred: Pred | null =
            claim.type === "CountAnswer" ? (a) => a === claim.answer :
            claim.type === "CountVowel" ? (a) => VOWELS.has(a) :
            claim.type === "CountConsonant" ? (a) => !VOWELS.has(a) :
            (claim.type === "CountAnswerAfter" || claim.type === "CountAnswerBefore") ? (a) => a === claim.answer : null;
          if (pred) {
            const from = claim.type === "CountAnswerAfter" ? claim.afterIndex + 1 : 0;
            const to = claim.type === "CountAnswerBefore" ? claim.beforeIndex : n;
            const cr = countMatching(answers, eliminated, pred, predMask(pred), from, to);
            if (crMin(cr) > claim.value || crMax(cr) < claim.value) invalid = true;
          }
        }
        if (claim.type === "MostCommon") {
          const claimedLetter = LETTERS[claim.value];
          let maxOther = 0;
          for (let li = 0; li < 5; li++) {
            if (li === claim.value) continue;
            let c = 0;
            for (let j = 0; j < n; j++) if (answers[j] === LETTERS[li]) c++;
            if (c > maxOther) maxOther = c;
          }
          let claimedMax = 0;
          for (let j = 0; j < n; j++) {
            if (answers[j] === claimedLetter) claimedMax++;
            else if (answers[j] == null && !isElim(eliminated, j, claim.value)) claimedMax++;
          }
          if (claimedMax < maxOther) invalid = true;
        }
        if (invalid) {
          results.push(res({ type: "eliminate", questionIndex: qi, optionIndex: oi }, "TrueStatementClaimInvalid"));
        }
      }
    }
  }

  return results;
}
