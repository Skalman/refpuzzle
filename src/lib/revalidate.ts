import {
  PUZZLE_VERSION,
  getVersions,
  setVersion,
  getCompletedPuzzleIds,
  loadState,
  markStale,
  unmarkStale,
} from "./store.ts";
import { isValid } from "../engine/state.ts";
import { fetchDaily } from "../puzzles/daily.ts";
import { wasmReady, createPuzzleHandle } from "./wasm.ts";

const BATCH_SIZE = 20;

type Listener = () => void;
const listeners = new Set<Listener>();

/**
 * Subscribe to sweep results; returns an unsubscribe. The sweep writes
 * localStorage directly, and nothing else observes it (the `storage` event
 * skips the document that wrote), so the list pages would otherwise keep
 * showing the outcome they read at mount.
 */
export function onRevalidated(fn: Listener): () => void {
  listeners.add(fn);
  return () => {
    listeners.delete(fn);
  };
}

// Runs before render; the sweep itself lands afterwards, batch by batch.
export function revalidateIfNeeded(): void {
  if (getVersions().puzzle >= PUZZLE_VERSION) return;

  const ids = getCompletedPuzzleIds();
  if (ids.length === 0) {
    setVersion("puzzle", PUZZLE_VERSION);
    return;
  }

  processAll(ids).catch(() => {});
}

async function processAll(ids: string[]): Promise<void> {
  await wasmReady();
  for (let start = 0; start < ids.length; start += BATCH_SIZE) {
    // oxlint-disable-next-line no-await-in-loop
    const rewritten = await Promise.all(ids.slice(start, start + BATCH_SIZE).map(revalidateOne));
    if (rewritten.some(Boolean)) for (const fn of listeners) fn();
    // oxlint-disable-next-line no-await-in-loop
    await new Promise((r) => setTimeout(r, 0));
  }
  setVersion("puzzle", PUZZLE_VERSION);
}

/** Whether this puzzle's stale flag changed. */
async function revalidateOne(puzzleId: string): Promise<boolean> {
  const match = /^\/?(\d{4}-\d{2}-\d{2})\/(\d)$/.exec(puzzleId);
  if (!match) return false;
  const [, dateStr, levelStr] = match;

  try {
    const dayData = await fetchDaily(dateStr);
    if (!dayData) return false;
    const puzzle = dayData[levelStr];
    if (!puzzle) return false;

    const n = puzzle.questions.length;
    const state = loadState(puzzleId, n);
    if (!state || !state.completed) return false;

    const handle = createPuzzleHandle(puzzle.compact, puzzleId);
    const validities = handle.checkAllAnswers(
      state.questions.map((q) => q.marks),
      puzzle.optionCount,
    );
    handle.free();

    const valid = validities.every(isValid);
    if (valid && state.stale) {
      unmarkStale(puzzleId);
      return true;
    }
    if (!valid && !state.stale) {
      markStale(puzzleId);
      return true;
    }
    return false;
  } catch {
    // fetch failed or puzzle not found — skip
    return false;
  }
}
