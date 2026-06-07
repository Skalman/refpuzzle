import type { Answer, FlatPuzzle, State } from "./types.ts";
import {
  LETTERS,
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
  "MostCommonCountElim",
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
  "TrueStatementSelfRef",
  "TrueStatementClaimInvalid",
  "TrueStatementClaimValid",
  "TrueStatementClaimKnownTrue",
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

const VOWEL_MASK = 0b10001;
const CONSONANT_MASK = 0b01110;

function isElim(eliminated: number[], qi: number, oi: number): boolean {
  return ((eliminated[qi] >> oi) & 1) === 1;
}

function remainingCount(eliminated: number): number {
  let v = ~eliminated & 0b11111;
  let c = 0;
  while (v !== 0) {
    v &= v - 1;
    c++;
  }
  return c;
}

function maskContains(mask: number, oi: number): boolean {
  return ((mask >> oi) & 1) !== 0;
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

/** Compute (count, guaranteed, possible) for a mask-selected predicate over [from, to). */
function countMatchingMask(
  answers: (Answer | null)[],
  eliminated: number[],
  mask: number,
  from: number,
  to: number,
): CountResult {
  const nonMask = ~mask & 0b11111;
  let count = 0;
  let guaranteed = 0;
  let possible = 0;
  for (let i = from; i < to; i++) {
    const a = answers[i];
    if (a != null) {
      if (maskContains(mask, letterIdx(a))) count++;
    } else {
      const remaining = ~eliminated[i] & 0b11111;
      const matching = remaining & mask;
      if (matching === 0) continue;
      if ((remaining & nonMask) === 0) guaranteed++;
      else possible++;
    }
  }
  return { count, guaranteed, possible };
}

/** Return the unique i in [from, to) satisfying f, or -1 if 0 or >1 do. */
function exactlyOne(from: number, to: number, f: (i: number) => boolean): number {
  let found = -1;
  for (let i = from; i < to; i++) {
    if (f(i)) {
      if (found !== -1) return -1;
      found = i;
    }
  }
  return found;
}

/** Whole-puzzle per-letter counts. `known[i]` = number of qi answered with
 *  letter i; `max[i]` = `known[i]` + number of unanswered qi where letter i
 *  is still possible. */
function computeLetterCounts(
  answers: (Answer | null)[],
  eliminated: number[],
  n: number,
): [number[], number[]] {
  const known = [0, 0, 0, 0, 0];
  const max = [0, 0, 0, 0, 0];
  for (let j = 0; j < n; j++) {
    const a = answers[j];
    if (a != null) {
      const li = letterIdx(a);
      known[li]++;
      max[li]++;
    } else {
      for (let li = 0; li < 5; li++) {
        if (!isElim(eliminated, j, li)) max[li]++;
      }
    }
  }
  return [known, max];
}

type Push = (rule: DeduceRule, action: DeduceAction) => void;

// ── Per-qi dispatch helpers ──

/** Per-qi count-family dispatch (CountAnswer/Before/After/Vowel/Consonant).
 *  Handles the answered case (CountSaturated / CountMustMatch{Force,Elim}) and
 *  the unanswered case (CountAllAnswered + per-option CountExceeded/Impossible). */
function applyCount(
  fp: FlatPuzzle,
  state: State,
  push: Push,
  qi: number,
  mask: number,
  from: number,
  to: number,
  full: boolean,
): void {
  const { answers, eliminated } = state;
  const cr = countMatchingMask(answers, eliminated, mask, from, to);
  const ans = answers[qi];

  if (ans != null) {
    // Answered count qi: CountSaturated / CountMustMatch{Force,Elim}.
    const sv = fp.optionValues[qi][letterIdx(ans)];
    if (sv == null) return;
    const on = sv;

    if (crMin(cr) === on && cr.possible > 0) {
      for (let j = from; j < to; j++) {
        if (answers[j] != null) continue;
        const remBits = ~eliminated[j] & 0b11111;
        if ((remBits & (~mask & 0b11111)) === 0) continue;
        for (let oi = 0; oi < 5; oi++) {
          if (!isElim(eliminated, j, oi) && maskContains(mask, oi)) {
            push("CountSaturated", { type: "eliminate", qi: j, oi });
          }
        }
      }
    }

    if (crMax(cr) === on && cr.possible > 0) {
      if (cr.possible === 1) {
        for (let j = from; j < to; j++) {
          if (answers[j] != null) continue;
          if ((eliminated[j] & mask) === mask) continue;
          const oi = exactlyOne(0, 5, (o) => !isElim(eliminated, j, o) && maskContains(mask, o));
          if (oi !== -1) {
            push("CountMustMatchForce", { type: "force", qi: j, answer: LETTERS[oi] });
          }
        }
      }

      for (let j = from; j < to; j++) {
        if (answers[j] != null) continue;
        if ((eliminated[j] & mask) === mask) continue;
        for (let oi = 0; oi < 5; oi++) {
          if (!isElim(eliminated, j, oi) && !maskContains(mask, oi)) {
            push("CountMustMatchElim", { type: "eliminate", qi: j, oi });
          }
        }
      }
    }
  } else {
    // Unanswered count qi: CountAllAnswered + per-option CountExceeded/Impossible.
    if (full && cr.possible === 0) {
      const target = crMin(cr);
      const oi = exactlyOne(
        0,
        fp.optionCount,
        (o) => !isElim(eliminated, qi, o) && fp.optionValues[qi][o] === target,
      );
      if (oi !== -1) {
        push("CountAllAnswered", { type: "force", qi, answer: LETTERS[oi] });
      }
    }

    for (let oi = 0; oi < 5; oi++) {
      if (isElim(eliminated, qi, oi)) continue;
      const sv = fp.optionValues[qi][oi];
      if (sv == null) {
        // NONE on a count option is meaningless: any claim that count == null
        // is impossible.
        push("CountExceeded", { type: "eliminate", qi, oi });
        continue;
      }
      if (crMin(cr) > sv) {
        push("CountExceeded", { type: "eliminate", qi, oi });
      }
      if (crMax(cr) < sv) {
        push("CountImpossible", { type: "eliminate", qi, oi });
      }
    }
  }
}

/** Per-qi OnlyOdd/OnlyEven dispatch (qi must be unanswered). `parity` is
 *  1 for OnlyOdd (1-indexed odd = 0-indexed even positions), 0 for OnlyEven. */
function applyOnlyOddEven(
  fp: FlatPuzzle,
  state: State,
  push: Push,
  qi: number,
  answer: string,
  parity: number,
  full: boolean,
): void {
  const n = fp.n;
  const { answers, eliminated } = state;
  const answerOi = letterIdx(answer);

  for (let oi = 0; oi < 5; oi++) {
    if (isElim(eliminated, qi, oi)) continue;
    const sv = fp.optionValues[qi][oi];
    if (sv != null) {
      const pos = sv;
      if ((pos + 1) % 2 !== parity) {
        push("OnlyOddEvenWrongParity", { type: "eliminate", qi, oi });
      }
      if ((pos + 1) % 2 === parity && pos < n) {
        const pa = answers[pos];
        if (pa != null && pa !== answer) {
          push("OnlyOddEvenWrongAnswer", { type: "eliminate", qi, oi });
        }
        if (pa == null && isElim(eliminated, pos, answerOi)) {
          push("OnlyOddEvenRuledOut", { type: "eliminate", qi, oi });
        }
      }
    } else {
      let found = false;
      for (let i = 0; i < n; i++) {
        if ((i + 1) % 2 === parity && answers[i] === answer) {
          found = true;
          break;
        }
      }
      if (found) {
        push("OnlyOddEvenNoneMatch", { type: "eliminate", qi, oi });
      }
    }
  }

  // OnlyOddEvenRangeElim (full mode): positions with the right parity
  // that aren't reachable from this OnlyOdd/Even's remaining options
  // can't hold `answer`.
  if (full) {
    let claimed = 0;
    for (let oi = 0; oi < 5; oi++) {
      if (isElim(eliminated, qi, oi)) continue;
      const sv = fp.optionValues[qi][oi];
      if (sv != null && sv < n) claimed |= 1 << sv;
    }
    let qMask = 0;
    for (let j = 0; j < n; j++) {
      if (j === qi) continue;
      if ((j + 1) % 2 !== parity) continue;
      if (answers[j] != null) continue;
      if ((claimed >> j) & 1) continue;
      if (!isElim(eliminated, j, answerOi)) qMask |= 1 << j;
    }
    if (qMask !== 0) {
      push("OnlyOddEvenRangeElim", {
        type: "eliminateMulti",
        questionMask: qMask,
        optionMask: 1 << answerOi,
      });
    }
  }
}

/** Forward positional dispatch (FirstWith / ClosestAfter).
 *  `scanStart` = 0 for FirstWith, `afterIndex + 1` for ClosestAfter. */
function applyPositionalForward(
  fp: FlatPuzzle,
  state: State,
  push: Push,
  qi: number,
  answer: string,
  scanStart: number,
): void {
  const n = fp.n;
  const { answers, eliminated } = state;
  const ans = answers[qi];
  const letterOi = letterIdx(answer);

  if (ans != null) {
    // PositionalRangeAnswered: positions before the claimed target can't have `answer`.
    const sv = fp.optionValues[qi][letterIdx(ans)];
    if (sv != null) {
      let qMask = 0;
      for (let j = scanStart; j < sv; j++) {
        if (answers[j] != null) continue;
        if (!isElim(eliminated, j, letterOi)) qMask |= 1 << j;
      }
      if (qMask !== 0) {
        push("PositionalRangeAnswered", {
          type: "eliminateMulti",
          questionMask: qMask,
          optionMask: 1 << letterOi,
        });
      }
    }
  } else {
    // Per-option elim.
    for (let oi = 0; oi < 5; oi++) {
      if (isElim(eliminated, qi, oi)) continue;
      const sv = fp.optionValues[qi][oi];
      if (sv != null) {
        const pos = sv;
        if (pos < scanStart || pos >= n) {
          push("FirstClosestAfterOutOfRange", { type: "eliminate", qi, oi });
        }
        if (pos >= scanStart && pos < n) {
          const pa = answers[pos];
          if (pa != null && pa !== answer) {
            push("FirstClosestAfterWrongAnswer", { type: "eliminate", qi, oi });
          }
          if (pa == null && isElim(eliminated, pos, letterOi)) {
            push("FirstClosestAfterRuledOut", { type: "eliminate", qi, oi });
          }
          for (let j = scanStart; j < pos; j++) {
            if (answers[j] === answer) {
              push("FirstClosestAfterEarlierMatch", { type: "eliminate", qi, oi });
            }
          }
          if (oi === letterOi && qi >= scanStart && qi < pos) {
            push("FirstClosestAfterSelfRef", { type: "eliminate", qi, oi });
          }
        }
      } else {
        let found = false;
        for (let j = scanStart; j < n; j++) {
          if (answers[j] === answer) {
            found = true;
            break;
          }
        }
        if (found) {
          push("FirstClosestAfterNoneMatch", { type: "eliminate", qi, oi });
        }
      }
    }
    // PositionalRangeUnanswered: positions before the minimum remaining claim can't have `answer`.
    let minPos = n;
    for (let oi = 0; oi < 5; oi++) {
      if (isElim(eliminated, qi, oi)) continue;
      const sv = fp.optionValues[qi][oi];
      if (sv != null && sv < minPos) minPos = sv;
    }
    let qMask = 0;
    for (let j = scanStart; j < minPos; j++) {
      if (answers[j] != null) continue;
      if (!isElim(eliminated, j, letterOi)) qMask |= 1 << j;
    }
    if (qMask !== 0) {
      push("PositionalRangeUnanswered", {
        type: "eliminateMulti",
        questionMask: qMask,
        optionMask: 1 << letterOi,
      });
    }
  }
}

/** Backward positional dispatch (LastWith / ClosestBefore).
 *  `scanEnd` = n for LastWith, `beforeIndex` for ClosestBefore. */
function applyPositionalBackward(
  fp: FlatPuzzle,
  state: State,
  push: Push,
  qi: number,
  answer: string,
  scanEnd: number,
): void {
  const { answers, eliminated } = state;
  const ans = answers[qi];
  const letterOi = letterIdx(answer);

  if (ans != null) {
    // PositionalRangeAnswered: positions after the claimed target can't have `answer`.
    const sv = fp.optionValues[qi][letterIdx(ans)];
    if (sv != null) {
      let qMask = 0;
      for (let j = sv + 1; j < scanEnd; j++) {
        if (answers[j] != null) continue;
        if (!isElim(eliminated, j, letterOi)) qMask |= 1 << j;
      }
      if (qMask !== 0) {
        push("PositionalRangeAnswered", {
          type: "eliminateMulti",
          questionMask: qMask,
          optionMask: 1 << letterOi,
        });
      }
    }
  } else {
    // Per-option elim.
    for (let oi = 0; oi < 5; oi++) {
      if (isElim(eliminated, qi, oi)) continue;
      const sv = fp.optionValues[qi][oi];
      if (sv != null) {
        const pos = sv;
        if (pos >= scanEnd) {
          push("LastClosestBeforeOutOfRange", { type: "eliminate", qi, oi });
        }
        if (pos < scanEnd) {
          const pa = answers[pos];
          if (pa != null && pa !== answer) {
            push("LastClosestBeforeWrongAnswer", { type: "eliminate", qi, oi });
          }
          if (pa == null && isElim(eliminated, pos, letterOi)) {
            push("LastClosestBeforeRuledOut", { type: "eliminate", qi, oi });
          }
          let laterMatch = false;
          for (let j = scanEnd - 1; j > pos; j--) {
            if (answers[j] === answer) {
              laterMatch = true;
              break;
            }
          }
          if (laterMatch) {
            push("LastClosestBeforeLaterMatch", { type: "eliminate", qi, oi });
          }
          if (oi === letterOi && qi > pos && qi < scanEnd) {
            push("LastClosestBeforeSelfRef", { type: "eliminate", qi, oi });
          }
        }
      } else {
        let found = false;
        for (let j = 0; j < scanEnd; j++) {
          if (answers[j] === answer) {
            found = true;
            break;
          }
        }
        if (found) {
          push("LastClosestBeforeNoneMatch", { type: "eliminate", qi, oi });
        }
      }
    }
    // PositionalRangeUnanswered: positions after the maximum remaining claim can't have `answer`.
    let maxPos: number | null = null;
    for (let oi = 0; oi < 5; oi++) {
      if (isElim(eliminated, qi, oi)) continue;
      const sv = fp.optionValues[qi][oi];
      if (sv != null && (maxPos === null || sv > maxPos)) maxPos = sv;
    }
    const scanStart = maxPos === null ? 0 : maxPos + 1;
    let qMask = 0;
    for (let j = scanStart; j < scanEnd; j++) {
      if (answers[j] != null) continue;
      if (!isElim(eliminated, j, letterOi)) qMask |= 1 << j;
    }
    if (qMask !== 0) {
      push("PositionalRangeUnanswered", {
        type: "eliminateMulti",
        questionMask: qMask,
        optionMask: 1 << letterOi,
      });
    }
  }
}

/** Rules shared by `SameAs` and `OnlySame` arms: reverse force, NoneForward
 *  (answered qi), and the common per-option elims (NoneMatch / SelfRef /
 *  RuledOut) for unanswered qi. `reverseRule` distinguishes the two arms'
 *  reverse-force rule name (SameAsReverse vs PrevNextOnlySameReverse). */
function applySameShared(
  fp: FlatPuzzle,
  state: State,
  push: Push,
  qi: number,
  reverseRule: DeduceRule,
  full: boolean,
): void {
  const n = fp.n;
  const { answers, eliminated } = state;
  const ans = answers[qi];

  if (ans != null) {
    const ai = letterIdx(ans);
    const sv = fp.optionValues[qi][ai];
    // Reverse: qi answered with an index → force that target qi to qi's letter.
    if (sv != null) {
      const targetQi = sv;
      if (targetQi < n && answers[targetQi] == null) {
        push(reverseRule, { type: "force", qi: targetQi, answer: ans });
      }
    }

    // OnlySameNoneForward: an answered None means qi's answer is unique,
    // so no other question can have that letter. Sound, ungated.
    if (full && sv == null) {
      for (let j = 0; j < n; j++) {
        if (j === qi) continue;
        if (answers[j] == null && !isElim(eliminated, j, ai)) {
          push("OnlySameNoneForward", { type: "eliminate", qi: j, oi: ai });
        }
      }
    }
  } else {
    // Per-option elim (qi unanswered): rules shared by both arms.
    for (let oi = 0; oi < 5; oi++) {
      if (isElim(eliminated, qi, oi)) continue;
      const sv = fp.optionValues[qi][oi];
      if (sv == null) {
        let found = false;
        for (let j = 0; j < n; j++) {
          if (j !== qi && answers[j] === LETTERS[oi]) {
            found = true;
            break;
          }
        }
        if (found) {
          push("OnlySameNoneMatch", { type: "eliminate", qi, oi });
        }
      } else {
        const pos = sv;
        if (pos === qi) {
          push("OnlySameSelfRef", { type: "eliminate", qi, oi });
        }
        if (pos < n && isElim(eliminated, pos, oi)) {
          push("OnlySameRuledOut", { type: "eliminate", qi, oi });
        }
      }
    }
  }
}

/** PrevSame / NextSame dispatch. Reverse force (when answered) into the
 *  referenced position, PositionalRangeAnswered over the open interval between
 *  qi and the target, plus per-option elims for unanswered qi.
 *
 *  `[rangeStart, rangeEnd)` is the candidate range. `between(x)` returns the
 *  open interval `[lo, hi)` between qi and x. */
function applyPrevOrNextSame(
  fp: FlatPuzzle,
  state: State,
  push: Push,
  qi: number,
  rangeStart: number,
  rangeEnd: number,
  between: (x: number) => [number, number],
  noneRule: DeduceRule,
  outRule: DeduceRule,
  ruledOutRule: DeduceRule,
  closerRule: DeduceRule,
): void {
  const n = fp.n;
  const { answers, eliminated } = state;
  const ans = answers[qi];

  if (ans != null) {
    const ai = letterIdx(ans);
    const sv = fp.optionValues[qi][ai];
    if (sv != null) {
      const targetQi = sv;
      if (targetQi < n && answers[targetQi] == null) {
        push("PrevNextOnlySameReverse", { type: "force", qi: targetQi, answer: ans });
      }
      // PositionalRangeAnswered: positions strictly between qi and target
      // can't hold qi's letter.
      const letterOi = ai;
      const [bLo, bHi] = between(targetQi);
      let qMask = 0;
      for (let j = bLo; j < bHi; j++) {
        if (answers[j] != null) continue;
        if (!isElim(eliminated, j, letterOi)) qMask |= 1 << j;
      }
      if (qMask !== 0) {
        push("PositionalRangeAnswered", {
          type: "eliminateMulti",
          questionMask: qMask,
          optionMask: 1 << letterOi,
        });
      }
    }
  } else {
    // Per-option elim (qi unanswered).
    for (let oi = 0; oi < 5; oi++) {
      if (isElim(eliminated, qi, oi)) continue;
      const sv = fp.optionValues[qi][oi];
      if (sv == null) {
        let found = false;
        for (let j = rangeStart; j < rangeEnd; j++) {
          if (answers[j] === LETTERS[oi]) {
            found = true;
            break;
          }
        }
        if (found) {
          push(noneRule, { type: "eliminate", qi, oi });
        }
      } else {
        const pos = sv;
        if (pos < rangeStart || pos >= rangeEnd) {
          push(outRule, { type: "eliminate", qi, oi });
        }
        if (pos >= rangeStart && pos < rangeEnd) {
          if (isElim(eliminated, pos, oi)) {
            push(ruledOutRule, { type: "eliminate", qi, oi });
          }
          const [bLo, bHi] = between(pos);
          let closerMatch = false;
          for (let j = bLo; j < bHi; j++) {
            if (answers[j] === LETTERS[oi]) {
              closerMatch = true;
              break;
            }
          }
          if (closerMatch) {
            push(closerRule, { type: "eliminate", qi, oi });
          }
        }
      }
    }
  }
}

/** LeastCommon / MostCommon dispatch. Per-option, check whether `oi`'s claimed
 *  count "could be" the extremum (others' max within reach) and "must be" the
 *  extremum (others' min strictly past). Emit Elim for ¬could, Force when
 *  exactly one option could-be AND it must-be. */
function applyExtremumCount(
  fp: FlatPuzzle,
  state: State,
  push: Push,
  qi: number,
  isLeast: boolean,
  letterKnown: number[],
  letterMax: number[],
  elimRule: DeduceRule,
  forceRule: DeduceRule,
): void {
  const { eliminated } = state;
  const oc = fp.optionCount;

  // qi is unanswered; remove its contribution to letter_max so adj_*
  // doesn't double-count when we test "if qi were `oi`".
  const minCount = letterKnown.slice();
  const maxCount = letterMax.slice();
  for (let li = 0; li < 5; li++) {
    if (!isElim(eliminated, qi, li)) maxCount[li]--;
  }

  let canMask = 0;
  let mustMask = 0;
  for (let oi = 0; oi < 5; oi++) {
    if (isElim(eliminated, qi, oi)) continue;
    const v = fp.optionValues[qi][oi];
    if (v == null || v >= 5) continue;
    const claimed = v;

    const adjMin = minCount.slice();
    const adjMax = maxCount.slice();
    adjMin[oi]++;
    adjMax[oi]++;

    let canBeExtreme = true;
    let mustBeExtreme = true;
    for (let li = 0; li < oc; li++) {
      if (li === claimed) continue;
      // For Least: a=li, b=claimed. For Most: a=claimed, b=li.
      const a = isLeast ? li : claimed;
      const b = isLeast ? claimed : li;
      if (adjMax[a] < adjMin[b]) canBeExtreme = false;
      if (adjMin[a] <= adjMax[b]) mustBeExtreme = false;
    }

    if (canBeExtreme) canMask |= 1 << oi;
    if (mustBeExtreme) mustMask |= 1 << oi;
    if (!canBeExtreme) {
      push(elimRule, { type: "eliminate", qi, oi });
    }
  }

  // Force when exactly one option could-be the extremum AND it must-be.
  if (canMask !== 0 && (canMask & (canMask - 1)) === 0) {
    let oi = 0;
    let m = canMask;
    while ((m & 1) === 0) {
      m >>= 1;
      oi++;
    }
    if ((mustMask & (1 << oi)) !== 0) {
      push(forceRule, { type: "force", qi, answer: LETTERS[oi] });
    }
  }
}

/** Vowel + Consonant = n cross-elim. Fires once per deduce call from the
 *  canonical CountVowel arm. */
function applyVowelConsonantCrossElim(
  fp: FlatPuzzle,
  state: State,
  push: Push,
  vq: number,
  cq: number,
  n: number,
): void {
  const { eliminated } = state;

  // NONE counts as "valid" (still a candidate option) but can't partner —
  // leaving it in `valid` without a partner is what triggers the cross-elim.
  // Phantom mask in `eliminated` keeps oi >= optionCount out.
  const validMask = (q: number): number => {
    let mask = 0;
    for (let oi = 0; oi < 5; oi++) {
      if (!isElim(eliminated, q, oi)) mask |= 1 << oi;
    }
    return mask;
  };
  const vowelValid = validMask(vq);
  const consonantValid = validMask(cq);

  // 5×5 cross-product: find (voi, coi) pairs whose option values sum to n.
  let vowelHasPartner = 0;
  let consonantHasPartner = 0;
  let vIter = vowelValid;
  while (vIter !== 0) {
    const voi = lowestBit(vIter);
    vIter &= vIter - 1;
    const vv = fp.optionValues[vq][voi];
    if (vv == null) continue;
    let cIter = consonantValid;
    while (cIter !== 0) {
      const coi = lowestBit(cIter);
      cIter &= cIter - 1;
      const cv = fp.optionValues[cq][coi];
      if (cv == null) continue;
      if (vv + cv === n) {
        vowelHasPartner |= 1 << voi;
        consonantHasPartner |= 1 << coi;
      }
    }
  }

  const emitUnpaired = (q: number, valid: number, hasPartner: number, rule: DeduceRule) => {
    let unpaired = valid & ~hasPartner;
    while (unpaired !== 0) {
      const oi = lowestBit(unpaired);
      unpaired &= unpaired - 1;
      push(rule, { type: "eliminate", qi: q, oi });
    }
  };
  emitUnpaired(vq, vowelValid, vowelHasPartner, "VowelCrossElim");
  emitUnpaired(cq, consonantValid, consonantHasPartner, "ConsonantCrossElim");
}

function lowestBit(x: number): number {
  // x must be non-zero; returns the index of the lowest set bit.
  let i = 0;
  let v = x;
  while ((v & 1) === 0) {
    v >>= 1;
    i++;
  }
  return i;
}

// ── Main functions ──

/**
 * Sound-only deduction. Safe to use during generation: every conclusion is
 * true in any valid extension of the current state, regardless of whether the
 * puzzle has a unique solution.
 */
export function deduce(fp: FlatPuzzle, state: State): DeduceResult[] {
  return deduceImpl(fp, state, null, null, true, false);
}

/**
 * Deduction that may apply uniqueness-assuming rules (e.g. "TrueStmt has
 * exactly one true claim, so the known-true one must be it"). Only sound
 * when the puzzle is known to have a unique solution — use for play, check,
 * or tests; NOT during generation.
 */
export function deduceAssumingUnique(fp: FlatPuzzle, state: State): DeduceResult[] {
  return deduceImpl(fp, state, null, null, true, true);
}

/**
 * Fast-path variant of `deduce`: skips expensive non-fast rules. Sound-only
 * (does NOT apply uniqueness-assuming rules); used by lookahead's
 * hypothesis-testing where the hypothesis may be inconsistent.
 */
export function deduceFast(fp: FlatPuzzle, state: State): DeduceResult[] {
  return deduceImpl(fp, state, null, null, false, false);
}

export function deduceWithRule(
  fp: FlatPuzzle,
  state: State,
  rule: DeduceRule | null,
  exclude: DeduceRule | null = null,
): DeduceResult[] {
  return deduceImpl(fp, state, rule, exclude, true, true);
}

function deduceImpl(
  fp: FlatPuzzle,
  state: State,
  rule: DeduceRule | null,
  exclude: DeduceRule | null,
  full: boolean,
  assumeUnique: boolean,
): DeduceResult[] {
  const n = fp.n;
  const { answers, eliminated } = state;
  const results: DeduceResult[] = [];
  const push: Push = (r, action) => {
    if ((rule === null || rule === r) && exclude !== r) {
      results.push({ action, rule: r });
    }
  };

  // Canonical CountVowel/CountConsonant pair (last unanswered of each type).
  // Used by the CountVowel arm below for vowel+consonant = n cross-elim, which
  // fires exactly once per deduce call regardless of how many of each type
  // exist in the puzzle.
  let vowelQi = -1;
  let consonantQi = -1;
  for (let qi = 0; qi < n; qi++) {
    const t = fp.questions[qi].t;
    if (t === QT_COUNT_VOWEL && answers[qi] == null) vowelQi = qi;
    else if (t === QT_COUNT_CONSONANT && answers[qi] == null) consonantQi = qi;
  }

  let cachedLetterCounts: [number[], number[]] | null = null;
  const getLetterCounts = (): [number[], number[]] => {
    if (cachedLetterCounts == null) {
      cachedLetterCounts = computeLetterCounts(answers, eliminated, n);
    }
    return cachedLetterCounts;
  };

  // ── Per-qi dispatch ──
  // For each qi, dispatch on its type. Each arm owns all rules whose source
  // is qi (regardless of qi's answered state). The type-agnostic
  // OnlyOptionLeft fires at the end of each iteration.
  for (let qi = 0; qi < n; qi++) {
    const q = fp.questions[qi];
    const ans = answers[qi];

    switch (q.t) {
      case QT_COUNT_ANSWER: {
        applyCount(fp, state, push, qi, 1 << L2I[q.answer!], 0, n, full);
        break;
      }
      case QT_COUNT_ANSWER_BEFORE: {
        applyCount(fp, state, push, qi, 1 << L2I[q.answer!], 0, q.beforeIndex, full);
        break;
      }
      case QT_COUNT_ANSWER_AFTER: {
        applyCount(fp, state, push, qi, 1 << L2I[q.answer!], q.afterIndex + 1, n, full);
        break;
      }
      case QT_COUNT_CONSONANT: {
        applyCount(fp, state, push, qi, CONSONANT_MASK, 0, n, full);
        break;
      }
      case QT_MOST_COMMON_COUNT: {
        if (ans != null) break;
        let maxKnown = 0;
        let maxPossible = 0;
        for (let li = 0; li < fp.optionCount; li++) {
          const cr = countMatchingMask(answers, eliminated, 1 << li, 0, n);
          if (crMin(cr) > maxKnown) maxKnown = crMin(cr);
          if (crMax(cr) > maxPossible) maxPossible = crMax(cr);
        }
        for (let oi = 0; oi < 5; oi++) {
          if (isElim(eliminated, qi, oi)) continue;
          const sv = fp.optionValues[qi][oi];
          if (sv == null) continue;
          if (sv < maxKnown || sv > maxPossible) {
            push("MostCommonCountElim", { type: "eliminate", qi, oi });
          }
        }
        break;
      }
      case QT_COUNT_VOWEL: {
        applyCount(fp, state, push, qi, VOWEL_MASK, 0, n, full);
        if (full && qi === vowelQi && consonantQi !== -1) {
          applyVowelConsonantCrossElim(fp, state, push, qi, consonantQi, n);
        }
        break;
      }
      case QT_ANSWER_OF: {
        const targetQi = q.questionIndex;
        if (ans != null) {
          // Reverse: qi answered → force the target qi.
          const implied = fp.optionValues[qi][letterIdx(ans)];
          if (implied != null && implied <= 4 && targetQi < n && answers[targetQi] == null) {
            push("AnswerOfReverse", { type: "force", qi: targetQi, answer: LETTERS[implied] });
          }
        } else {
          // Forward + per-option elim (qi unanswered).
          const targetAns = answers[targetQi];
          if (targetAns != null) {
            const tIdx = letterIdx(targetAns);
            let best = -1;
            for (let oi = 0; oi < 5; oi++) {
              if (fp.optionValues[qi][oi] === tIdx) {
                if (!isElim(eliminated, qi, oi)) {
                  best = oi;
                  break;
                }
                if (best === -1) best = oi;
              }
            }
            if (best !== -1) {
              push("AnswerOfForward", { type: "force", qi, answer: LETTERS[best] });
            }
          }
          for (let oi = 0; oi < 5; oi++) {
            if (isElim(eliminated, qi, oi)) continue;
            const ov = fp.optionValues[qi][oi];
            if (ov != null && ov <= 4) {
              if (targetAns != null) {
                if (letterIdx(targetAns) !== ov) {
                  push("AnswerOfTargetRuledOut", { type: "eliminate", qi, oi });
                }
              } else if (isElim(eliminated, targetQi, ov)) {
                push("AnswerOfTargetRuledOut", { type: "eliminate", qi, oi });
              }
            }
          }
        }
        break;
      }
      case QT_LETTER_DIST: {
        const targetQi = q.questionIndex;
        if (ans != null) {
          // Reverse (src answered): narrow target's options to those at the claimed distance.
          if (targetQi < n && targetQi !== qi && answers[targetQi] == null) {
            const aIdx = letterIdx(ans);
            const sv = fp.optionValues[qi][aIdx];
            // NONE distance is unsatisfiable: every non-eliminated option
            // ends up in elimMask (the `actual === sv` check is skipped when
            // the source's distance value is null).
            let elimMask = 0;
            let validCount = 0;
            let validOi = 0;
            for (let oi = 0; oi < 5; oi++) {
              if (isElim(eliminated, targetQi, oi)) continue;
              const actual = Math.abs(oi - aIdx);
              if (sv != null && actual === sv) {
                validCount++;
                validOi = oi;
              } else {
                elimMask |= 1 << oi;
              }
            }
            if (validCount === 1 && elimMask !== 0) {
              push("LetterDistReverseForce", {
                type: "force",
                qi: targetQi,
                answer: LETTERS[validOi],
              });
            }
            if (elimMask !== 0 && validCount !== 1) {
              push("LetterDistReverseElim", {
                type: "eliminateMulti",
                questionMask: 1 << targetQi,
                optionMask: elimMask,
              });
            }
          }
        } else {
          // Forward + per-option elim (qi unanswered).
          const targetAns = answers[targetQi];
          if (targetAns != null) {
            const otherIdx = letterIdx(targetAns);
            const oi = exactlyOne(0, 5, (o) => {
              const sv = fp.optionValues[qi][o];
              return !isElim(eliminated, qi, o) && sv != null && Math.abs(o - otherIdx) === sv;
            });
            if (oi !== -1) {
              push("LetterDistForward", { type: "force", qi, answer: LETTERS[oi] });
            }
          }
          for (let oi = 0; oi < 5; oi++) {
            if (isElim(eliminated, qi, oi)) continue;
            const sv = fp.optionValues[qi][oi];
            const maxDist = Math.max(oi, 4 - oi);
            if (sv != null && sv > maxDist) {
              push("LetterDistImpossible", { type: "eliminate", qi, oi });
            }
            if (targetAns != null) {
              const dist = Math.abs(oi - letterIdx(targetAns));
              const matches = sv != null && dist === sv;
              if (!matches) {
                push("LetterDistWrong", { type: "eliminate", qi, oi });
              }
            }
            if (sv != null && targetAns == null && sv <= maxDist) {
              const on = sv;
              let noMatch = true;
              for (let ti = 0; ti < 5; ti++) {
                if (!isElim(eliminated, targetQi, ti) && Math.abs(oi - ti) === on) {
                  noMatch = false;
                  break;
                }
              }
              if (noMatch) {
                push("LetterDistNoMatch", { type: "eliminate", qi, oi });
              }
            }
          }

          // Reverse (src unanswered): narrow target by what's compatible from src's remaining options.
          if (targetQi < n && targetQi !== qi && answers[targetQi] == null) {
            let elimMask = 0;
            for (let oi = 0; oi < 5; oi++) {
              if (isElim(eliminated, targetQi, oi)) continue;
              let compatible = false;
              for (let si = 0; si < 5; si++) {
                if (isElim(eliminated, qi, si)) continue;
                const sv = fp.optionValues[qi][si];
                if (sv != null && Math.abs(oi - si) === sv) {
                  compatible = true;
                  break;
                }
              }
              if (!compatible) elimMask |= 1 << oi;
            }
            if (elimMask !== 0) {
              push("LetterDistReverseElim", {
                type: "eliminateMulti",
                questionMask: 1 << targetQi,
                optionMask: elimMask,
              });
            }
          }
        }
        break;
      }
      case QT_FIRST_WITH: {
        applyPositionalForward(fp, state, push, qi, q.answer!, 0);
        break;
      }
      case QT_CLOSEST_AFTER: {
        applyPositionalForward(fp, state, push, qi, q.answer!, q.afterIndex + 1);
        break;
      }
      case QT_LAST_WITH: {
        applyPositionalBackward(fp, state, push, qi, q.answer!, n);
        break;
      }
      case QT_CLOSEST_BEFORE: {
        applyPositionalBackward(fp, state, push, qi, q.answer!, q.beforeIndex);
        break;
      }
      case QT_ONLY_ODD: {
        if (ans == null) applyOnlyOddEven(fp, state, push, qi, q.answer!, 1, full);
        break;
      }
      case QT_ONLY_EVEN: {
        if (ans == null) applyOnlyOddEven(fp, state, push, qi, q.answer!, 0, full);
        break;
      }
      case QT_CONSEC_IDENT: {
        // Reverse: any qi state. Eliminate matching neighbors at positions
        // that this ConsecIdent's remaining options can't claim.
        let possiblePairs = 0;
        for (let oi = 0; oi < 5; oi++) {
          if (isElim(eliminated, qi, oi)) continue;
          const sv = fp.optionValues[qi][oi];
          if (sv == null) continue;
          if (sv + 1 < n) possiblePairs |= 1 << sv;
        }
        const impossibleMask = ~possiblePairs & ((1 << Math.max(0, n - 1)) - 1);
        let impossible = impossibleMask;
        while (impossible !== 0) {
          const j = lowestBit(impossible);
          impossible &= impossible - 1;
          const aj = answers[j];
          const aj1 = answers[j + 1];
          if (aj != null && aj1 == null && !isElim(eliminated, j + 1, letterIdx(aj))) {
            push("ConsecIdentReverse", { type: "eliminate", qi: j + 1, oi: letterIdx(aj) });
          }
          if (aj1 != null && aj == null && !isElim(eliminated, j, letterIdx(aj1))) {
            push("ConsecIdentReverse", { type: "eliminate", qi: j, oi: letterIdx(aj1) });
          }
        }

        if (ans != null) {
          // Forward force/elim/both (qi answered, full mode only).
          if (full) {
            const sv = fp.optionValues[qi][letterIdx(ans)];
            if (sv != null && sv + 1 < n) {
              const p = sv;
              const possA = ~eliminated[p] & 0b11111;
              const possB = ~eliminated[p + 1] & 0b11111;
              const ansA = answers[p];
              const ansB = answers[p + 1];

              if (ansA != null && ansB == null && !isElim(eliminated, p + 1, letterIdx(ansA))) {
                push("ConsecIdentForwardForce", { type: "force", qi: p + 1, answer: ansA });
              }
              if (ansB != null && ansA == null && !isElim(eliminated, p, letterIdx(ansB))) {
                push("ConsecIdentForwardForce", { type: "force", qi: p, answer: ansB });
              }

              // Options at p that are remaining for p but impossible at p+1
              // (and vice versa) can't be in a consec-identical pair → eliminate.
              if (ansA == null) {
                let toElim = possA & ~possB & 0b11111;
                while (toElim !== 0) {
                  const oi = lowestBit(toElim);
                  toElim &= toElim - 1;
                  push("ConsecIdentForwardElim", { type: "eliminate", qi: p, oi });
                }
              }
              if (ansB == null) {
                let toElim = possB & ~possA & 0b11111;
                while (toElim !== 0) {
                  const oi = lowestBit(toElim);
                  toElim &= toElim - 1;
                  push("ConsecIdentForwardElim", { type: "eliminate", qi: p + 1, oi });
                }
              }

              if (ansA == null && ansB == null) {
                const common = possA & possB;
                if (common !== 0 && (common & (common - 1)) === 0) {
                  const oi = lowestBit(common);
                  push("ConsecIdentForwardBothForce", {
                    type: "force",
                    qi: p,
                    answer: LETTERS[oi],
                  });
                  push("ConsecIdentForwardBothForce", {
                    type: "force",
                    qi: p + 1,
                    answer: LETTERS[oi],
                  });
                }
              }
            }
          }
        } else {
          // Per-option elim (qi unanswered): OOR, NoCommon, SelfRef, NonePair.
          for (let oi = 0; oi < 5; oi++) {
            if (isElim(eliminated, qi, oi)) continue;
            const sv = fp.optionValues[qi][oi];
            if (sv != null) {
              const pos = sv;
              if (pos + 1 >= n) {
                push("ConsecIdentOutOfRange", { type: "eliminate", qi, oi });
              } else {
                const common = ~eliminated[pos] & 0b11111 & (~eliminated[pos + 1] & 0b11111);
                if (common === 0) {
                  push("ConsecIdentNoCommon", { type: "eliminate", qi, oi });
                } else if (pos === qi || pos + 1 === qi) {
                  const partner = pos === qi ? pos + 1 : pos;
                  if (isElim(eliminated, partner, oi)) {
                    push("ConsecIdentSelfRef", { type: "eliminate", qi, oi });
                  }
                }
              }
            } else {
              let hasPair = false;
              for (let i = 0; i < n - 1; i++) {
                const a0 = answers[i];
                const a1 = answers[i + 1];
                if (a0 != null && a1 != null && a0 === a1) {
                  hasPair = true;
                  break;
                }
              }
              if (hasPair) {
                push("ConsecIdentNonePair", { type: "eliminate", qi, oi });
              }
            }
          }
        }
        break;
      }
      case QT_EQUAL_COUNT: {
        if (ans != null) break;
        const answerIdx = L2I[q.answer!];
        for (let oi = 0; oi < 5; oi++) {
          if (isElim(eliminated, qi, oi)) continue;
          const sv = fp.optionValues[qi][oi];
          if (sv == null) continue;
          if (sv === answerIdx) {
            push("EqualCountSelfRef", { type: "eliminate", qi, oi });
          }
          if (sv !== answerIdx) {
            // Impossible iff max-possible for one letter is below known for the other.
            const [known, max] = getLetterCounts();
            if (max[answerIdx] < known[sv] || max[sv] < known[answerIdx]) {
              push("EqualCountRangeElim", { type: "eliminate", qi, oi });
            }
          }
        }
        break;
      }
      case QT_PREV_SAME: {
        applyPrevOrNextSame(
          fp,
          state,
          push,
          qi,
          0,
          qi,
          (x) => [x + 1, qi],
          "PrevSameNoneMatch",
          "PrevSameNotBefore",
          "PrevSameRuledOut",
          "PrevSameCloser",
        );
        break;
      }
      case QT_NEXT_SAME: {
        applyPrevOrNextSame(
          fp,
          state,
          push,
          qi,
          qi + 1,
          n,
          (x) => [qi + 1, x],
          "NextSameNoneMatch",
          "NextSameNotAfter",
          "NextSameRuledOut",
          "NextSameCloser",
        );
        break;
      }
      case QT_SAME_AS_WHICH: {
        const qiRef = q.questionIndex;
        const refAns = answers[qiRef];
        if (ans != null) {
          // Reverse (full only).
          if (full) {
            const sv = fp.optionValues[qi][letterIdx(ans)];
            if (sv != null) {
              const j = sv;
              if (j < n) {
                const jAns = answers[j];
                if (refAns != null && jAns == null && !isElim(eliminated, j, letterIdx(refAns))) {
                  push("SameAsWhichReverse", { type: "force", qi: j, answer: refAns });
                }
                if (jAns != null && refAns == null && !isElim(eliminated, qiRef, letterIdx(jAns))) {
                  push("SameAsWhichReverse", { type: "force", qi: qiRef, answer: jAns });
                }
              }
            }
          }
        } else if (refAns != null) {
          // Forward per-option elim (qi unanswered, target known).
          for (let oi = 0; oi < 5; oi++) {
            if (isElim(eliminated, qi, oi)) continue;
            const sv = fp.optionValues[qi][oi];
            if (sv == null) continue;
            const j = sv;
            if (j < n && j !== qi && j !== qiRef) {
              const ja = answers[j];
              const wrong = ja != null ? ja !== refAns : isElim(eliminated, j, letterIdx(refAns));
              if (wrong) {
                push("SameAsWhichForward", { type: "eliminate", qi, oi });
              }
            }
          }
        }
        break;
      }
      case QT_SAME_AS: {
        applySameShared(fp, state, push, qi, "SameAsReverse", full);

        // SameAs negative: non-selected option targets cannot share qi's
        // answer. Uniqueness-assuming, answered-qi only.
        if (assumeUnique && ans != null) {
          const ai = letterIdx(ans);
          const selectedSv = fp.optionValues[qi][ai];
          // The "none" answer's sound inference is handled in applySameShared
          // (OnlySameNoneForward); this rule is for the index case.
          if (selectedSv != null) {
            const selected = selectedSv;
            let qMask = 0;
            for (let oi = 0; oi < fp.optionCount; oi++) {
              if (oi === ai) continue;
              const tsv = fp.optionValues[qi][oi];
              if (tsv == null) continue;
              const target = tsv;
              if (target >= n || target === qi) continue;
              if (tsv !== selected && answers[target] == null && !isElim(eliminated, target, ai)) {
                qMask |= 1 << target;
              }
            }
            if (qMask !== 0) {
              push("SameAsNegative", {
                type: "eliminateMulti",
                questionMask: qMask,
                optionMask: 1 << ai,
              });
            }
          }
        }
        break;
      }
      case QT_ONLY_SAME: {
        applySameShared(fp, state, push, qi, "PrevNextOnlySameReverse", full);

        // OnlySameOtherMatch: per-option elim, OnlySame only. If pos is
        // pointing at a position where some OTHER qi (not pos) already
        // has letter `oi`, then qi can't be "only same as pos" via oi.
        if (ans == null) {
          for (let oi = 0; oi < 5; oi++) {
            if (isElim(eliminated, qi, oi)) continue;
            const sv = fp.optionValues[qi][oi];
            if (sv == null) continue;
            const pos = sv;
            if (pos >= n || pos === qi) continue;
            // qi is unanswered, so it doesn't contribute to letter_known.
            // Subtract pos's contribution to check for any OTHER match.
            const letter = LETTERS[oi];
            const [known] = getLetterCounts();
            const posContrib = answers[pos] === letter ? 1 : 0;
            if (known[oi] > posContrib) {
              push("OnlySameOtherMatch", { type: "eliminate", qi, oi });
            }
          }
        }
        break;
      }
      case QT_LEAST_COMMON: {
        if (full && ans == null) {
          const [known, max] = getLetterCounts();
          applyExtremumCount(
            fp,
            state,
            push,
            qi,
            true,
            known,
            max,
            "LeastCommonElim",
            "LeastCommonForce",
          );
        }
        break;
      }
      case QT_MOST_COMMON: {
        if (full && ans == null) {
          const [known, max] = getLetterCounts();
          applyExtremumCount(
            fp,
            state,
            push,
            qi,
            false,
            known,
            max,
            "MostCommonElim",
            "MostCommonForce",
          );
        }
        break;
      }
      case QT_TRUE_STMT: {
        if (!full) break;
        // SelfRef + ClaimInvalid: any qi state. Per-option scan.
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
            claim.value !== -1 &&
            claim.value <= 4 &&
            LETTERS[claim.value] !== LETTERS[oi]
          ) {
            contradicts = true;
          }
          if (contradicts) {
            push("TrueStatementSelfRef", { type: "eliminate", qi, oi });
          }

          const v = checkClaim(fp, state, { qi, oi }, claim);
          if (v === V_INVALID) {
            push("TrueStatementClaimInvalid", { type: "eliminate", qi, oi });
          }
        }

        if (ans != null) {
          // Forward (qi answered): a true-statement's claim, if compatible
          // with current state, forces the referenced target.
          const claim = claimAt(fp, qi, letterIdx(ans));
          if (claim) {
            const cqt = claim.questionType;
            if ((cqt.type === "FirstWith" || cqt.type === "LastWith") && claim.value !== -1) {
              const tqi = claim.value;
              if (tqi < n && answers[tqi] == null && !isElim(eliminated, tqi, L2I[cqt.answer])) {
                push("TrueStatementForward", {
                  type: "force",
                  qi: tqi,
                  answer: cqt.answer,
                });
              }
            } else if (cqt.type === "AnswerOf") {
              const tqi = cqt.questionIndex;
              if (claim.value !== -1 && claim.value <= 4 && tqi < n && answers[tqi] == null) {
                const letter = LETTERS[claim.value];
                if (!isElim(eliminated, tqi, claim.value)) {
                  push("TrueStatementForward", { type: "force", qi: tqi, answer: letter });
                }
              }
            }
          }
        } else {
          // ClaimValid + ClaimKnownTrue (qi unanswered).
          const validOi = exactlyOne(0, 5, (oi) => {
            if (isElim(eliminated, qi, oi)) return false;
            const claim = claimAt(fp, qi, oi);
            if (!claim) return false;
            const hyp: State = { answers: answers.slice(), eliminated: eliminated.slice() };
            hyp.answers[qi] = LETTERS[oi];
            hyp.eliminated[qi] = 0b11111 ^ (1 << oi);
            return checkClaim(fp, hyp, { qi, oi }, claim) !== V_INVALID;
          });
          if (validOi !== -1) {
            push("TrueStatementClaimValid", { type: "force", qi, answer: LETTERS[validOi] });
          }

          // Uniqueness-assuming: gated so it never fires during generation.
          if (assumeUnique) {
            const knownTrueOi = exactlyOne(0, 5, (oi) => {
              if (isElim(eliminated, qi, oi)) return false;
              const claim = claimAt(fp, qi, oi);
              if (!claim) return false;
              return checkClaim(fp, state, { qi, oi }, claim) === V_VALID;
            });
            if (knownTrueOi !== -1) {
              push("TrueStatementClaimKnownTrue", {
                type: "force",
                qi,
                answer: LETTERS[knownTrueOi],
              });
            }
          }
        }
        break;
      }
      default:
        break;
    }

    // OnlyOptionLeft is type-agnostic — fires when only one option remains.
    if (ans == null && remainingCount(eliminated[qi]) === 1) {
      const oi = lowestBit(~eliminated[qi] & 0b11111);
      push("OnlyOptionLeft", { type: "force", qi, answer: LETTERS[oi] });
    }
  }

  return results;
}
