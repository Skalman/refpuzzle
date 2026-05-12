import type { Answer, FlatPuzzle } from "./types.ts";
import { letterIdx } from "./types.ts";
import { deduce } from "./deduce.ts";
import type { DeduceAction } from "./deduce.ts";
import { lookahead } from "./lookahead.ts";
import { checkAnswerValidity } from "./check-validity.ts";

export function solvePuzzle(fp: FlatPuzzle): (Answer | null)[] {
  const n = fp.n;
  const phantomMask = 0b11111 & ~((1 << fp.optionCount) - 1);
  const answers: (Answer | null)[] = new Array(n).fill(null);
  const eliminated: number[] = new Array(n).fill(phantomMask);

  for (let iter = 0; iter < n * 30; iter++) {
    if (answers.slice(0, n).every((a) => a != null)) break;

    const drs = deduce(fp, answers, eliminated);
    if (drs.length > 0) {
      for (const dr of drs) applyAction(dr.action, answers, eliminated);
      continue;
    }

    const lr = lookahead(fp, answers, eliminated);
    if (lr) {
      eliminated[lr.eliminateQi] |= 1 << lr.eliminateOi;
      continue;
    }

    break;
  }

  return answers;
}

export type SolveOutcome = "solved" | "stuck";

export function checkSolvable(fp: FlatPuzzle): SolveOutcome {
  const n = fp.n;
  const phantomMask = 0b11111 & ~((1 << fp.optionCount) - 1);
  const answers: (Answer | null)[] = new Array(n).fill(null);
  const eliminated: number[] = new Array(n).fill(phantomMask);

  for (let iter = 0; iter < n * 30; iter++) {
    if (answers.slice(0, n).every((a) => a != null)) return "solved";

    const drs = deduce(fp, answers, eliminated);
    if (drs.length > 0) {
      for (const dr of drs) applyAction(dr.action, answers, eliminated);
      continue;
    }

    const lr = lookahead(fp, answers, eliminated);
    if (lr) {
      eliminated[lr.eliminateQi] |= 1 << lr.eliminateOi;
      continue;
    }

    break;
  }

  return answers.slice(0, n).every((a) => a != null) ? "solved" : "stuck";
}

export function checkPuzzleSolved(
  fp: FlatPuzzle,
  answers: (Answer | null)[],
  eliminated: number[],
): boolean {
  const n = fp.n;
  for (let i = 0; i < n; i++) {
    if (answers[i] == null) return false;
    if (checkAnswerValidity(fp, answers, eliminated, i) !== "valid") return false;
  }
  return true;
}

function applyAction(action: DeduceAction, answers: (Answer | null)[], eliminated: number[]): void {
  if (action.type === "force") {
    const oi = letterIdx(action.answer);
    eliminated[action.qi] = 0b11111 ^ (1 << oi);
    answers[action.qi] = action.answer;
  } else if (action.type === "eliminateMulti") {
    for (let i = 0; i < eliminated.length; i++) {
      if ((action.questionMask >> i) & 1) eliminated[i] |= action.optionMask;
    }
  } else if (action.type === "eliminate") {
    eliminated[action.qi] |= 1 << action.oi;
  }
}
