import type { Answer, Marks } from "./types.ts";
import { LETTERS } from "./types.ts";

/** No answer selected yet. */
export const V_NEUTRAL = "neutral";
/** Provably true regardless of this question's answer. Safe to force in deduce. */
export const V_VALID = "valid";
/** True under the assumption that this option is selected, but may not hold for other options. */
export const V_CONSISTENT = "consistent";
/** Provably false given the current state. */
export const V_INVALID = "invalid";
/** Cannot determine validity yet — depends on unanswered questions. */
export const V_PENDING = "pending";
export type Validity =
  | typeof V_NEUTRAL
  | typeof V_VALID
  | typeof V_INVALID
  | typeof V_PENDING
  | typeof V_CONSISTENT;

export function deriveState(
  markSets: Marks[],
  optionCount = 5,
): {
  answers: (Answer | null)[];
  eliminated: number[];
} {
  const phantomMask = 0b11111 & ~((1 << optionCount) - 1);
  const answers: (Answer | null)[] = [];
  const eliminated: number[] = [];
  for (let qi = 0; qi < markSets.length; qi++) {
    const marks = markSets[qi];
    let answer: Answer | null = null;
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
