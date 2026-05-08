#!/usr/bin/env node --experimental-transform-types
import { readFileSync } from "fs";
import { parseCompactYear } from "../src/puzzles/daily.ts";
import type { CompactPuzzle } from "../src/puzzles/daily.ts";
import type { FlatPuzzle, Puzzle } from "../src/engine/types.ts";
import { flattenPuzzle } from "../src/engine/types.ts";
import { checkSolvable } from "../src/engine/solve.ts";
import type { SolveOutcome } from "../src/engine/solve.ts";

interface TestCase {
  name: string;
  puzzle: CompactPuzzle;
  expect: SolveOutcome;
}

interface SectionHeader {
  section: string;
}

interface TestSuite {
  tests: (TestCase | SectionHeader)[];
}

function isSectionHeader(
  entry: TestCase | SectionHeader,
): entry is SectionHeader {
  return "section" in entry;
}

const suite: TestSuite = JSON.parse(readFileSync("tests/solve.json", "utf8"));

function parsePuzzle(compact: CompactPuzzle): Puzzle {
  const wrapped: Record<string, Record<string, typeof compact>> = {
    "0101": { "level-1": compact },
  };
  const parsed = parseCompactYear(wrapped);
  return parsed["0101"]["level-1"];
}

let passed = 0;
let failed = 0;

for (const test of suite.tests) {
  if (isSectionHeader(test)) continue;
  const t = test;
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
