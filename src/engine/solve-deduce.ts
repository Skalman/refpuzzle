import type { Answer, FlatPuzzle, State } from "./types.ts";
import { LETTERS, letterIdx } from "./types.ts";
import { deduce } from "./deduce.ts";
import type { DeduceAction } from "./deduce.ts";
import { lookahead } from "./lookahead.ts";
import { checkAnswer } from "./check-answer.ts";
import { isValid } from "./state.ts";

export interface SolveResult {
  answers: (Answer | null)[];
  eliminated: number[];
  steps: string[];
}

export function solvePuzzle(fp: FlatPuzzle): SolveResult {
  const n = fp.n;
  const phantomMask = 0b11111 & ~((1 << fp.optionCount) - 1);
  const state: State = {
    answers: new Array(n).fill(null),
    eliminated: new Array(n).fill(phantomMask),
  };
  const steps: string[] = [];

  for (let iter = 0; iter < n * 30; iter++) {
    if (state.answers.every((a) => a != null)) break;

    const drs = deduce(fp, state);
    if (drs.length > 0) {
      for (const dr of drs) {
        collectSteps(dr.action, n, steps);
        applyAction(dr.action, state);
      }
      continue;
    }

    const lr = lookahead(fp, state);
    if (lr) {
      state.eliminated[lr.eliminateQi] |= 1 << lr.eliminateOi;
      steps.push(`${lr.eliminateQi + 1}${LETTERS[lr.eliminateOi].toLowerCase()}`);
      continue;
    }

    break;
  }

  return { answers: state.answers, eliminated: state.eliminated, steps };
}

export type SolveOutcome = "solved" | "stuck";

export function checkSolvable(fp: FlatPuzzle): SolveOutcome {
  const { answers } = solvePuzzle(fp);
  return answers.slice(0, fp.n).every((a) => a != null) ? "solved" : "stuck";
}

export function checkPuzzleSolved(
  fp: FlatPuzzle,
  answers: (Answer | null)[],
  eliminated: number[],
): boolean {
  const state = { answers, eliminated };
  const n = fp.n;
  for (let i = 0; i < n; i++) {
    if (answers[i] == null) return false;
    if (!isValid(checkAnswer(fp, state, i))) return false;
  }
  return true;
}

function applyAction(action: DeduceAction, state: State): void {
  if (action.type === "force") {
    const oi = letterIdx(action.answer);
    state.eliminated[action.qi] = 0b11111 ^ (1 << oi);
    state.answers[action.qi] = action.answer;
  } else if (action.type === "eliminateMulti") {
    for (let i = 0; i < state.eliminated.length; i++) {
      if ((action.questionMask >> i) & 1) state.eliminated[i] |= action.optionMask;
    }
  } else if (action.type === "eliminate") {
    state.eliminated[action.qi] |= 1 << action.oi;
  }
}

function collectSteps(action: DeduceAction, n: number, steps: string[]): void {
  if (action.type === "force") {
    steps.push(`${action.qi + 1}${action.answer}`);
  } else if (action.type === "eliminate") {
    steps.push(`${action.qi + 1}${LETTERS[action.oi].toLowerCase()}`);
  } else if (action.type === "eliminateMulti") {
    for (let i = 0; i < n; i++) {
      if (!((action.questionMask >> i) & 1)) continue;
      for (let oi = 0; oi < 5; oi++) {
        if ((action.optionMask >> oi) & 1) steps.push(`${i + 1}${LETTERS[oi].toLowerCase()}`);
      }
    }
  }
}
