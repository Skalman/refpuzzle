import { defineConfig, minifySync } from "vite";
import { preact } from "@preact/preset-vite";
import wasm from "vite-plugin-wasm";
import { minify as minifyHtml } from "html-minifier-terser";
import { brotliCompressSync, constants } from "node:zlib";
import { createHash } from "node:crypto";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { execSync } from "node:child_process";
import { join } from "node:path";
import type { Plugin } from "vite";

function versionPlugin(): Plugin {
  return {
    name: "version-txt",
    apply: "build",
    closeBundle() {
      const hash = execSync("git rev-parse --short HEAD").toString().trim();
      const ts = new Date().toISOString();
      writeFileSync(join(__dirname, "dist", "version.txt"), `${hash} ${ts}\n`);
    },
  };
}

// Inject a preload for the hashed wasm so the browser fetches it in parallel
// with the JS bundle instead of discovering it only after init() runs. The
// filename is content-hashed, so the tag is injected at build time from the
// emitted bundle. `as="fetch" crossorigin` matches the glue's
// instantiateStreaming fetch (a same-origin CORS fetch).
function wasmPreloadPlugin(): Plugin {
  return {
    name: "wasm-preload",
    apply: "build",
    transformIndexHtml: {
      order: "post",
      handler(html, ctx) {
        const wasmFile = ctx.bundle
          ? Object.keys(ctx.bundle).find((f) => f.endsWith(".wasm"))
          : undefined;
        if (!wasmFile) return html;
        return {
          html,
          tags: [
            {
              tag: "link",
              attrs: {
                rel: "preload",
                href: `/${wasmFile}`,
                as: "fetch",
                type: "application/wasm",
                crossorigin: "",
              },
              injectTo: "head",
            },
          ],
        };
      },
    },
  };
}

function brotliPlugin(): Plugin {
  return {
    name: "brotli-compress",
    apply: "build",
    closeBundle() {
      const dir = join(__dirname, "dist", "puzzles", "daily");
      let files: string[];
      try {
        files = readdirSync(dir).filter((f) => f.endsWith(".json"));
      } catch {
        return;
      }
      for (const file of files) {
        const path = join(dir, file);
        const buf = readFileSync(path);
        const compressed = brotliCompressSync(buf, {
          params: { [constants.BROTLI_PARAM_QUALITY]: 9 },
        });
        writeFileSync(path + ".br", compressed);
        const pct = ((1 - compressed.length / buf.length) * 100).toFixed(0);
        console.log(`  ${file} → ${file}.br (${pct}% smaller)`);
      }
    },
  };
}

/**
 * Injects the precache manifest (hashed /assets + shell files) into the built SW
 * and minifies it. SHELL_CACHE derives from the asset hashes, so each build
 * rotates the shell cache; the puzzle-data cache (sw.js) is left untouched.
 */
function swPrecachePlugin(): Plugin {
  return {
    name: "sw-precache",
    apply: "build",
    closeBundle() {
      const dist = join(__dirname, "dist");
      const swPath = join(dist, "sw.js");
      const assets = readdirSync(join(dist, "assets")).map((file) => `/assets/${file}`);
      const precache = ["/", "/logo.svg", "/manifest.json", ...assets].sort();
      const hash = createHash("sha256").update(precache.join(",")).digest("hex").slice(0, 8);

      const sw = readFileSync(swPath, "utf8")
        .replace(/const SHELL_CACHE = "[^"]*";/, `const SHELL_CACHE = "refpuzzle-${hash}";`)
        .replace(/const PRECACHE = \[[^\]]*\];/, `const PRECACHE = ${JSON.stringify(precache)};`);
      // Inject before minifying — the regex needs the readable source.
      const { code } = minifySync(swPath, sw);
      writeFileSync(swPath, code);
    },
  };
}

// Vite leaves index.html and its inline <script>/<style> unminified. Runs last,
// after every tag has been injected.
function htmlMinifyPlugin(): Plugin {
  return {
    name: "html-minify",
    apply: "build",
    transformIndexHtml: {
      order: "post",
      handler(html) {
        return minifyHtml(html, {
          collapseWhitespace: true,
          removeComments: true,
          minifyJS: true,
          minifyCSS: true,
        });
      },
    },
  };
}

export default defineConfig({
  plugins: [
    preact(),
    wasm(),
    versionPlugin(),
    wasmPreloadPlugin(),
    brotliPlugin(),
    swPrecachePlugin(),
    htmlMinifyPlugin(),
  ],
  build: {
    target: "es2018",
  },
});
