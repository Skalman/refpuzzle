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
  RT_LETTER_DIST,
} from "./types.ts";

export type DeduceRuleFilter =
  | "count_saturation"
  | "forced_values"
  | "positional_range"
  | "vowel_consonant_cross"
  | "eliminations"
  | null;

export type DeduceAction =
  | { type: "force"; questionIndex: number; letter: AnswerLetter }
  | { type: "eliminate"; questionIndex: number; optionIndex: number }
  | { type: "eliminateMulti"; questionMask: number; optionMask: number };

export interface DeduceResult {
  action: DeduceAction;
  rule: string;
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

function countMatching(
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  pred: Pred,
  matchMask: number,
  from: number,
  to: number,
): { count: number; remaining: number } {
  let count = 0;
  let remaining = 0;
  for (let i = from; i < to; i++) {
    const a = answers[i];
    if (a != null) {
      if (pred(a)) count++;
    } else if ((eliminated[i] & matchMask) !== matchMask) {
      remaining++;
    }
  }
  return { count, remaining };
}

function countPred(r: { t: number; answer: string | null }): { pred: Pred; mask: number } | null {
  switch (r.t) {
    case RT_COUNT_ANSWER:
    case RT_COUNT_ANSWER_BEFORE:
    case RT_COUNT_ANSWER_AFTER: {
      const answer = r.answer!;
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
  r: { t: number; afterIndex: number; beforeIndex: number },
  n: number,
): [number, number] {
  if (r.t === RT_COUNT_ANSWER_BEFORE) return [0, r.beforeIndex];
  if (r.t === RT_COUNT_ANSWER_AFTER) return [r.afterIndex + 1, n];
  return [0, n];
}

function canStillMatch(pred: Pred, eliminated: number): boolean {
  for (let oi = 0; oi < 5; oi++) {
    if (((eliminated >> oi) & 1) === 0 && pred(LETTERS[oi])) return true;
  }
  return false;
}

function res(action: DeduceAction, rule: string): DeduceResult {
  return { action, rule };
}

// ── Main functions ──

export function deduce(
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
): DeduceResult | null {
  return deduceWithRule(fp, answers, eliminated, null);
}

export function deduceWithRule(
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  rule: DeduceRuleFilter,
  exclude: DeduceRuleFilter = null,
): DeduceResult | null {
  const n = fp.n;
  const run = (r: DeduceRuleFilter) => (rule === null || rule === r) && exclude !== r;

  // ── Count saturation ──
  if (run("count_saturation")) {
    for (let qi = 0; qi < n; qi++) {
      if (answers[qi] == null) continue;
      const r = fp.questions[qi];
      const cp = countPred(r);
      if (!cp) continue;
      const ai = letterIdx(answers[qi]!);
      const v = fp.optionValues[qi][ai];
      if (v == null) continue;
      const [from, to] = countRange(r, n);
      const cr = countMatching(answers, eliminated, cp.pred, cp.mask, from, to);

      if (cr.count === v && cr.remaining > 0) {
        for (let j = from; j < to; j++) {
          if (answers[j] != null) continue;
          for (let oi = 0; oi < 5; oi++) {
            if (!isElim(eliminated, j, oi) && cp.pred(LETTERS[oi])) {
              return res(
                { type: "eliminate", questionIndex: j, optionIndex: oi },
                "count_saturation",
              );
            }
          }
        }
      }
      if (cr.count + cr.remaining === v && cr.remaining > 0) {
        if (cr.remaining === 1) {
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
              return res(
                { type: "force", questionIndex: j, letter: LETTERS[matchOi] },
                "count_saturation",
              );
            }
          }
        }
        for (let j = from; j < to; j++) {
          if (answers[j] != null || !canStillMatch(cp.pred, eliminated[j])) continue;
          for (let oi = 0; oi < 5; oi++) {
            if (!isElim(eliminated, j, oi) && !cp.pred(LETTERS[oi])) {
              return res(
                { type: "eliminate", questionIndex: j, optionIndex: oi },
                "count_saturation",
              );
            }
          }
        }
      }
    }
  }

  // ── Forced values ──
  if (run("forced_values")) {
    for (let qi = 0; qi < n; qi++) {
      if (answers[qi] != null) continue;
      const r = fp.questions[qi];

      if (remainingCount(eliminated[qi]) === 1) {
        for (let oi = 0; oi < 5; oi++) {
          if (!isElim(eliminated, qi, oi)) {
            return res({ type: "force", questionIndex: qi, letter: LETTERS[oi] }, "forced_values");
          }
        }
      }

      if (r.t === RT_ANSWER_OF && answers[r.questionIndex] != null) {
        const target = answers[r.questionIndex]!;
        const targetIdx = letterIdx(target);
        for (let oi = 0; oi < 5; oi++) {
          if (fp.optionValues[qi][oi] === targetIdx) {
            return res({ type: "force", questionIndex: qi, letter: LETTERS[oi] }, "forced_values");
          }
        }
      }

      for (let other = 0; other < n; other++) {
        const otherAns = answers[other];
        if (otherAns == null) continue;
        const otherR = fp.questions[other];
        if (otherR.t === RT_ANSWER_OF && otherR.questionIndex === qi) {
          const impliedIdx = fp.optionValues[other][letterIdx(otherAns)];
          if (impliedIdx != null && impliedIdx >= 0 && impliedIdx < 5) {
            return res(
              { type: "force", questionIndex: qi, letter: LETTERS[impliedIdx] },
              "forced_values",
            );
          }
        }
        if (otherR.t === RT_SAME_AS) {
          const targetQ = fp.optionValues[other][letterIdx(otherAns)];
          if (targetQ != null && targetQ >= 0 && targetQ === qi) {
            return res({ type: "force", questionIndex: qi, letter: otherAns }, "forced_values");
          }
        }
        if (
          otherR.t === RT_PREV_SAME ||
          otherR.t === RT_NEXT_SAME ||
          otherR.t === RT_ONLY_SAME
        ) {
          const targetQ = fp.optionValues[other][letterIdx(otherAns)];
          if (targetQ != null && targetQ >= 0 && targetQ === qi) {
            return res({ type: "force", questionIndex: qi, letter: otherAns }, "forced_values");
          }
        }
      }

      if (r.t === RT_LETTER_DIST) {
        const otherAns = answers[r.questionIndex];
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
            return res({ type: "force", questionIndex: qi, letter: validLetter }, "forced_values");
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
          if (validCount === 1 && elimMask !== 0) {
            return res(
              { type: "force", questionIndex: qi, letter: LETTERS[validOi] },
              "forced_values",
            );
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
        }
        if (elimMask !== 0) {
          return res(
            { type: "eliminateMulti", questionMask: 1 << qi, optionMask: elimMask },
            "forced_values",
          );
        }
      }

      const cp = countPred(r);
      if (cp) {
        const [from, to] = countRange(r, n);
        const cr = countMatching(answers, eliminated, cp.pred, cp.mask, from, to);
        if (cr.remaining === 0) {
          for (let oi = 0; oi < 5; oi++) {
            if (isElim(eliminated, qi, oi)) continue;
            if (fp.optionValues[qi][oi] === cr.count) {
              return res(
                { type: "force", questionIndex: qi, letter: LETTERS[oi] },
                "forced_values",
              );
            }
          }
        }
      }
    }
  }

  // ── Positional range elimination ──
  if (run("positional_range")) {
    // Answered positional rules exclude answer from range
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
        return res(
          { type: "eliminateMulti", questionMask: qMask, optionMask: 1 << letterOi },
          "positional_range",
        );
      }
    }

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
          return res(
            { type: "eliminateMulti", questionMask: qMask, optionMask: 1 << letterOi },
            "positional_range",
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
          return res(
            { type: "eliminateMulti", questionMask: qMask, optionMask: 1 << letterOi },
            "positional_range",
          );
        }
      }
    }
  }

  // ── Vowel/consonant cross-elimination ──
  if (run("vowel_consonant_cross")) {
    let vowelQi = -1;
    let consonantQi = -1;
    for (let i = 0; i < n; i++) {
      if (answers[i] != null) continue;
      if (fp.questions[i].t === RT_COUNT_VOWEL) vowelQi = i;
      if (fp.questions[i].t === RT_COUNT_CONSONANT) consonantQi = i;
    }
    if (vowelQi >= 0 && consonantQi >= 0) {
      for (let oi = 0; oi < 5; oi++) {
        if (isElim(eliminated, vowelQi, oi)) continue;
        const vv = fp.optionValues[vowelQi][oi];
        if (vv == null) continue;
        const need = n - vv;
        let has = false;
        for (let coi = 0; coi < 5; coi++) {
          if (!isElim(eliminated, consonantQi, coi) && fp.optionValues[consonantQi][coi] === need) {
            has = true;
            break;
          }
        }
        if (!has)
          return res(
            { type: "eliminate", questionIndex: vowelQi, optionIndex: oi },
            "vowel_consonant_cross",
          );
      }
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
          return res(
            { type: "eliminate", questionIndex: consonantQi, optionIndex: oi },
            "vowel_consonant_cross",
          );
      }
    }
  }

  // ── Eliminations ──
  if (run("eliminations")) {
    for (let qi = 0; qi < n; qi++) {
      if (answers[qi] != null) continue;
      const r = fp.questions[qi];

      for (let oi = 0; oi < 5; oi++) {
        if (isElim(eliminated, qi, oi)) continue;
        const v = fp.optionValues[qi][oi];
        let elim = false;

        const cp = countPred(r);
        if (cp && r.t !== 5 /* RT_MOST_COMMON_COUNT */) {
          const [from, to] = countRange(r, n);
          const cr = countMatching(answers, eliminated, cp.pred, cp.mask, from, to);
          if (v != null && (cr.count > v || cr.count + cr.remaining < v)) {
            elim = true;
          }
        }

        if (!elim && r.t === RT_ANSWER_OF) {
          const target = answers[r.questionIndex];
          if (target != null && v != null && letterIdx(target) !== v) {
            elim = true;
          }
          if (!elim && target == null && v != null && v >= 0 && v < 5) {
            if (isElim(eliminated, r.questionIndex, v)) {
              elim = true;
            }
          }
        }

        if (!elim && r.t === RT_LETTER_DIST) {
          if (v != null && v > Math.max(oi, 4 - oi)) {
            elim = true;
          }
          const other = !elim ? answers[r.questionIndex] : null;
          if (!elim && other != null && v != null && Math.abs(oi - letterIdx(other)) !== v) {
            elim = true;
          }
          if (!elim && other == null && v != null) {
            let anyPossible = false;
            for (let ti = 0; ti < 5; ti++) {
              if (!isElim(eliminated, r.questionIndex, ti) && Math.abs(oi - ti) === v) {
                anyPossible = true;
                break;
              }
            }
            if (!anyPossible) elim = true;
          }
        }

        if (!elim && (r.t === RT_CLOSEST_AFTER || r.t === RT_FIRST_WITH)) {
          const scanStart = r.t === RT_CLOSEST_AFTER ? r.afterIndex + 1 : 0;
          if (v != null) {
            if (v < scanStart || v >= n) {
              elim = true;
            } else {
              if (answers[v] != null && answers[v] !== r.answer) {
                elim = true;
              } else if (answers[v] == null && isElim(eliminated, v, L2I[r.answer!])) {
                elim = true;
              }
              if (!elim) {
                for (let j = scanStart; j < v; j++) {
                  if (answers[j] === r.answer) {
                    elim = true;
                    break;
                  }
                }
              }
              if (!elim && LETTERS[oi] === r.answer && qi >= scanStart && qi < v) {
                elim = true;
              }
            }
          } else {
            for (let j = scanStart; j < n; j++) {
              if (answers[j] === r.answer) {
                elim = true;
                break;
              }
            }
          }
        }

        if (!elim && (r.t === RT_CLOSEST_BEFORE || r.t === RT_LAST_WITH)) {
          const beforeIdx = r.t === RT_CLOSEST_BEFORE ? r.beforeIndex : n;
          if (v != null) {
            if (v < 0 || v >= beforeIdx) {
              elim = true;
            } else {
              if (answers[v] != null && answers[v] !== r.answer) {
                elim = true;
              } else if (answers[v] == null && isElim(eliminated, v, L2I[r.answer!])) {
                elim = true;
              }
              if (!elim) {
                for (let j = beforeIdx - 1; j > v; j--) {
                  if (answers[j] === r.answer) {
                    elim = true;
                    break;
                  }
                }
              }
              if (!elim && LETTERS[oi] === r.answer && qi > v && qi < beforeIdx) {
                elim = true;
              }
            }
          } else {
            for (let j = 0; j < beforeIdx; j++) {
              if (answers[j] === r.answer) {
                elim = true;
                break;
              }
            }
          }
        }

        if (!elim && (r.t === RT_ONLY_ODD || r.t === RT_ONLY_EVEN)) {
          const parity = r.t === RT_ONLY_ODD ? 1 : 0;
          if (v != null) {
            if ((v + 1) % 2 !== parity) {
              elim = true;
            } else if (v >= 0 && v < n) {
              if (answers[v] != null && answers[v] !== r.answer) {
                elim = true;
              } else if (answers[v] == null && isElim(eliminated, v, L2I[r.answer!])) {
                elim = true;
              }
            }
          } else {
            for (let i = 0; i < n; i++) {
              if ((i + 1) % 2 === parity && answers[i] === r.answer) {
                elim = true;
                break;
              }
            }
          }
        }

        if (!elim && r.t === RT_CONSEC_IDENT) {
          if (v != null) {
            if (v < 0 || v + 1 >= n) {
              elim = true;
            } else {
              const possibleA = ~eliminated[v] & 0b11111;
              const possibleB = ~eliminated[v + 1] & 0b11111;
              if ((possibleA & possibleB) === 0) elim = true;
              if (!elim && (v === qi || v + 1 === qi)) {
                const partner = v === qi ? v + 1 : v;
                if (isElim(eliminated, partner, oi)) elim = true;
              }
            }
          } else {
            for (let i = 0; i < n - 1; i++) {
              if (answers[i] != null && answers[i + 1] != null && answers[i] === answers[i + 1]) {
                elim = true;
                break;
              }
            }
          }
        }

        if (!elim && r.t === RT_EQUAL_COUNT) {
          if (v != null && LETTERS[v] === r.answer) elim = true;
        }

        if (!elim && r.t === RT_PREV_SAME && v != null) {
          if (v >= qi) {
            elim = true;
          } else if (isElim(eliminated, v, oi)) {
            elim = true;
          } else {
            for (let j = qi - 1; j > v; j--) {
              if (answers[j] === LETTERS[oi]) {
                elim = true;
                break;
              }
            }
          }
        }

        if (!elim && r.t === RT_NEXT_SAME && v != null) {
          if (v <= qi || v >= n) {
            elim = true;
          } else if (isElim(eliminated, v, oi)) {
            elim = true;
          } else {
            for (let j = qi + 1; j < v; j++) {
              if (answers[j] === LETTERS[oi]) {
                elim = true;
                break;
              }
            }
          }
        }

        if (!elim && (r.t === RT_ONLY_SAME || r.t === RT_SAME_AS) && v != null) {
          if (v === qi) elim = true;
          else if (v >= 0 && v < n && isElim(eliminated, v, oi)) elim = true;
        }

        if (elim) {
          return res({ type: "eliminate", questionIndex: qi, optionIndex: oi }, "eliminations");
        }
      }
    }
  }

  return null;
}
