import { hasState } from "../lib/store.ts";
import type { PuzzleProgress } from "../lib/store.ts";
import { LEVELS, puzzleId } from "./daily.ts";

/** Every level of one day, in tab order. */
export function dayStates(dateStr: string): PuzzleProgress[] {
  return LEVELS.map((level) => hasState(puzzleId(dateStr, level)));
}

/** A level that is done: solved, and the solution still checks out. */
export function isSolved(state: PuzzleProgress): boolean {
  return state.completed && !state.stale;
}

/** How far along one level is, as its tab and rail show it; null when untouched. */
export type LevelProgress = "stale" | "solved" | "started" | null;

/** A stale level reads as stale whatever else it is; a solved one outranks begun. */
export function levelProgress(state: PuzzleProgress): LevelProgress {
  if (state.stale) return "stale";
  if (state.completed) return "solved";
  if (state.started) return "started";
  return null;
}

/**
 * Where a day opens: the first level holding unfinished work — in progress, or
 * solved and since invalidated — then the first untouched one, and on a day
 * with neither, the first level.
 */
export function resumeLevel(states: PuzzleProgress[]): number {
  const firstLevel = (matches: (state: PuzzleProgress) => boolean) =>
    LEVELS.find((_, index) => matches(states[index]));
  return (
    firstLevel((state) => state.started && !isSolved(state)) ??
    firstLevel((state) => !state.started) ??
    LEVELS[0]
  );
}
