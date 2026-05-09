import type { AnswerLetter, Marks } from "./types.ts";
import { LETTERS } from "./types.ts";

export type Validity = "neutral" | "valid" | "invalid" | "pending";

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
