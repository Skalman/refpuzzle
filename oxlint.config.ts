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
    // "eslint/max-lines": ["error", { max: 1000 }],
  },
  overrides: [
    {
      files: ["scripts/**/*.ts", "scripts/**/*.mts"],
      rules: {
        "eslint/no-unused-vars": "off",
        "eslint/no-shadow": "off",
        "typescript/no-unsafe-type-assertion": "off",
        "typescript/no-unnecessary-type-assertion": "off",
        "oxc/no-map-spread": "off",
      },
    },
  ],
});
