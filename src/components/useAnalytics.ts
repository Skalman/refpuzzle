import { useRef, useState, useEffect, useCallback } from "preact/hooks";
import { emptyMeta, loadMeta, saveMeta } from "../lib/store.ts";
import type { PuzzleMeta } from "../lib/store.ts";
import { onActivity } from "../lib/activity.ts";
import { track, getClientInfo } from "../lib/analytics.ts";

/**
 * A stretch without activity after which the clock stops at the last activity.
 * Generous, since staring at a hard question is solving too. A break longer
 * than this also ends the sitting.
 */
const IDLE_CUTOFF_MS = 3 * 60_000;

/**
 * The puzzle's counters and its solve clock. The clock runs from the first
 * mark while the tab is visible and the puzzle unsolved, and stops at the last
 * activity once a stretch passes the idle cutoff.
 *
 * Like the board, the counters are this tab's own: with two tabs on one
 * puzzle, the last to write wins.
 */
export function useAnalytics(
  puzzleId: string,
  opts: {
    level: number;
    initialHash?: string | null;
    initStarted: boolean;
    initCompleted: boolean;
  },
) {
  const wasStarted = useRef(opts.initStarted);
  const wasCompleted = useRef(opts.initCompleted);
  // `useRef`'s argument is evaluated on every render, so the load runs here.
  const [initialMeta] = useState<PuzzleMeta>(() => loadMeta(puzzleId));
  const meta = useRef<PuzzleMeta>(initialMeta);
  const clock = useRef({
    /** When the running stretch began; null while stopped. */
    runningSince: null as number | null,
    lastActivity: 0,
    /** When the clock last stopped; null before its first run this mount. */
    stoppedAt: null as number | null,
  });

  /** Applies `mutate` to this tab's counters and saves them. */
  const update = useCallback(
    function update(mutate: (m: PuzzleMeta) => void) {
      mutate(meta.current);
      saveMeta(puzzleId, meta.current);
    },
    [puzzleId],
  );

  /** The moment the clock should stop at: now, or the last activity if idle since. */
  function settledEnd(now: number): number {
    const c = clock.current;
    return now - c.lastActivity > IDLE_CUTOFF_MS ? c.lastActivity : now;
  }

  function startClock(now: number) {
    const c = clock.current;
    if (!wasStarted.current || wasCompleted.current || c.runningSince !== null) return;
    const newSitting = c.stoppedAt === null || now - c.stoppedAt > IDLE_CUTOFF_MS;
    c.runningSince = now;
    c.lastActivity = now;
    if (newSitting) update((m) => m.sessions++);
  }

  function stopClock(end: number) {
    const c = clock.current;
    if (c.runningSince === null) return;
    // Clamped: a wall-clock correction can move `end` before the start.
    const seconds = Math.max(0, Math.round((end - c.runningSince) / 1000));
    c.runningSince = null;
    c.stoppedAt = end;
    if (wasCompleted.current) return;
    update((m) => {
      m.elapsedS += seconds;
    });
  }

  // Handlers bound once at mount reach the current closures through this.
  const clockActions = useRef({ startClock, stopClock, settledEnd });
  clockActions.current = { startClock, stopClock, settledEnd };

  useEffect(() => {
    const actions = () => clockActions.current;
    function noteActivity() {
      const now = Date.now();
      const c = clock.current;
      if (c.runningSince !== null && now - c.lastActivity > IDLE_CUTOFF_MS) {
        actions().stopClock(c.lastActivity);
      }
      if (document.hidden) return;
      actions().startClock(now);
      c.lastActivity = now;
    }
    function onVisibility() {
      const now = Date.now();
      if (document.hidden) actions().stopClock(actions().settledEnd(now));
      else actions().startClock(now);
    }

    if (!document.hidden) actions().startClock(Date.now());
    const stopListening = onActivity(noteActivity);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      actions().stopClock(actions().settledEnd(Date.now()));
      stopListening();
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [puzzleId]);

  function markStarted() {
    if (wasStarted.current) return;
    wasStarted.current = true;
    if (opts.initialHash) meta.current.fromShared = true;
    startClock(Date.now());
    track("puzzle_started", {
      puzzleId,
      level: opts.level,
      ...getClientInfo(),
    });
  }

  /** Lands the in-memory counters once the puzzle has a stored entry to hold them. */
  const persist = useCallback(
    function persist() {
      if (wasStarted.current && !wasCompleted.current) update(() => {});
    },
    [update],
  );

  /** The solve just landed: the clock stops for good, its last stretch counted. */
  const finish = useCallback(function finish() {
    const actions = clockActions.current;
    actions.stopClock(actions.settledEnd(Date.now()));
    wasCompleted.current = true;
  }, []);

  /**
   * A second run at a solved puzzle, which is unstarted again: fresh counters,
   * and the clock waits for the first mark.
   */
  function restart() {
    wasStarted.current = false;
    wasCompleted.current = false;
    meta.current = emptyMeta();
    clock.current = { runningSince: null, lastActivity: 0, stoppedAt: null };
  }

  return { meta, wasStarted, wasCompleted, markStarted, update, persist, finish, restart };
}
