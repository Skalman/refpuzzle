import type { AnswerLetter, Marks } from "./types.ts";
import { LETTERS } from "./types.ts";

export type Validity = "neutral" | "valid" | "invalid" | "pending";

export function deriveState(markSets: Marks[]): {
  answers: (AnswerLetter | null)[];
  eliminated: number[];
} {
  const answers: (AnswerLetter | null)[] = [];
  const eliminated: number[] = [];
  for (let qi = 0; qi < markSets.length; qi++) {
    const marks = markSets[qi];
    let answer: AnswerLetter | null = null;
    let elim = 0;
    for (let oi = 0; oi < 5; oi++) {
      if (marks[oi] === "correct") answer = LETTERS[oi];
      else if (marks[oi] === "incorrect") elim |= 1 << oi;
    }
    answers.push(answer);
    eliminated.push(elim);
  }
  return { answers, eliminated };
}
