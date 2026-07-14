import { render } from "preact";
import "./index.css"; // oxlint-disable-line import/no-unassigned-import
import { App } from "./App.tsx";
import { setupErrorTracking, trackFatalError } from "./lib/analytics.ts";
import { migrateLocalStorage } from "./lib/store.ts";
import { revalidateIfNeeded } from "./lib/revalidate.ts";
import { wasmReady } from "./lib/wasm.ts";

// Provided by the inline #fatal fallback in index.html (works without the bundle).
declare global {
  interface Window {
    showFatalError?: (detail?: unknown, opts?: { title?: string; body?: string }) => void;
    cancelBootTimeout?: () => void;
  }
}

// The bundle ran, so cancel the inline boot watchdog.
window.cancelBootTimeout?.();

migrateLocalStorage();
revalidateIfNeeded();

// The board renders question/option text through wasm (single source of truth
// with the hint engine), so the module must be initialized before the first
// paint. Kick the fetch immediately; the binary is small and the service worker
// caches it, so this only costs anything on the very first visit.
void wasmReady().then(
  () => render(<App />, document.getElementById("app")!),
  (e: unknown) => {
    console.error("wasm init failed", e);
    window.showFatalError?.(e);
    if (import.meta.env.PROD) trackFatalError(e, "wasm_init_failed");
  },
);

if (import.meta.env.PROD) {
  setupErrorTracking();
  if ("serviceWorker" in navigator) {
    navigator.serviceWorker.register("/sw.js");
  }
}
