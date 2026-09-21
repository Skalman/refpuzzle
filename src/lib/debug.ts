/**
 * Development switches, kept in sessionStorage so they survive a reload but
 * never a new tab. `?debug` is the one URL entry point, and sets the flag for
 * the session; everything else is set from the footer's Debug panel.
 */

const DEBUG_KEY = "debug";
const NUDGE_KEY = "debug:nudge";

function read(key: string): string | null {
  try {
    return sessionStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string | null): void {
  try {
    if (value === null) sessionStorage.removeItem(key);
    else sessionStorage.setItem(key, value);
  } catch {}
}

/** Reads `?debug` off the URL once, so the flag outlives the query string. */
export function adoptDebugParam(): void {
  if (typeof window === "undefined") return;
  if (new URLSearchParams(window.location.search).has("debug")) write(DEBUG_KEY, "1");
}

/** Debug mode: the whole hint ladder at once, and any date opens. */
export function debugEnabled(): boolean {
  return read(DEBUG_KEY) === "1";
}

export function setDebugEnabled(on: boolean): void {
  write(DEBUG_KEY, on ? "1" : null);
}

/** Seconds of idle before a nudge; null leaves the shipped wait alone. */
export function nudgeSeconds(): number | null {
  const seconds = Number(read(NUDGE_KEY));
  return seconds > 0 ? seconds : null;
}

export function setNudgeSeconds(seconds: number | null): void {
  write(NUDGE_KEY, seconds === null ? null : String(seconds));
}
