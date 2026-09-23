/** Input that shows someone at the page — pointer movement as well as presses. */
const ACTIVITY_EVENTS = ["pointerdown", "pointermove", "keydown", "wheel", "touchstart", "scroll"];

/** Calls `listener` on every activity event anywhere in the window; returns the unsubscribe. */
export function onActivity(listener: () => void): () => void {
  for (const eventName of ACTIVITY_EVENTS) {
    window.addEventListener(eventName, listener, { passive: true, capture: true });
  }
  return () => {
    for (const eventName of ACTIVITY_EVENTS) {
      window.removeEventListener(eventName, listener, { capture: true });
    }
  };
}
