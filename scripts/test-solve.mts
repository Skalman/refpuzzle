#!/usr/bin/env node --experimental-transform-types
import { readFileSync } from "fs";
import { parseCompactYear } from "../src/puzzles/daily.ts";
import type { FlatPuzzle } from "../src/engine/types.ts";
import { flattenPuzzle } from "../src/engine/types.ts";
import { checkSolvable } from "../src/engine/solve.ts";

interface TestCase {
  name: string;
  puzzle: { q: any[] };
  expect: string;
}

const suite: { tests: (TestCase | { section: string })[] } = JSON.parse(
  readFileSync("tests/solve.json", "utf8"),
);

function parsePuzzle(compact: { q: any[] }) {
  const wrapped: Record<string, Record<string, typeof compact>> = {
    "0101": { "level-1": compact },
  };
  const parsed = parseCompactYear(wrapped);
  return parsed["0101"]["level-1"];
}

let passed = 0;
let failed = 0;

for (const test of suite.tests) {
  if ("section" in test) continue;
  const t = test as TestCase;
  const puzzle = parsePuzzle(t.puzzle);
  const fp: FlatPuzzle = flattenPuzzle(puzzle);

  const got = checkSolvable(fp);

  if (got === t.expect) {
    passed++;
  } else {
    failed++;
    console.log(`FAIL: ${t.name}`);
    console.log(`  expected: ${t.expect}`);
    console.log(`  got:      ${got}`);
  }
}

console.log(`\n${passed}/${passed + failed} passed`);
if (failed > 0) process.exit(1);
