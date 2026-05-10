import { useRef, useEffect, useCallback } from "preact/hooks";
import { loadMeta, saveMeta } from "../lib/store.ts";
import type { PuzzleMeta } from "../lib/store.ts";
import { track, getClientInfo } from "../lib/analytics.ts";

export interface MetaWithSession extends PuzzleMeta {
  sessionStart: number | null;
}

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
  const meta = useRef<MetaWithSession>({
    ...loadMeta(puzzleId),
    sessionStart: null,
  });

  function markStarted() {
    if (wasStarted.current) return;
    wasStarted.current = true;
    meta.current.sessions = 1;
    meta.current.sessionStart = Date.now();
    if (opts.initialHash) meta.current.fromShared = true;
    saveMeta(puzzleId, meta.current);
    track("puzzle_started", {
      puzzleId,
      level: opts.level,
      ...getClientInfo(),
    });
  }

  const flushElapsed = useCallback(
    function flushElapsed() {
      const m = meta.current;
      if (m.sessionStart != null) {
        m.elapsedS += Math.round((Date.now() - m.sessionStart) / 1000);
        m.sessionStart = null;
        saveMeta(puzzleId, m);
      }
    },
    [meta, puzzleId],
  );

  useEffect(() => {
    if (wasStarted.current) {
      meta.current.sessions++;
      meta.current.sessionStart = Date.now();
      saveMeta(puzzleId, meta.current);
    }

    function onVisibility() {
      if (document.hidden) {
        flushElapsed();
      } else if (wasStarted.current && !wasCompleted.current) {
        meta.current.sessions++;
        meta.current.sessionStart = Date.now();
        saveMeta(puzzleId, meta.current);
      }
    }

    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      flushElapsed();
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [puzzleId, flushElapsed]);

  return { meta, wasStarted, wasCompleted, markStarted };
}
