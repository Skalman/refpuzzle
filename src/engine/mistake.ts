import type { Answer } from "./types.ts";
import { letterIdx } from "./types.ts";

/**
 * A mark that contradicts the answer key. `answer` is the letter the note names:
 * the player's wrong answer, or the key option they ruled out.
 */
export type Mistake = { kind: "answer" | "elim"; qi: number; answer: Answer };

/**
 * The first mark that disagrees with the answer key, or null — a filled answer
 * that isn't the key's, or an elimination struck through the key option.
 * Key-based, so it flags a mistake before the red validity bar would. Questions
 * the solver can't pin down (null in `solution`) are skipped.
 */
export function findMistake(
  answers: (Answer | null)[],
  eliminated: number[],
  solution: (Answer | null)[],
): Mistake | null {
  for (let qi = 0; qi < solution.length; qi++) {
    const correct = solution[qi];
    if (correct == null) continue;
    const given = answers[qi];
    if (given != null && given !== correct) return { kind: "answer", qi, answer: given };
    if ((eliminated[qi] >> letterIdx(correct)) & 1) return { kind: "elim", qi, answer: correct };
  }
  return null;
}
