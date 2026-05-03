import type { AnswerLetter, FlatPuzzle } from "./types.ts";
import { letterIdx } from "./types.ts";
import { deduce } from "./deduce.ts";
import { lookahead } from "./lookahead.ts";
import { checkAnswerValidity } from "./check-validity.ts";

export function solvePuzzle(fp: FlatPuzzle): (AnswerLetter | null)[] {
  const n = fp.n;
  const answers: (AnswerLetter | null)[] = new Array(n).fill(null);
  const eliminated: number[] = new Array(n).fill(0);

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
  const answers: (AnswerLetter | null)[] = new Array(n).fill(null);
  const eliminated: number[] = new Array(n).fill(0);

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
  answers: (AnswerLetter | null)[],
  eliminated: number[],
): boolean {
  const n = fp.n;
  for (let i = 0; i < n; i++) {
    if (answers[i] == null) return false;
    if (checkAnswerValidity(fp, answers, eliminated, i) !== "valid") return false;
  }
  return true;
}

function applyAction(
  action: {
    type: string;
    questionIndex?: number;
    questionMask?: number;
    letter?: AnswerLetter;
    optionIndex?: number;
    optionMask?: number;
  },
  answers: (AnswerLetter | null)[],
  eliminated: number[],
): void {
  if (action.type === "force" && action.letter && action.questionIndex != null) {
    const oi = letterIdx(action.letter);
    eliminated[action.questionIndex] = 0b11111 ^ (1 << oi);
    answers[action.questionIndex] = action.letter;
  } else if (action.type === "eliminateMulti" && action.questionMask != null && action.optionMask != null) {
    for (let i = 0; i < eliminated.length; i++) {
      if ((action.questionMask >> i) & 1) eliminated[i] |= action.optionMask;
    }
  } else if (action.type === "eliminate" && action.questionIndex != null && action.optionIndex != null) {
    eliminated[action.questionIndex] |= 1 << action.optionIndex;
  }
}
