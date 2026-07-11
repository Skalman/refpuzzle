import type { CompactPuzzle } from "../puzzles/daily.ts";

export type Answer = "A" | "B" | "C" | "D" | "E";
export type OptionMark = "unmarked" | "incorrect" | "correct";
export type Marks = [OptionMark, OptionMark, OptionMark, OptionMark, OptionMark];

export const LETTERS: readonly Answer[] = ["A", "B", "C", "D", "E"];
export const FRESH_MARKS: Marks = ["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"];
export const L2I: Record<string, number> = { A: 0, B: 1, C: 2, D: 3, E: 4 };

// Fast char-code based lookup: 'A'.charCodeAt(0)=65, so l2i[65]=0, etc.
const l2i = new Int8Array(70);
l2i[65] = 0;
l2i[66] = 1;
l2i[67] = 2;
l2i[68] = 3;
l2i[69] = 4;
export function letterIdx(s: string): number {
  return l2i[s.charCodeAt(0)];
}

export interface Puzzle {
  id: string;
  optionCount: number;
  questions: RenderedQuestion[];
  /** The compact on-disk blob, handed to wasm to build a solving handle. */
  compact: CompactPuzzle;
}

/** One question's board text, rendered by Rust via `PuzzleHandle.renderBoard`. */
export interface RenderedQuestion {
  text: string;
  options: string[];
}
