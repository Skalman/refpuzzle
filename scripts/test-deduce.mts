#!/usr/bin/env node --experimental-transform-types
import { readFileSync } from "fs";
import { parseCompactYear } from "../src/puzzles/daily.ts";
import type { FlatPuzzle } from "../src/engine/types.ts";
import { LETTERS, L2I, flattenPuzzle } from "../src/engine/types.ts";
import { deduce, deduceWithRule } from "../src/engine/deduce.ts";
import type { DeduceRuleFilter } from "../src/engine/deduce.ts";

interface TestCase {
  name: string;
  rule?: string;
  puzzle: { q: any[] };
  state: string[];
  expect: string | null;
}

const suite: { tests: (TestCase | { section: string })[] } = JSON.parse(
  readFileSync("tests/deduce.json", "utf8"),
);

function parsePuzzle(compact: { q: any[] }) {
  const wrapped: Record<string, Record<string, typeof compact>> = {
    "0101": { "level-1": compact },
  };
  const parsed = parseCompactYear(wrapped);
  return parsed["0101"]["level-1"];
}

function applyState(
  n: number,
  state: string[],
): { answers: (string | null)[]; eliminated: number[] } {
  const answers: (string | null)[] = new Array(n).fill(null);
  const eliminated: number[] = new Array(n).fill(0);

  for (let qi = 0; qi < n; qi++) {
    const s = state[qi] || "";
    for (const ch of s) {
      if (ch >= "A" && ch <= "E") {
        const oi = L2I[ch];
        answers[qi] = ch;
        eliminated[qi] = 0b11111 ^ (1 << oi);
      } else if (ch >= "a" && ch <= "e") {
        const oi = L2I[ch.toUpperCase()];
        eliminated[qi] |= 1 << oi;
      }
    }
  }
  return { answers, eliminated };
}

function formatAction(result: any): string | null {
  if (!result) return null;
  const a = result.action;
  if (a.type === "force") return `${a.questionIndex + 1}${a.letter}`;
  if (a.type === "eliminate") return `${a.questionIndex + 1}${"abcde"[a.optionIndex]}`;
  return null;
}

let passed = 0;
let failed = 0;

for (const test of suite.tests) {
  if ("section" in test) continue;
  const t = test as TestCase;
  const puzzle = parsePuzzle(t.puzzle);
  const fp: FlatPuzzle = flattenPuzzle(puzzle);
  const n = puzzle.questions.length;
  const { answers, eliminated } = applyState(n, t.state);

  const ruleFilter = (t.rule ?? null) as DeduceRuleFilter;
  const result = ruleFilter
    ? deduceWithRule(fp, answers as any, eliminated, ruleFilter)
    : deduce(fp, answers as any, eliminated);

  const got = formatAction(result);
  const expected = t.expect;

  if (got === expected) {
    passed++;
  } else {
    failed++;
    console.log(`FAIL: ${t.name}`);
    console.log(`  expected: ${expected}`);
    console.log(`  got:      ${got}`);
  }
}

console.log(`\n${passed}/${passed + failed} passed`);
if (failed > 0) process.exit(1);
