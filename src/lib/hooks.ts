import { useState, useCallback, useEffect, useRef } from "preact/hooks";
import { onRevalidated } from "./revalidate.ts";

export function useForceUpdate(): () => void {
  const [, set] = useState(0);
  return useCallback(() => set((v) => v + 1), []);
}

/** Re-render when the boot re-check rewrites a stored outcome. */
export function useRevalidated(): void {
  const forceUpdate = useForceUpdate();
  useEffect(() => onRevalidated(forceUpdate), [forceUpdate]);
}

/**
 * The share/copy pair a sheet or dialog offers: whether the system share is
 * available, and a copy that confirms itself for a couple of seconds. `text`
 * is what lands on the clipboard; `share` hands the payload to the system.
 */
export function useShareable(payload: { text: string; title?: string; url?: string }) {
  const [copied, setCopied] = useState(false);
  const timer = useRef(0);
  useEffect(() => () => clearTimeout(timer.current), []);

  async function share() {
    try {
      await navigator.share(payload);
    } catch {
      /* canceled, or the sheet was dismissed */
    }
  }

  async function copy() {
    try {
      await navigator.clipboard.writeText(payload.text);
      setCopied(true);
      clearTimeout(timer.current);
      timer.current = window.setTimeout(() => setCopied(false), 2000);
    } catch {
      /* no clipboard permission */
    }
  }

  return {
    canShare: typeof navigator !== "undefined" && !!navigator.share,
    copied,
    share,
    copy,
  };
}

/**
 * Calls `onElapse` once `ms` have passed with the tab visible: hiding it
 * cancels the wait, and showing it again starts the stretch over. Inert while
 * `active` is falsy, and a change to `active` restarts the wait.
 */
export function useVisibleTimeout(active: unknown, ms: number, onElapse: () => void): void {
  const elapse = useRef(onElapse);
  elapse.current = onElapse;

  useEffect(() => {
    if (!active) return undefined;
    let timer = 0;
    function onVisibility() {
      clearTimeout(timer);
      if (!document.hidden) timer = window.setTimeout(() => elapse.current(), ms);
    }
    onVisibility();
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      clearTimeout(timer);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [active, ms]);
}
