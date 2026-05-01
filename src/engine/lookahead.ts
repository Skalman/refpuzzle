import type { AnswerLetter, FlatPuzzle } from "./types.ts";
import { LETTERS, letterIdx } from "./types.ts";
import { deduce } from "./deduce.ts";
import type { DeduceResult } from "./deduce.ts";
import { checkAnswerValidity } from "./check-validity.ts";

export interface LookaheadResult {
  eliminateQi: number;
  eliminateOi: number;
  assumptionQi: number;
  assumptionAnswer: AnswerLetter;
  chain: DeduceResult[];
  contradictionQi: number;
}

export function lookahead(
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
): LookaheadResult | null {
  const n = fp.n;
  for (let qi = 0; qi < n; qi++) {
    if (answers[qi] != null) continue;
    for (let oi = 0; oi < 5; oi++) {
      if ((eliminated[qi] >> oi) & 1) continue;

      const hypAnswers: (AnswerLetter | null)[] = answers.slice(0, n);
      const hypEliminated: number[] = eliminated.slice(0, n);
      hypAnswers[qi] = LETTERS[oi];
      hypEliminated[qi] = 0b11111 ^ (1 << oi);

      const chain: DeduceResult[] = [];

      for (let iter = 0; iter < n * 5; iter++) {
        const dr = deduce(fp, hypAnswers, hypEliminated);
        if (!dr) break;
        applyAction(dr, hypAnswers, hypEliminated);
        chain.push(dr);
      }

      for (let checkQi = 0; checkQi < n; checkQi++) {
        if (hypAnswers[checkQi] == null) {
          let rem = 0;
          for (let b = 0; b < 5; b++) if (((hypEliminated[checkQi] >> b) & 1) === 0) rem++;
          if (rem === 0) {
            return {
              eliminateQi: qi,
              eliminateOi: oi,
              assumptionQi: qi,
              assumptionAnswer: LETTERS[oi],
              chain,
              contradictionQi: checkQi,
            };
          }
          continue;
        }
        if (checkAnswerValidity(fp, hypAnswers, hypEliminated, checkQi) === "invalid") {
          return {
            eliminateQi: qi,
            eliminateOi: oi,
            assumptionQi: qi,
            assumptionAnswer: LETTERS[oi],
            chain,
            contradictionQi: checkQi,
          };
        }
      }
    }
  }
  return null;
}

function applyAction(
  dr: DeduceResult,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
): void {
  const a = dr.action;
  if (a.type === "force") {
    const oi = letterIdx(a.letter);
    eliminated[a.questionIndex] = 0b11111 ^ (1 << oi);
    answers[a.questionIndex] = a.letter;
  } else {
    eliminated[a.questionIndex] |= 1 << a.optionIndex;
  }
}
