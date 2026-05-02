#!/usr/bin/env node --experimental-transform-types
import { readFileSync } from "fs";
import { parseCompactYear } from "../src/puzzles/daily.ts";
import type { AnswerLetter, FlatPuzzle } from "../src/engine/types.ts";
import { LETTERS, L2I, flattenPuzzle } from "../src/engine/types.ts";
import { deduce, deduceWithRule, ALL_DEDUCE_RULES } from "../src/engine/deduce.ts";
import type { DeduceRule } from "../src/engine/deduce.ts";
import { explainDeduce } from "../src/engine/explain.ts";
import type { ExplainStep } from "../src/engine/explain.ts";

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
  if (a.type === "eliminate")
    return `${a.questionIndex + 1}${"abcde"[a.optionIndex]}`;
  if (a.type === "eliminateMulti")
    return `qm${a.questionMask.toString(2)}o${a.optionMask.toString(2).padStart(5, "0")}`;
  return null;
}

function hasGenericFallback(steps: ExplainStep[]): boolean {
  for (const step of steps) {
    if (step.type === "simple") {
      if (/^Q\d+ can't be [A-E]\.$/.test(step.text)) return true;
      if (/^Q\d+ options? [A-E, ]+ can be ruled out\.$/.test(step.text))
        return true;
    }
  }
  return false;
}

const testedRules = new Set(
  suite.tests
    .filter((t): t is TestCase => "rule" in t && typeof (t as TestCase).rule === "string")
    .map((t) => (t as TestCase).rule!),
);
const uncoveredRules = ALL_DEDUCE_RULES.filter((r) => !testedRules.has(r));
if (uncoveredRules.length > 0) {
  console.log(`MISSING TEST COVERAGE: ${uncoveredRules.join(", ")}`);
}

let passed = 0;
let failed = 0;
let explainFailed = 0;
let dryFailed = 0;

for (const test of suite.tests) {
  if ("section" in test) continue;
  const t = test as TestCase;
  const puzzle = parsePuzzle(t.puzzle);
  const fp: FlatPuzzle = flattenPuzzle(puzzle);
  const n = puzzle.questions.length;
  const { answers, eliminated } = applyState(n, t.state);

  const ruleFilter = (t.rule ?? null) as DeduceRule;
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

  if (result && got === expected) {
    try {
      const steps = explainDeduce(
        puzzle,
        fp,
        answers as (AnswerLetter | null)[],
        eliminated,
        result,
      );
      if (hasGenericFallback(steps)) {
        explainFailed++;
        console.log(`EXPLAIN FALLBACK: ${t.name}`);
        for (const s of steps) {
          console.log(`  ${s.type === "simple" ? s.text : JSON.stringify(s)}`);
        }
      }
    } catch (e) {
      explainFailed++;
      console.log(`EXPLAIN THROW: ${t.name}: ${e}`);
    }
  }
  // DRY check: if test specifies a rule, running without that rule should not produce the same action
  if (ruleFilter && result && got === expected) {
    const withoutResult = deduceWithRule(
      fp,
      answers as any,
      eliminated,
      null,
      ruleFilter,
    );
    const withoutGot = formatAction(withoutResult);
    if (withoutGot === got) {
      dryFailed++;
      console.log(`DRY: ${t.name}`);
      console.log(
        `  excluding "${ruleFilter}" still produces: ${got} (via rule: ${withoutResult!.rule})`,
      );
    }
  }
}

console.log(`\n${passed}/${passed + failed} passed`);
if (explainFailed > 0) console.log(`${explainFailed} explain fallback(s)`);
if (dryFailed > 0) console.log(`${dryFailed} DRY violation(s)`);
if (uncoveredRules.length > 0) console.log(`${uncoveredRules.length} rule(s) without tests`);
if (failed > 0 || uncoveredRules.length > 0) process.exit(1);
