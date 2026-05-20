import { render } from "preact";
import "./index.css"; // oxlint-disable-line import/no-unassigned-import
import { App } from "./App.tsx";
import { setupErrorTracking } from "./lib/analytics.ts";
import { migrateLocalStorage } from "./lib/store.ts";
import { revalidateIfNeeded } from "./lib/revalidate.ts";

migrateLocalStorage();
revalidateIfNeeded();
render(<App />, document.getElementById("app")!);

if (import.meta.env.PROD) {
  setupErrorTracking();
  if ("serviceWorker" in navigator) {
    navigator.serviceWorker.register("/sw.js");
  }
}
