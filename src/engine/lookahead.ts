import type { Answer, FlatPuzzle, State } from "./types.ts";
import { LETTERS, letterIdx } from "./types.ts";
import { deduce, deduceFast } from "./deduce.ts";
import type { DeduceAction, DeduceResult } from "./deduce.ts";
import { checkAnswer } from "./check-answer.ts";

export interface LookaheadResult {
  eliminateQi: number;
  eliminateOi: number;
  assumptionQi: number;
  assumptionAnswer: Answer;
  chain: DeduceResult[];
  contradictionQi: number;
}

function hasContradiction(action: DeduceAction, hyp: State): boolean {
  if (action.type === "force") {
    if (hyp.answers[action.qi] != null && hyp.answers[action.qi] !== action.answer) return true;
    return ((hyp.eliminated[action.qi] >> letterIdx(action.answer)) & 1) === 1;
  }
  if (action.type === "eliminate") {
    return hyp.answers[action.qi] === LETTERS[action.oi];
  }
  if (action.type === "eliminateMulti") {
    for (let i = 0; i < hyp.answers.length; i++) {
      if ((action.questionMask >> i) & 1) {
        const a = hyp.answers[i];
        if (a != null && (action.optionMask >> letterIdx(a)) & 1) return true;
      }
    }
  }
  return false;
}

function tryAssumption(
  fp: FlatPuzzle,
  state: State,
  hyp: State,
  qi: number,
  oi: number,
  stopDeducingAfterNResults: number,
  fast: boolean,
): LookaheadResult | null {
  const n = fp.n;
  for (let i = 0; i < n; i++) {
    hyp.answers[i] = state.answers[i];
    hyp.eliminated[i] = state.eliminated[i];
  }
  hyp.answers[qi] = LETTERS[oi];
  hyp.eliminated[qi] = 0b11111 ^ (1 << oi);

  const chain: DeduceResult[] = [];
  let contradiction = false;

  while (chain.length < stopDeducingAfterNResults) {
    const drs = fast ? deduceFast(fp, hyp) : deduce(fp, hyp);
    if (drs.length === 0) break;
    for (const dr of drs) {
      if (hasContradiction(dr.action, hyp)) {
        contradiction = true;
        break;
      }
      applyAction(dr, hyp);
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
    if (hyp.answers[checkQi] == null) {
      let rem = 0;
      for (let b = 0; b < 5; b++) if (((hyp.eliminated[checkQi] >> b) & 1) === 0) rem++;
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
    if (checkAnswer(fp, hyp, checkQi) === "invalid") {
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
  state: State,
  stopDeducingAfterNResults = Infinity,
  fast = false,
): LookaheadResult | null {
  const n = fp.n;
  const hyp: State = { answers: new Array(n), eliminated: new Array(n) };
  for (let qi = 0; qi < n; qi++) {
    if (state.answers[qi] != null) continue;
    for (let oi = 0; oi < 5; oi++) {
      if ((state.eliminated[qi] >> oi) & 1) continue;
      const result = tryAssumption(fp, state, hyp, qi, oi, stopDeducingAfterNResults, fast);
      if (result) return result;
    }
  }
  return null;
}

export function lookaheadShortest(
  fp: FlatPuzzle,
  state: State,
  stopDeducingAfterNResults = Infinity,
): LookaheadResult | null {
  const n = fp.n;
  const hyp: State = { answers: new Array(n), eliminated: new Array(n) };
  let best: LookaheadResult | null = null;
  for (let qi = 0; qi < n; qi++) {
    if (state.answers[qi] != null) continue;
    for (let oi = 0; oi < 5; oi++) {
      if ((state.eliminated[qi] >> oi) & 1) continue;
      const result = tryAssumption(fp, state, hyp, qi, oi, stopDeducingAfterNResults, false);
      if (result && (best == null || result.chain.length < best.chain.length)) {
        best = result;
        if (best.chain.length === 0) return best;
      }
    }
  }
  return best;
}

function applyAction(dr: DeduceResult, hyp: State): void {
  const a = dr.action;
  if (a.type === "force") {
    const oi = letterIdx(a.answer);
    hyp.eliminated[a.qi] = 0b11111 ^ (1 << oi);
    hyp.answers[a.qi] = a.answer;
  } else if (a.type === "eliminateMulti") {
    for (let i = 0; i < hyp.eliminated.length; i++) {
      if ((a.questionMask >> i) & 1) hyp.eliminated[i] |= a.optionMask;
    }
  } else {
    hyp.eliminated[a.qi] |= 1 << a.oi;
  }
}
