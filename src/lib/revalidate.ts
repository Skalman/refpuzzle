import {
  PUZZLE_VERSION,
  getStoredVersion,
  setStoredVersion,
  getCompletedPuzzleIds,
  loadState,
  markStale,
  unmarkStale,
} from "./store.ts";
import { deriveState, isValid } from "../engine/state.ts";
import { checkAnswer } from "../engine/check-answer.ts";
import { flattenPuzzle } from "../engine/types.ts";
import { fetchDaily } from "../puzzles/daily.ts";

const BATCH_SIZE = 20;

// Runs before render. Results are written to localStorage but don't trigger
// re-renders — the UI picks them up on the next page load.
export function revalidateIfNeeded(): void {
  if (getStoredVersion() >= PUZZLE_VERSION) return;

  const ids = getCompletedPuzzleIds();
  if (ids.length === 0) {
    setStoredVersion(PUZZLE_VERSION);
    return;
  }

  processAll(ids).catch(() => {});
}

async function processAll(ids: string[]): Promise<void> {
  for (let start = 0; start < ids.length; start += BATCH_SIZE) {
    // oxlint-disable-next-line no-await-in-loop
    await Promise.all(ids.slice(start, start + BATCH_SIZE).map(revalidateOne));
    // oxlint-disable-next-line no-await-in-loop
    await new Promise((r) => setTimeout(r, 0));
  }
  setStoredVersion(PUZZLE_VERSION);
}

async function revalidateOne(puzzleId: string): Promise<void> {
  const match = /^\/?(\d{4}-\d{2}-\d{2})\/(\d)$/.exec(puzzleId);
  if (!match) return;
  const [, dateStr, levelStr] = match;

  try {
    const dayData = await fetchDaily(dateStr);
    if (!dayData) return;
    const puzzle = dayData[levelStr];
    if (!puzzle) return;

    const n = puzzle.questions.length;
    const state = loadState(puzzleId, n);
    if (!state || !state.completed) return;

    const fp = flattenPuzzle(puzzle);
    const { answers, eliminated } = deriveState(
      state.questions.map((q) => q.marks),
      puzzle.optionCount,
    );

    let valid = true;
    for (let qi = 0; qi < n; qi++) {
      if (!isValid(checkAnswer(fp, { answers, eliminated }, qi))) {
        valid = false;
        break;
      }
    }

    if (valid && state.stale) unmarkStale(puzzleId);
    if (!valid && !state.stale) markStale(puzzleId);
  } catch {
    // fetch failed or puzzle not found — skip
  }
}
