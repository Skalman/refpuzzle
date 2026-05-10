#!/usr/bin/env node --experimental-transform-types
import { readFileSync } from "fs";
import { parseCompactYear } from "../src/puzzles/daily.ts";
import type { CompactPuzzle } from "../src/puzzles/daily.ts";
import type { AnswerLetter, FlatPuzzle, Puzzle } from "../src/engine/types.ts";
import { L2I } from "../src/engine/types.ts";
import { checkAnswerValidity } from "../src/engine/check-validity.ts";
import type { Validity } from "../src/engine/state.ts";
import { flattenPuzzle } from "../src/engine/types.ts";

interface TestCase {
  name: string;
  qi: number;
  puzzle: CompactPuzzle;
  state: string[];
  expect: Validity;
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

const suite: TestSuite = JSON.parse(
  readFileSync("tests/check-validity.json", "utf8"),
);

function parsePuzzle(compact: CompactPuzzle): Puzzle {
  const wrapped: Record<string, Record<string, typeof compact>> = {
    "0101": { "level-1": compact },
  };
  const parsed = parseCompactYear(wrapped);
  return parsed["0101"]["level-1"];
}

function isUpperAnswer(ch: string): ch is AnswerLetter {
  return ch >= "A" && ch <= "E";
}

function isLowerAnswer(ch: string): boolean {
  return ch >= "a" && ch <= "e";
}

function applyState(
  n: number,
  state: string[],
): { answers: (AnswerLetter | null)[]; eliminated: number[] } {
  const answers: (AnswerLetter | null)[] = new Array(n).fill(null);
  const eliminated: number[] = new Array(n).fill(0);

  for (let qi = 0; qi < n; qi++) {
    const s = state[qi] || "";
    for (const ch of s) {
      if (isUpperAnswer(ch)) {
        const oi = L2I[ch];
        answers[qi] = ch;
        eliminated[qi] = 0b11111 ^ (1 << oi);
      } else if (isLowerAnswer(ch)) {
        const oi = L2I[ch.toUpperCase()];
        eliminated[qi] |= 1 << oi;
      }
    }
  }
  return { answers, eliminated };
}

let passed = 0;
let failed = 0;

for (const test of suite.tests) {
  if (isSectionHeader(test)) continue;
  const t = test;
  const puzzle = parsePuzzle(t.puzzle);
  const fp: FlatPuzzle = flattenPuzzle(puzzle);
  const n = puzzle.questions.length;
  const { answers, eliminated } = applyState(n, t.state);

  const got = checkAnswerValidity(fp, answers, eliminated, t.qi);

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
