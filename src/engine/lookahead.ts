import type { AnswerLetter, FlatPuzzle } from "./types.ts";
import { LETTERS, letterIdx } from "./types.ts";
import { deduce } from "./deduce.ts";
import type { DeduceAction, DeduceResult } from "./deduce.ts";
import { checkAnswerValidity } from "./check-validity.ts";

export interface LookaheadResult {
  eliminateQi: number;
  eliminateOi: number;
  assumptionQi: number;
  assumptionAnswer: AnswerLetter;
  chain: DeduceResult[];
  contradictionQi: number;
}

function hasContradiction(action: DeduceAction, answers: (AnswerLetter | null)[]): boolean {
  if (action.type === "force") {
    return answers[action.qi] != null && answers[action.qi] !== action.answer;
  }
  if (action.type === "eliminate") {
    return answers[action.qi] === LETTERS[action.oi];
  }
  if (action.type === "eliminateMulti") {
    for (let i = 0; i < answers.length; i++) {
      if ((action.questionMask >> i) & 1) {
        const a = answers[i];
        if (a != null && (action.optionMask >> letterIdx(a)) & 1) return true;
      }
    }
  }
  return false;
}

function tryAssumption(
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  qi: number,
  oi: number,
  stopDeducingAfterNResults: number,
): LookaheadResult | null {
  const n = fp.n;
  const hypAnswers: (AnswerLetter | null)[] = answers.slice(0, n);
  const hypEliminated: number[] = eliminated.slice(0, n);
  hypAnswers[qi] = LETTERS[oi];
  hypEliminated[qi] = 0b11111 ^ (1 << oi);

  const chain: DeduceResult[] = [];
  let contradiction = false;

  while (chain.length < stopDeducingAfterNResults) {
    const drs = deduce(fp, hypAnswers, hypEliminated);
    if (drs.length === 0) break;
    for (const dr of drs) {
      if (hasContradiction(dr.action, hypAnswers)) {
        contradiction = true;
        break;
      }
      applyAction(dr, hypAnswers, hypEliminated);
      chain.push(dr);
    }
    if (contradiction) break;
  }

  if (contradiction) {
    return {
      eliminateQi: qi,
      eliminateOi: oi,
      assumptionQi: qi,
      assumptionAnswer: LETTERS[oi],
      chain,
      contradictionQi: qi,
    };
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
  return null;
}

export function lookahead(
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  stopDeducingAfterNResults = Infinity,
): LookaheadResult | null {
  const n = fp.n;
  for (let qi = 0; qi < n; qi++) {
    if (answers[qi] != null) continue;
    for (let oi = 0; oi < 5; oi++) {
      if ((eliminated[qi] >> oi) & 1) continue;
      const result = tryAssumption(fp, answers, eliminated, qi, oi, stopDeducingAfterNResults);
      if (result) return result;
    }
  }
  return null;
}

export function lookaheadShortest(
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  stopDeducingAfterNResults = Infinity,
): LookaheadResult | null {
  const n = fp.n;
  let best: LookaheadResult | null = null;
  for (let qi = 0; qi < n; qi++) {
    if (answers[qi] != null) continue;
    for (let oi = 0; oi < 5; oi++) {
      if ((eliminated[qi] >> oi) & 1) continue;
      const result = tryAssumption(fp, answers, eliminated, qi, oi, stopDeducingAfterNResults);
      if (result && (best == null || result.chain.length < best.chain.length)) {
        best = result;
        if (best.chain.length === 0) return best;
      }
    }
  }
  return best;
}

function applyAction(
  dr: DeduceResult,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
): void {
  const a = dr.action;
  if (a.type === "force") {
    const oi = letterIdx(a.answer);
    eliminated[a.qi] = 0b11111 ^ (1 << oi);
    answers[a.qi] = a.answer;
  } else if (a.type === "eliminateMulti") {
    for (let i = 0; i < eliminated.length; i++) {
      if ((a.questionMask >> i) & 1) eliminated[i] |= a.optionMask;
    }
  } else {
    eliminated[a.qi] |= 1 << a.oi;
  }
}
