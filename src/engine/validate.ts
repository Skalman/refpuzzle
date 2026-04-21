import type { AnswerLetter, Puzzle, FlatPuzzle } from "./types.ts";
import { flattenPuzzle } from "./types.ts";
import { evaluate } from "./evaluators.ts";

export type Validity = "neutral" | "valid" | "invalid";

let _cache: { puzzle: Puzzle; fp: FlatPuzzle } | null = null;

function getFlatPuzzle(puzzle: Puzzle): FlatPuzzle {
  if (_cache && _cache.puzzle === puzzle) return _cache.fp;
  const fp = flattenPuzzle(puzzle);
  _cache = { puzzle, fp };
  return fp;
}

export function validate(puzzle: Puzzle, answers: (AnswerLetter | null)[]): Validity[] {
  const fp = getFlatPuzzle(puzzle);
  return fp.rules.map((r, i) => {
    const answer = answers[i];
    if (answer == null) return "neutral";
    const isValid = evaluate(r, i, answer, answers, fp);
    return isValid ? "valid" : "invalid";
  });
}
