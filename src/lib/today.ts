import { useEffect, useState } from "preact/hooks";
import { todayDateStr } from "../puzzles/daily.ts";

function msUntilNextMidnight(): number {
  const now = new Date();
  const next = new Date(now);
  next.setHours(24, 0, 0, 0);
  return next.getTime() - now.getTime();
}

/**
 * Returns today's date string (`YYYY-MM-DD`), re-rendering when the date
 * changes. Combines a midnight `setTimeout` (for foreground tabs) with a
 * `visibilitychange` listener (catches late/missed timers after the device
 * sleeps or the tab is throttled in the background).
 */
export function useToday(): string {
  const [today, setToday] = useState(todayDateStr);

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout>;

    function check() {
      const current = todayDateStr();
      setToday((prev) => (prev === current ? prev : current));
    }

    function schedule() {
      timer = setTimeout(() => {
        check();
        schedule();
      }, msUntilNextMidnight() + 100);
    }

    function onVisible() {
      if (document.visibilityState === "visible") check();
    }

    schedule();
    document.addEventListener("visibilitychange", onVisible);
    window.addEventListener("focus", check);

    return () => {
      clearTimeout(timer);
      document.removeEventListener("visibilitychange", onVisible);
      window.removeEventListener("focus", check);
    };
  }, []);

  return today;
}
