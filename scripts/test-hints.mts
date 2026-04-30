#!/usr/bin/env node --experimental-transform-types
/**
 * Runs shared hint-engine test cases from tests/hint-checks.json.
 * Each test specifies a puzzle, a state, and an expected action.
 *
 * Usage: node --experimental-transform-types scripts/test-hints.mts
 */
import { readFileSync } from "fs";
import { parseCompactYear } from "../src/puzzles/daily.ts";
import type { Marks } from "../src/engine/types.ts";
import { LETTERS, L2I } from "../src/engine/types.ts";
import { findActionFast } from "../src/engine/hints.ts";

interface TestCase {
  name: string;
  description?: string;
  puzzle: { q: any[] };
  state: string[];
  expect: string | null;
}

const suite: { tests: TestCase[] } = JSON.parse(
  readFileSync("tests/hint-checks.json", "utf8"),
);

function applyState(
  n: number,
  state: string[],
): { answers: (string | null)[]; marks: Marks[] } {
  const answers: (string | null)[] = new Array(n).fill(null);
  const marks: Marks[] = Array.from({ length: n }, () =>
    ["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"] as Marks,
  );

  for (let qi = 0; qi < n; qi++) {
    const s = state[qi] || "";
    for (const ch of s) {
      if (ch >= "A" && ch <= "E") {
        const oi = L2I[ch];
        answers[qi] = ch;
        for (let j = 0; j < 5; j++) marks[qi][j] = j === oi ? "correct" : "incorrect";
      } else if (ch >= "a" && ch <= "e") {
        const oi = L2I[ch.toUpperCase()];
        marks[qi][oi] = "incorrect";
      }
    }
  }
  return { answers, marks };
}

function formatAction(action: any): string | null {
  if (!action) return null;
  if (action.type === "contradiction") return `!${action.questionIndex + 1}`;
  if (action.type === "force") return `${action.questionIndex + 1}${action.letter}`;
  if (action.type === "eliminate") {
    return `${action.questionIndex + 1}${"abcde"[action.optionIndex]}`;
  }
  return null;
}

// Parse puzzle using the same compact format parser
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
  const puzzle = parsePuzzle(test.puzzle);
  const n = puzzle.questions.length;
  const { answers, marks } = applyState(n, test.state);

  const action = findActionFast(puzzle, answers as any, marks, n);
  const got = formatAction(action);

  if (got === test.expect) {
    passed++;
  } else {
    failed++;
    console.log(`FAIL: ${test.name}`);
    console.log(`  expected: ${test.expect}`);
    console.log(`  got:      ${got}`);
    if (test.description) console.log(`  (${test.description})`);
  }
}

console.log(`\n${passed}/${passed + failed} passed`);
if (failed > 0) process.exit(1);
