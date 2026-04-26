import { defineConfig } from "vite";
import { preact } from "@preact/preset-vite";
import { brotliCompressSync, constants } from "node:zlib";
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
      const ts = new Date().toISOString().slice(0, 19).replace("T", " ");
      writeFileSync(join(__dirname, "dist", "version.txt"), `${hash} ${ts}\n`);
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

export default defineConfig({
  plugins: [preact(), versionPlugin(), brotliPlugin()],
  build: {
    target: "es2018",
  },
});
