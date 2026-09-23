import type { FailMarker, HintMarker, PuzzleMeta, QuestionState } from "./store.ts";

/**
 * How one question went, for sharing: answered unaided (`clean`), named by a
 * hint (`hinted`), or caught wrong by a checkpoint (`caught`). Read off the
 * questions the markers recorded, so a marker that named none leaves every
 * question clean. A catch outranks a hint on the same question.
 */
export type QuestionOutcome = "clean" | "hinted" | "caught";

export function questionOutcomes(
  questionCount: number,
  hints: Map<number, HintMarker>,
  fails: Map<number, FailMarker>,
): QuestionOutcome[] {
  const outcomes: QuestionOutcome[] = new Array<QuestionOutcome>(questionCount).fill("clean");
  // A marker from a string written against another board can name a question
  // off this one; writing it would grow the row.
  const onBoard = (qi: number) => qi >= 0 && qi < questionCount;
  for (const hint of hints.values()) {
    if (hint.qi !== null && onBoard(hint.qi) && outcomes[hint.qi] === "clean") {
      outcomes[hint.qi] = "hinted";
    }
  }
  for (const fail of fails.values()) {
    for (const qi of fail.qis) {
      if (onBoard(qi)) outcomes[qi] = "caught";
    }
  }
  return outcomes;
}

/**
 * The tally for one solve. Time and rewinds are null for a solve not made on
 * this device — a shared board, an old backup — where the board can't tell.
 */
export interface SolveStats {
  elapsedS: number | null;
  historyBursts: number | null;
  hints: number;
  checkpoints: number;
  checkpointFails: number;
}

/** A checkpoint step pins the board without changing a cell. */
function isCheckpointStep(prev: QuestionState[], next: QuestionState[]): boolean {
  return prev.every((question, qi) =>
    question.marks.every((mark, oi) => mark === next[qi].marks[oi]),
  );
}

/**
 * A solve's counts read back off the stored track, for a summary reopened
 * later: checkpoints are the pin steps, refusals the fail markers' sum, hints
 * the steps carrying a hint marker (an undercount when one step took several).
 */
export function summarizeSolve(
  history: QuestionState[][],
  hints: Map<number, HintMarker>,
  fails: Map<number, FailMarker>,
): { hints: number; checkpoints: number; checkpointFails: number } {
  let checkpoints = 0;
  for (let i = 1; i < history.length; i++) {
    if (isCheckpointStep(history[i - 1], history[i])) checkpoints++;
  }
  let checkpointFails = 0;
  for (const fail of fails.values()) checkpointFails += fail.count;
  return { hints: hints.size, checkpoints, checkpointFails };
}

/**
 * Stats for a stored solve: the ledger's counters when the solve happened
 * here (it has sessions), else what the track can say, which excludes time
 * and rewinds. `meta` is null in the playground.
 */
export function storedSolveStats(
  meta: PuzzleMeta | null,
  history: QuestionState[][],
  hints: Map<number, HintMarker>,
  fails: Map<number, FailMarker>,
): SolveStats {
  if (meta && meta.sessions > 0) return meta;
  return { ...summarizeSolve(history, hints, fails), elapsedS: null, historyBursts: null };
}
