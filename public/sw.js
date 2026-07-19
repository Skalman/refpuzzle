// Rewritten at build time by swPrecachePlugin (vite.config.ts) to the hashed
// asset list. These dev fallbacks never run — the SW is prod-only (main.tsx).
const SHELL_CACHE = "refpuzzle-dev";

// Own cache, so code deploys don't wipe it. Bump -vN only if the compact puzzle
// format changes incompatibly (forces a fresh fetch).
const DATA_CACHE = "refpuzzle-data-v1";

const PRECACHE = ["/", "/logo.svg", "/manifest.json"];

self.addEventListener("install", (event) => {
  event.waitUntil(caches.open(SHELL_CACHE).then((cache) => cache.addAll(PRECACHE)));
  self.skipWaiting();
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(
          keys
            .filter((key) => key !== SHELL_CACHE && key !== DATA_CACHE)
            .map((key) => caches.delete(key)),
        ),
      ),
  );
  self.clients.claim();
});

self.addEventListener("fetch", (event) => {
  const url = new URL(event.request.url);

  // Only handle same-origin GET requests
  if (event.request.method !== "GET" || url.origin !== self.location.origin) return;

  // Every SPA route renders index.html, so serve the precached shell — deep
  // links and refreshes work offline. install keeps it current per version.
  if (event.request.mode === "navigate") {
    event.respondWith(
      caches
        .open(SHELL_CACHE)
        .then((cache) => cache.match("/").then((shell) => shell || fetch(event.request))),
    );
    return;
  }

  const cacheable =
    event.request.destination === "script" ||
    event.request.destination === "style" ||
    event.request.destination === "image" ||
    url.pathname.endsWith(".svg") ||
    url.pathname.endsWith(".json") ||
    // wasm (instantiateStreaming) has an empty `destination`; match extension.
    url.pathname.endsWith(".wasm");
  if (!cacheable) return;

  // Puzzle JSON → its own cache (survives deploys); everything else → shell.
  const cacheName = url.pathname.startsWith("/puzzles/") ? DATA_CACHE : SHELL_CACHE;

  // Hashed assets are immutable → cache-first. Others change in place → SWR.
  const immutable = url.pathname.startsWith("/assets/");

  event.respondWith(
    caches.open(cacheName).then((cache) =>
      cache.match(event.request).then((cached) => {
        if (cached && immutable) return cached;
        const fetched = fetch(event.request).then((response) => {
          if (response.ok) cache.put(event.request, response.clone());
          return response;
        });
        // Serve cache now, revalidate in background (swallow offline errors).
        if (cached) {
          fetched.catch(() => {});
          return cached;
        }
        return fetched;
      }),
    ),
  );
});
