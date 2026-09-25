import { useEffect, useState } from "preact/hooks";
import { Button } from "./ui/Button.tsx";

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
      return !sawAny; // no recognizable frames → can't tell, default to showing
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
    <div
      class="fixed inset-0 z-9999 flex items-center justify-center bg-backdrop p-safe-4"
      role="alert"
    >
      <div class="w-full max-w-md rounded-xl border border-strong bg-surface px-6 py-5 shadow-dialog">
        <h2 class="mb-2.5 text-dialog font-bold">Something went wrong</h2>
        <p class="mt-1.5 mb-3 rounded-md bg-[color-mix(in_srgb,var(--bg-surface),var(--text)_6%)] px-3 py-2 text-body font-mono wrap-break-word">
          {message}
        </p>
        <p>
          This often clears up after a cache reset — usually needed once after the app updates its
          data format.
        </p>
        <div class="mt-4 flex justify-end gap-2">
          {/* The border keeps it level with Dismiss's. */}
          <Button variant="primary" class="border border-accent" onClick={resetAndReload}>
            Reset cache &amp; reload
          </Button>
          <Button variant="outline-muted" onClick={() => setMessage(null)}>
            Dismiss
          </Button>
        </div>
      </div>
    </div>
  );
}
