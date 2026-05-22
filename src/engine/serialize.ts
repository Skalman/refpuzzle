import type { FlatPuzzle } from "./types.ts";
import { flattenPuzzle } from "./types.ts";
import type { CompactPuzzle } from "../puzzles/daily.ts";
import { parseCompactPuzzle } from "../puzzles/daily.ts";

export function parsePuzzle(compact: CompactPuzzle): FlatPuzzle {
  return flattenPuzzle(parseCompactPuzzle(compact));
}
