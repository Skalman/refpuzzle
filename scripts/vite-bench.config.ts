import { defineConfig } from "vite";
import { resolve } from "node:path";

export default defineConfig({
  build: {
    target: "node22",
    outDir: "dist-scripts",
    ssr: true,
    lib: {
      entry: resolve(import.meta.dirname, "bench.ts"),
      formats: ["es"],
      fileName: "bench",
    },
    minify: true,
  },
});
