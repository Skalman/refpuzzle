import { render } from "preact";
import "./index.css"; // oxlint-disable-line import/no-unassigned-import
import { App } from "./App.tsx";
import { setupErrorTracking } from "./lib/analytics.ts";
import { migrateLocalStorage } from "./lib/store.ts";
import { revalidateIfNeeded } from "./lib/revalidate.ts";
import { wasmReady } from "./lib/wasm.ts";

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
    const app = document.getElementById("app");
    if (app) app.textContent = "Failed to load. Please refresh the page.";
  },
);

if (import.meta.env.PROD) {
  setupErrorTracking();
  if ("serviceWorker" in navigator) {
    navigator.serviceWorker.register("/sw.js");
  }
}
