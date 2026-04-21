import { defineConfig } from "oxlint";

export default defineConfig({
  categories: {
    suspicious: "deny",
    perf: "deny",
    correctness: "deny",
  },
  plugins: ["eslint", "import", "oxc", "typescript", "react"],
  rules: {
    "react/react-in-jsx-scope": "off",
    "react-perf/jsx-no-new-function-as-prop": "off",
    "typescript/no-floating-promises": "off",
  },
});
