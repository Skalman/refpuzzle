#!/usr/bin/env node --experimental-transform-types
import { readFileSync } from "fs";
import { parseCompactYear } from "../src/puzzles/daily.ts";
import type { AnswerLetter, FlatPuzzle, Puzzle } from "../src/engine/types.ts";
import { L2I, flattenPuzzle } from "../src/engine/types.ts";
import {
  deduce,
  deduceWithRule,
  ALL_DEDUCE_RULES,
} from "../src/engine/deduce.ts";
import type { DeduceResult, DeduceRule } from "../src/engine/deduce.ts";
import { explainDeduce } from "../src/engine/explain.ts";
import type { ExplainStep } from "../src/engine/explain.ts";

interface CompactPuzzle {
  q: unknown[];
}

interface TestCase {
  name: string;
  rule?: string;
  puzzle: CompactPuzzle;
  state: string[];
  expect: string | null;
}

interface SectionHeader {
  section: string;
}

interface TestSuite {
  tests: (TestCase | SectionHeader)[];
}

function isSectionHeader(entry: TestCase | SectionHeader): entry is SectionHeader {
  return "section" in entry;
}

const suite: TestSuite = JSON.parse(
  readFileSync("tests/deduce.json", "utf8"),
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

function formatAction(result: DeduceResult | null): string | null {
  if (!result) return null;
  const a = result.action;
  switch (a.type) {
    case "force":
      return `${a.questionIndex + 1}${a.letter}`;
    case "eliminate":
      return `${a.questionIndex + 1}${"abcde"[a.optionIndex]}`;
    case "eliminateMulti":
      return `qm${a.questionMask.toString(2)}o${a.optionMask.toString(2).padStart(5, "0")}`;
    default:
      return null;
  }
}

const VALID_RULES = new Set<string>(ALL_DEDUCE_RULES);

function parseRule(rule: string | undefined): DeduceRule | "All" | null {
  if (rule == null) return null;
  if (rule === "All") return "All";
  if (VALID_RULES.has(rule)) return rule as DeduceRule;
  return null;
}

function isRealRule(rule: DeduceRule | "All" | null): rule is DeduceRule {
  return rule !== null && rule !== "All";
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

let passed = 0;
let failed = 0;
let explainFailed = 0;
let dryFailed = 0;
const coveredRules = new Set<DeduceRule>();

for (const test of suite.tests) {
  if (isSectionHeader(test)) continue;
  const t = test;
  const puzzle = parsePuzzle(t.puzzle);
  const fp: FlatPuzzle = flattenPuzzle(puzzle);
  const n = puzzle.questions.length;
  const { answers, eliminated } = applyState(n, t.state);

  const parsedRule = parseRule(t.rule);
  if (isRealRule(parsedRule)) {
    coveredRules.add(parsedRule);
  }

  const results = isRealRule(parsedRule)
    ? deduceWithRule(fp, answers, eliminated, parsedRule)
    : deduce(fp, answers, eliminated);
  const result = results[0] ?? null;

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
        answers,
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
      console.log(`EXPLAIN THROW: ${t.name}: ${String(e)}`);
    }
  }
  // DRY check: if test specifies a rule, running without that rule should not produce the same action
  if (parsedRule != null && result && got === expected) {
    const withoutResults = deduceWithRule(
      fp,
      answers,
      eliminated,
      null,
      isRealRule(parsedRule) ? parsedRule : null,
    );
    const withoutFirst = withoutResults[0] ?? null;
    const withoutGot = formatAction(withoutFirst);
    if (withoutGot === got) {
      dryFailed++;
      console.log(`DRY: ${t.name}`);
      console.log(
        `  excluding "${parsedRule}" still produces: ${got} (via rule: ${withoutFirst?.rule ?? "unknown"})`,
      );
    }
  }
}

const uncoveredRules = ALL_DEDUCE_RULES.filter((r) => !coveredRules.has(r));
if (uncoveredRules.length > 0) {
  console.log(`MISSING TEST COVERAGE: ${uncoveredRules.join(", ")}`);
}

console.log(`\n${passed}/${passed + failed} passed`);
if (explainFailed > 0) console.log(`${explainFailed} explain fallback(s)`);
if (dryFailed > 0) console.log(`${dryFailed} DRY violation(s)`);
if (uncoveredRules.length > 0)
  console.log(`${uncoveredRules.length} rule(s) without tests`);
if (failed > 0 || uncoveredRules.length > 0 || dryFailed > 0) process.exit(1);
