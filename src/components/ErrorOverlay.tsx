import { useEffect, useState } from "preact/hooks";

// Catches uncaught errors and unhandled rejections so the user can recover
// from a poisoned service-worker cache (e.g. after a wire-format change) by
// nuking SW + caches + reloading instead of having to dig through DevTools.
async function resetAndReload(): Promise<void> {
  try {
    if ("serviceWorker" in navigator) {
      const regs = await navigator.serviceWorker.getRegistrations();
      await Promise.all(regs.map((r) => r.unregister()));
    }
    if ("caches" in window) {
      const keys = await caches.keys();
      await Promise.all(keys.map((k) => caches.delete(k)));
    }
  } finally {
    window.location.reload();
  }
}

export function ErrorOverlay() {
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    // Extension content scripts hit window.onerror constantly. Cross-origin
    // ones show up as the opaque "Script error." with no filename/stack
    // (CORS-blocked); extension-origin ones have chrome-extension:// /
    // moz-extension:// filenames. Filter both so the modal doesn't fire on
    // unrelated noise.
    const isOurError = (filename: string | undefined, msg: string): boolean => {
      if (msg === "Script error." || msg === "Script error") return false;
      if (!filename) return true; // no filename + non-opaque message: trust it
      if (/^(chrome|moz|webkit|safari-web|ms-browser)-extension:\/\//.test(filename)) return false;
      try {
        return new URL(filename).origin === window.location.origin;
      } catch {
        return true;
      }
    };

    const stackLooksLikeOurs = (stack: string | undefined): boolean => {
      if (!stack) return true;
      // Reject if every frame is from an extension; accept if any frame is
      // from our origin or has no scheme prefix (anonymous eval).
      const lines = stack.split("\n");
      let sawAny = false;
      for (const line of lines) {
        const m =
          /(?:https?|chrome-extension|moz-extension|safari-web-extension|webkit-extension|ms-browser-extension):\/\/[^\s)]+/.exec(
            line,
          );
        if (!m) continue;
        sawAny = true;
        if (m[0].startsWith(window.location.origin)) return true;
      }
      return !sawAny; // no recognisable frames → can't tell, default to showing
    };

    const onError = (e: ErrorEvent) => {
      const err = e.error;
      const msg = err instanceof Error ? err.message : e.message;
      const stack = err instanceof Error ? err.stack : undefined;
      if (!isOurError(e.filename, msg)) return;
      if (!stackLooksLikeOurs(stack)) return;
      setMessage(msg);
    };
    const onRejection = (e: PromiseRejectionEvent) => {
      const r: unknown = e.reason;
      const msg = r instanceof Error ? r.message : String(r);
      const stack = r instanceof Error ? r.stack : undefined;
      if (!stackLooksLikeOurs(stack)) return;
      setMessage(msg);
    };
    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onRejection);
    return () => {
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onRejection);
    };
  }, []);

  if (!message) return null;

  return (
    <div class="error-overlay" role="alert">
      <div class="error-overlay-card">
        <h2>Something went wrong</h2>
        <p class="error-overlay-message">{message}</p>
        <p>
          This often clears up after a cache reset — usually needed once after the app updates its
          data format.
        </p>
        <div class="error-overlay-actions">
          <button class="error-overlay-primary" onClick={resetAndReload}>
            Reset cache &amp; reload
          </button>
          <button onClick={() => setMessage(null)}>Dismiss</button>
        </div>
      </div>
    </div>
  );
}
