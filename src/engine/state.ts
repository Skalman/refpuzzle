import type { AnswerLetter, Marks } from "./types.ts";
import { LETTERS } from "./types.ts";

export const V_NEUTRAL = "neutral";
export const V_VALID = "valid";
export const V_INVALID = "invalid";
export const V_PENDING = "pending";
export type Validity = typeof V_NEUTRAL | typeof V_VALID | typeof V_INVALID | typeof V_PENDING;

export function deriveState(
  markSets: Marks[],
  optionCount = 5,
): {
  answers: (AnswerLetter | null)[];
  eliminated: number[];
} {
  const phantomMask = 0b11111 & ~((1 << optionCount) - 1);
  const answers: (AnswerLetter | null)[] = [];
  const eliminated: number[] = [];
  for (let qi = 0; qi < markSets.length; qi++) {
    const marks = markSets[qi];
    let answer: AnswerLetter | null = null;
    let elim = phantomMask;
    for (let oi = 0; oi < optionCount; oi++) {
      if (marks[oi] === "correct") answer = LETTERS[oi];
      else if (marks[oi] === "incorrect") elim |= 1 << oi;
    }
    answers.push(answer);
    eliminated.push(elim);
  }
  return { answers, eliminated };
}
