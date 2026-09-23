import { useEffect, useRef, useState } from "preact/hooks";
import { onActivity } from "../lib/activity.ts";
import { nudgeSeconds } from "../lib/debug.ts";

export type NudgeKind = "checkpoint" | "hint";

/** Idle stretch — activity on the page but no change to the board — before a nudge. */
const IDLE_MS = 90_000;
/** How long a nudge stays up unanswered. */
const SHOW_MS = 12_000;

/** The wait, or the shorter one the debug dialog asked for. */
function idleWait(): number {
  const seconds = import.meta.env.DEV ? nudgeSeconds() : null;
  return seconds === null ? IDLE_MS : seconds * 1000;
}

/**
 * A callout toward Checkpoint or Hint for a solver who is active but stuck.
 * The window measures time at the board: a board change (`progressKey`) or a
 * return to the page restarts it. Checkpoint goes first whenever it can be
 * pressed, and the window reopens after each nudge, so the other one follows
 * later. Each kind shows at most once per mount, and a kind the solver pressed
 * themselves is never offered.
 */
export function useIdleNudge({
  enabled,
  canCheckpoint,
  progressKey,
}: {
  enabled: boolean;
  canCheckpoint: boolean;
  progressKey: unknown;
}) {
  const [kind, setKind] = useState<NudgeKind | null>(null);
  const canCheckpointRef = useRef(canCheckpoint);
  canCheckpointRef.current = canCheckpoint;
  // Shown or pressed this mount.
  const spent = useRef(new Set<NudgeKind>());
  const hideTimer = useRef(0);
  // Reopens the idle window from outside the effect that owns it. A no-op
  // between effect runs, so a torn-down run can't plant a timer nothing clears.
  const reopen = useRef(() => {});

  useEffect(() => {
    setKind(null);
    clearTimeout(hideTimer.current);
    if (!enabled) return undefined;

    const wait = idleWait();
    let timer = 0;
    // Activity since the window opened. Expiring without any waits for the
    // next event and reopens from there.
    let active = false;
    let waiting = false;
    // When the open window started, for spotting a deadline the machine slept
    // through — a timer can come due hours late, and none of it was spent here.
    let armedAt = 0;

    /** At the board: the tab on screen and the window taking input. */
    function present(): boolean {
      return !document.hidden && document.hasFocus();
    }
    function pick(): NudgeKind | null {
      const order: NudgeKind[] = canCheckpointRef.current ? ["checkpoint", "hint"] : ["hint"];
      return order.find((k) => !spent.current.has(k)) ?? null;
    }
    function fire() {
      if (!active || !present() || Date.now() - armedAt > wait * 2) {
        waiting = true;
        return;
      }
      const next = pick();
      if (!next) return;
      spent.current.add(next);
      setKind(next);
      // Unanswered, it retires and the window reopens for whatever is left.
      hideTimer.current = window.setTimeout(() => {
        setKind(null);
        arm();
      }, SHOW_MS);
    }
    function arm() {
      clearTimeout(timer);
      active = false;
      waiting = false;
      armedAt = Date.now();
      timer = window.setTimeout(fire, wait);
    }
    function noteActivity() {
      // `arm` clears the flag, so the event that reopened the window counts in it.
      if (waiting) arm();
      active = true;
    }
    // Coming back starts a fresh stretch rather than resuming the one that ran
    // while the solver was elsewhere, so a nudge always follows 90s at the board.
    function onPresence() {
      if (present()) arm();
    }

    const stopListening = onActivity(noteActivity);
    document.addEventListener("visibilitychange", onPresence);
    window.addEventListener("focus", onPresence);
    window.addEventListener("blur", onPresence);
    reopen.current = arm;
    arm();
    return () => {
      reopen.current = () => {};
      clearTimeout(timer);
      clearTimeout(hideTimer.current);
      stopListening();
      document.removeEventListener("visibilitychange", onPresence);
      window.removeEventListener("focus", onPresence);
      window.removeEventListener("blur", onPresence);
    };
  }, [enabled, progressKey]);

  function dismiss() {
    clearTimeout(hideTimer.current);
    setKind(null);
    reopen.current();
  }

  // Up, it goes on the next press anywhere — it carries no control of its own.
  useEffect(() => {
    if (!kind) return undefined;
    const onPress = () => dismiss();
    document.addEventListener("pointerdown", onPress);
    document.addEventListener("keydown", onPress);
    return () => {
      document.removeEventListener("pointerdown", onPress);
      document.removeEventListener("keydown", onPress);
    };
  }, [kind]);

  /** The solver pressed this button themselves: retire the kind. */
  function used(pressed: NudgeKind) {
    spent.current.add(pressed);
    dismiss();
  }

  // `showMs` lets the callout time its own exit against the retirement above.
  return { kind, showMs: SHOW_MS, dismiss, used };
}
