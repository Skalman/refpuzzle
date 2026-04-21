import type { AnswerLetter, Puzzle } from "./types.ts";
import { flattenPuzzle } from "./types.ts";
import { evaluate } from "./evaluators.ts";

export type Validity = "neutral" | "valid" | "invalid";

export function validate(
	puzzle: Puzzle,
	answers: (AnswerLetter | null)[],
): Validity[] {
	const fp = flattenPuzzle(puzzle);
	return fp.rules.map((r, i) => {
		const answer = answers[i];
		if (answer == null) return "neutral";
		const isValid = evaluate(r, i, answer, answers, fp);
		return isValid ? "valid" : "invalid";
	});
}
