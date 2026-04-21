import type {
  AnswerLetter,
  Puzzle,
  QuestionDef,
  OptionDef,
  ValidationRule,
  Claim,
  StatementOption,
} from "../engine/types.ts";
import { flattenPuzzle } from "../engine/types.ts";
import { evaluate, evaluateClaim } from "../engine/evaluators.ts";
import { findHint, findActionFast } from "../engine/hints.ts";
import { solve } from "./solver.ts";
import { RNG } from "./rng.ts";
import type { DifficultyProfile } from "./difficulty.ts";

import type { Marks } from "../engine/types.ts";
import { LETTERS, L2I } from "../engine/types.ts";

const CONSTRAINED_TYPES = new Set<string>([
  "least_common_answer",
  "most_common_answer",
  "unique_answer",
  "equal_count_as",
  "answer_is_self",
]);

export interface GenerateResult {
  puzzle: Puzzle;
  solution: AnswerLetter[];
}

export function generate(profile: DifficultyProfile, rng: RNG): GenerateResult | null {
  for (let attempt = 0; attempt < 500; attempt++) {
    const result = tryGenerate(profile, rng);
    if (result) return result;
  }
  return null;
}

function tryGenerate(profile: DifficultyProfile, rng: RNG): GenerateResult | null {
  const n = profile.questionCount;

  const types = pickTypes(profile, n, rng);
  const rules = assignParams(types, n, rng);

  let solution: AnswerLetter[] = Array.from({ length: n }, () => rng.pick(LETTERS));

  // Shape solution to satisfy structural requirements BEFORE reconciliation
  const shaped = shapeSolution(rules, solution, n, rng);
  if (!shaped) return null;
  solution = shaped;

  const reconciled = reconcile(rules, solution);
  if (!reconciled) return null;
  solution = reconciled;

  // Re-check after reconciliation (constrained questions may have broken shaping)
  for (let i = 0; i < n; i++) {
    if (!checkStructural(rules[i], i, solution)) return null;
  }

  const textArr = rules.map((r) => questionText(r));
  if (new Set(textArr).size < n) return null;

  const questions: QuestionDef[] = rules.map((rule, i) => ({
    text: textArr[i],
    options: engineerOptions(rule, i, solution, n, rng),
    rule,
  }));

  const puzzle: Puzzle = {
    id: `level-${profile.level}`,
    title: profile.name,
    difficulty: profile.level,
    questions,
  };

  const fp = flattenPuzzle(puzzle);

  for (let i = 0; i < n; i++) {
    if (!evaluate(fp.rules[i], i, solution[i], solution, fp)) {
      return null;
    }
  }

  const solutions = solve(puzzle, undefined, 2);
  if (solutions.length !== 1) return null;

  if (!checkSolvable(puzzle, n)) return null;

  return { puzzle, solution: solutions[0] };
}

function checkSolvable(puzzle: Puzzle, n: number): boolean {
  const marks: Marks[] = Array.from(
    { length: n },
    () => ["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"] as Marks,
  );
  const answers: (AnswerLetter | null)[] = new Array(n).fill(null);

  for (let step = 0; step < n * 15; step++) {
    if (answers.every((a) => a != null)) return true;

    // Try fast deductions first (no messages, no look-ahead)
    const fast = findActionFast(puzzle, answers, marks, n);
    if (fast) {
      applyAction(fast, marks, answers);
      continue;
    }

    // Fall back to full hint engine (includes look-ahead)
    const hint = findHint(puzzle, marks);
    if (!hint?.action) return false;
    applyAction(hint.action, marks, answers);
  }
  return false;
}

function applyAction(
  action: { type: string; questionIndex: number; letter?: AnswerLetter; optionIndex?: number },
  marks: Marks[],
  answers: (AnswerLetter | null)[],
) {
  if (action.type === "force" && action.letter) {
    const qi = action.questionIndex;
    const oi = L2I[action.letter];
    for (let j = 0; j < 5; j++) marks[qi][j] = "incorrect";
    marks[qi][oi] = "correct";
    answers[qi] = action.letter;
  } else if (action.type === "eliminate" && action.optionIndex != null) {
    marks[action.questionIndex][action.optionIndex] = "incorrect";
  }
}

// Some rules require specific solution properties (e.g. exactly one consecutive pair)
function checkStructural(rule: ValidationRule, qi: number, sol: AnswerLetter[]): boolean {
  switch (rule.type) {
    case "only_same_answer": {
      let matches = 0;
      for (let i = 0; i < sol.length; i++) {
        if (i !== qi && sol[i] === sol[qi]) matches++;
      }
      return matches === 1;
    }
    case "consecutive_identical": {
      let pairs = 0;
      for (let i = 0; i < sol.length - 1; i++) {
        if (sol[i] === sol[i + 1]) pairs++;
      }
      return pairs === 1;
    }
    case "only_odd_with_answer": {
      let matches = 0;
      for (let i = 0; i < sol.length; i++) {
        if ((i + 1) % 2 === 1 && sol[i] === rule.answer) matches++;
      }
      return matches === 1;
    }
    case "unique_answer":
      return countLetter(sol, sol[qi]) === 1;
    case "equal_count_as": {
      const refCount = countLetter(sol, rule.answer);
      let matches = 0;
      for (const l of LETTERS) {
        if (l !== rule.answer && countLetter(sol, l) === refCount) matches++;
      }
      return matches >= 1;
    }
    default:
      return true;
  }
}

// Shape solution to satisfy structural requirements
function shapeSolution(
  rules: ValidationRule[],
  initial: AnswerLetter[],
  n: number,
  rng: RNG,
): AnswerLetter[] | null {
  const sol = [...initial];

  // Track which positions are "locked" by constrained types (don't modify them)
  const locked = new Set<number>();
  for (let i = 0; i < n; i++) {
    if (CONSTRAINED_TYPES.has(rules[i].type)) locked.add(i);
  }

  function pickFree(exclude: AnswerLetter): AnswerLetter {
    return rng.pick(LETTERS.filter((l) => l !== exclude));
  }

  for (let qi = 0; qi < n; qi++) {
    const rule = rules[qi];

    switch (rule.type) {
      case "unique_answer": {
        // sol[qi] must appear exactly once
        if (locked.has(qi)) break;
        // Find a letter used least (ideally 0 times elsewhere)
        const counts = letterCounts(sol);
        counts[L2I[sol[qi]]]--; // exclude self
        let best: AnswerLetter | null = null;
        let bestCount = n;
        for (const l of LETTERS) {
          const c = counts[L2I[l]];
          if (c < bestCount) {
            bestCount = c;
            best = l;
          }
        }
        if (!best) break;
        // Remove all other occurrences of this letter
        for (let i = 0; i < n; i++) {
          if (i !== qi && !locked.has(i) && sol[i] === best) {
            sol[i] = pickFree(best);
          }
        }
        sol[qi] = best;
        break;
      }

      case "only_same_answer": {
        // sol[qi] must appear exactly twice total
        if (locked.has(qi)) break;
        const myLetter = sol[qi];
        const positions = [];
        for (let i = 0; i < n; i++) {
          if (i !== qi && sol[i] === myLetter) positions.push(i);
        }
        if (positions.length === 1) break; // already good
        if (positions.length === 0) {
          // Need one more occurrence — pick a random unlocked position
          const candidates = [];
          for (let i = 0; i < n; i++) {
            if (i !== qi && !locked.has(i)) candidates.push(i);
          }
          if (candidates.length > 0) sol[rng.pick(candidates)] = myLetter;
        } else {
          // Too many — change extras
          const keep = rng.pick(positions);
          for (const p of positions) {
            if (p !== keep && !locked.has(p)) sol[p] = pickFree(myLetter);
          }
        }
        break;
      }

      case "consecutive_identical": {
        // Exactly one consecutive pair
        const pairs: number[] = [];
        for (let i = 0; i < n - 1; i++) {
          if (sol[i] === sol[i + 1]) pairs.push(i);
        }
        if (pairs.length === 1) break;
        if (pairs.length === 0) {
          // Create one pair
          const candidates = [];
          for (let i = 0; i < n - 1; i++) {
            if (!locked.has(i + 1)) candidates.push(i);
          }
          if (candidates.length > 0) {
            const pos = rng.pick(candidates);
            sol[pos + 1] = sol[pos];
          }
        }
        // Break extra pairs
        for (let pass = 0; pass < 5; pass++) {
          const extra: number[] = [];
          for (let i = 0; i < n - 1; i++) {
            if (sol[i] === sol[i + 1]) extra.push(i);
          }
          if (extra.length <= 1) break;
          const breakAt = extra[extra.length - 1];
          if (!locked.has(breakAt + 1)) {
            sol[breakAt + 1] = pickFree(sol[breakAt]);
          } else if (!locked.has(breakAt)) {
            sol[breakAt] = pickFree(sol[breakAt + 1]);
          }
        }
        break;
      }

      case "only_odd_with_answer": {
        // Exactly one odd-numbered question has answer rule.answer
        const oddPositions: number[] = [];
        for (let i = 0; i < n; i++) {
          if ((i + 1) % 2 === 1) oddPositions.push(i);
        }
        const withAnswer = oddPositions.filter((p) => sol[p] === rule.answer);
        if (withAnswer.length === 1) break;
        if (withAnswer.length === 0) {
          const candidates = oddPositions.filter((p) => !locked.has(p));
          if (candidates.length > 0) sol[rng.pick(candidates)] = rule.answer;
        } else {
          const keep = rng.pick(withAnswer);
          for (const p of withAnswer) {
            if (p !== keep && !locked.has(p)) sol[p] = pickFree(rule.answer);
          }
        }
        break;
      }

      case "equal_count_as": {
        // Some other letter must have same count as rule.answer
        const refCount = countLetter(sol, rule.answer);
        const hasMatch = LETTERS.some((l) => l !== rule.answer && countLetter(sol, l) === refCount);
        if (hasMatch) break;
        // Find the letter closest to refCount and adjust
        let closest: AnswerLetter | null = null;
        let closestDiff = n;
        for (const l of LETTERS) {
          if (l === rule.answer) continue;
          const diff = Math.abs(countLetter(sol, l) - refCount);
          if (diff < closestDiff) {
            closestDiff = diff;
            closest = l;
          }
        }
        if (!closest) break;
        const curCount = countLetter(sol, closest);
        if (curCount < refCount) {
          // Need more of `closest`
          for (let i = 0; i < n && countLetter(sol, closest) < refCount; i++) {
            if (!locked.has(i) && sol[i] !== closest && sol[i] !== rule.answer) {
              sol[i] = closest;
            }
          }
        } else {
          // Need fewer of `closest`
          for (let i = n - 1; i >= 0 && countLetter(sol, closest) > refCount; i--) {
            if (!locked.has(i) && sol[i] === closest) {
              sol[i] = pickFree(closest);
            }
          }
        }
        break;
      }
    }
  }

  return sol;
}

// ── Step 1: Pick question types ──

function pickTypes(profile: DifficultyProfile, n: number, rng: RNG): ValidationRule["type"][] {
  const allowed = profile.allowedTypes;
  const maxPerType = Math.ceil(n * 0.4);
  const types: ValidationRule["type"][] = [];
  const counts = new Map<string, number>();

  // Seed with at least 1 count_answer (entry point)
  if (allowed.includes("count_answer")) {
    types.push("count_answer");
    counts.set("count_answer", 1);
  }

  // Ensure enough answer_of_question for deduction chains (~25% of questions)
  if (allowed.includes("answer_of_question")) {
    const target = Math.max(1, Math.floor(n / 4));
    for (let i = 0; i < target; i++) {
      types.push("answer_of_question");
    }
    counts.set("answer_of_question", target);
  }

  if (allowed.includes("only_true_statement")) {
    types.push("only_true_statement");
    counts.set("only_true_statement", 1);
  }

  while (types.length < n) {
    const type = rng.pick(allowed);
    const cur = counts.get(type) ?? 0;
    if (cur < maxPerType) {
      types.push(type);
      counts.set(type, cur + 1);
    }
  }

  return rng.shuffle(types);
}

// ── Step 2: Assign parameters ──

function assignParams(types: ValidationRule["type"][], n: number, rng: RNG): ValidationRule[] {
  const aofqIndices = new Set<number>();
  for (let i = 0; i < n; i++) {
    if (types[i] === "answer_of_question") aofqIndices.add(i);
  }

  return types.map((type, i) => {
    switch (type) {
      case "count_answer":
        return { type, answer: rng.pick(LETTERS) };
      case "count_answer_before":
        return { type, answer: rng.pick(LETTERS), beforeIndex: rng.int(2, n - 1) };
      case "count_answer_after":
        return { type, answer: rng.pick(LETTERS), afterIndex: rng.int(0, Math.max(0, n - 3)) };
      case "count_vowel_answers":
      case "count_consonant_answers":
      case "most_common_count":
      case "least_common_answer":
      case "most_common_answer":
      case "unique_answer":
      case "answer_is_self":
      case "previous_same_answer":
      case "next_same_answer":
      case "only_same_answer":
      case "same_answer_as":
      case "consecutive_identical":
      case "only_true_statement":
        return { type };
      case "answer_of_question": {
        const pool = [];
        for (let j = 0; j < n; j++) {
          if (j !== i && !aofqIndices.has(j)) pool.push(j);
        }
        if (pool.length === 0) {
          for (let j = 0; j < n; j++) if (j !== i) pool.push(j);
        }
        return { type, questionIndex: rng.pick(pool) };
      }
      case "closest_after":
        return { type, afterIndex: rng.int(0, Math.max(0, n - 5)), answer: rng.pick(LETTERS) };
      case "closest_before":
        return { type, beforeIndex: rng.int(4, n - 1), answer: rng.pick(LETTERS) };
      case "first_with_answer":
      case "last_with_answer":
        return { type, answer: rng.pick(LETTERS) };
      case "only_odd_with_answer":
        return { type, answer: rng.pick(LETTERS) };
      case "equal_count_as":
        return { type, answer: rng.pick(LETTERS) };
      case "letter_distance": {
        const pool = [];
        for (let j = 0; j < n; j++) if (j !== i) pool.push(j);
        return { type, otherQuestionIndex: rng.pick(pool) };
      }
    }
    throw new Error(`Unknown rule type in assignParams: ${type as string}`);
  });
}

// ── Step 3: Reconcile constrained questions ──

function reconcile(rules: ValidationRule[], initial: AnswerLetter[]): AnswerLetter[] | null {
  const solution = [...initial];
  const n = solution.length;

  for (let iter = 0; iter < 30; iter++) {
    let changed = false;
    for (let i = 0; i < n; i++) {
      if (!CONSTRAINED_TYPES.has(rules[i].type)) continue;

      const needed = computeConstrainedAnswer(rules[i], i, solution);
      if (needed && needed !== solution[i]) {
        solution[i] = needed;
        changed = true;
      }
    }
    if (!changed) return solution;
  }
  return null;
}

function computeConstrainedAnswer(
  rule: ValidationRule,
  _qi: number,
  solution: AnswerLetter[],
): AnswerLetter | null {
  switch (rule.type) {
    case "most_common_answer": {
      const counts = letterCounts(solution);
      const max = Math.max(...counts);
      for (let i = 0; i < 5; i++) {
        if (counts[i] === max) return LETTERS[i];
      }
      return null;
    }
    case "least_common_answer": {
      const counts = letterCounts(solution);
      const min = Math.min(...counts);
      for (let i = 0; i < 5; i++) {
        if (counts[i] === min) return LETTERS[i];
      }
      return null;
    }
    case "unique_answer": {
      // Must be a letter that appears exactly once; keep current if it qualifies
      if (countLetter(solution, solution[_qi]) === 1) return solution[_qi];
      for (const l of LETTERS) {
        if (countLetter(solution, l) === 1) return l;
      }
      return null;
    }
    case "equal_count_as": {
      const refCount = countLetter(solution, rule.answer);
      for (const l of LETTERS) {
        if (l !== rule.answer && countLetter(solution, l) === refCount) return l;
      }
      return null;
    }
    case "answer_is_self":
      return solution[_qi]; // no constraint — keep whatever
    default:
      return null;
  }
}

function letterCounts(solution: AnswerLetter[]): number[] {
  const counts = [0, 0, 0, 0, 0];
  for (const a of solution) counts[L2I[a]]++;
  return counts;
}

function countLetter(sol: AnswerLetter[], l: AnswerLetter): number {
  let c = 0;
  for (const a of sol) if (a === l) c++;
  return c;
}

// ── Step 4 & 5: Engineer options ──

function engineerOptions(
  rule: ValidationRule,
  questionIdx: number,
  solution: AnswerLetter[],
  n: number,
  rng: RNG,
): OptionDef[] {
  if (CONSTRAINED_TYPES.has(rule.type)) {
    return LETTERS.map((l) => ({ label: l }));
  }

  if (rule.type === "only_true_statement") {
    return buildClaims(questionIdx, solution, n, rng);
  }

  const correctValue = computeValue(rule, questionIdx, solution);
  const targetIdx = L2I[solution[questionIdx]];
  const distractors = makeDistractors(rule, correctValue, n, rng);

  const options: OptionDef[] = new Array(5);
  options[targetIdx] = { label: correctValue };
  let di = 0;
  for (let i = 0; i < 5; i++) {
    if (i !== targetIdx) {
      options[i] = { label: distractors[di++] };
    }
  }
  return options;
}

function computeValue(rule: ValidationRule, qi: number, sol: AnswerLetter[]): string {
  switch (rule.type) {
    case "answer_of_question":
      return sol[rule.questionIndex];
    case "count_answer":
      return String(sol.filter((a) => a === rule.answer).length);
    case "count_answer_before":
      return String(sol.slice(0, rule.beforeIndex).filter((a) => a === rule.answer).length);
    case "count_answer_after":
      return String(sol.slice(rule.afterIndex + 1).filter((a) => a === rule.answer).length);
    case "count_vowel_answers":
      return String(sol.filter((a) => a === "A" || a === "E").length);
    case "count_consonant_answers":
      return String(sol.filter((a) => a !== "A" && a !== "E").length);
    case "most_common_count": {
      const counts = letterCounts(sol);
      return String(Math.max(...counts));
    }
    case "closest_after":
      for (let i = rule.afterIndex + 1; i < sol.length; i++) {
        if (sol[i] === rule.answer) return String(i + 1);
      }
      return "None";
    case "closest_before":
      for (let i = rule.beforeIndex - 1; i >= 0; i--) {
        if (sol[i] === rule.answer) return String(i + 1);
      }
      return "None";
    case "first_with_answer":
      for (let i = 0; i < sol.length; i++) {
        if (sol[i] === rule.answer) return String(i + 1);
      }
      return "None";
    case "last_with_answer":
      for (let i = sol.length - 1; i >= 0; i--) {
        if (sol[i] === rule.answer) return String(i + 1);
      }
      return "None";
    case "previous_same_answer":
      for (let i = qi - 1; i >= 0; i--) {
        if (sol[i] === sol[qi]) return String(i + 1);
      }
      return "None";
    case "next_same_answer":
      for (let i = qi + 1; i < sol.length; i++) {
        if (sol[i] === sol[qi]) return String(i + 1);
      }
      return "None";
    case "only_same_answer":
      for (let i = 0; i < sol.length; i++) {
        if (i !== qi && sol[i] === sol[qi]) return String(i + 1);
      }
      return "None";
    case "same_answer_as":
      for (let i = 0; i < sol.length; i++) {
        if (i !== qi && sol[i] === sol[qi]) return String(i + 1);
      }
      return "None";
    case "only_odd_with_answer":
      for (let i = 0; i < sol.length; i++) {
        if ((i + 1) % 2 === 1 && sol[i] === rule.answer) return String(i + 1);
      }
      return "None";
    case "consecutive_identical":
      for (let i = 0; i < sol.length - 1; i++) {
        if (sol[i] === sol[i + 1]) return `${i + 1} and ${i + 2}`;
      }
      return "None";
    case "letter_distance":
      return String(Math.abs(L2I[sol[qi]] - L2I[sol[rule.otherQuestionIndex]]));
  }
  throw new Error(`Unexpected rule type in computeValue: ${rule.type}`);
}

// ── Distractor generation ──

function makeDistractors(rule: ValidationRule, correct: string, n: number, rng: RNG): string[] {
  if (rule.type === "answer_of_question") {
    const pool = (LETTERS as readonly string[]).filter((v) => v !== correct);
    return rng.shuffle(pool);
  }

  if (rule.type === "letter_distance") {
    const pool = ["0", "1", "2", "3", "4"].filter((v) => v !== correct);
    return rng.shuffle(pool).slice(0, 4);
  }

  if (rule.type === "consecutive_identical") {
    return pairDistractors(correct, n, rng);
  }

  if (isCountingType(rule.type)) {
    return countDistractors(Number(correct), countMax(rule, n), rng);
  }

  return positionalDistractors(correct, n, rng, rule);
}

function isCountingType(type: string): boolean {
  return [
    "count_answer",
    "count_answer_before",
    "count_answer_after",
    "count_vowel_answers",
    "count_consonant_answers",
    "most_common_count",
  ].includes(type);
}

function countMax(rule: ValidationRule, n: number): number {
  if (rule.type === "count_answer_before") return rule.beforeIndex;
  if (rule.type === "count_answer_after") return n - rule.afterIndex - 1;
  return n;
}

function countDistractors(correct: number, max: number, rng: RNG): string[] {
  const upper = Math.max(max, 4);
  const pool: number[] = [];
  for (let i = 0; i <= upper; i++) {
    if (i !== correct) pool.push(i);
  }
  return rng.shuffle(pool).slice(0, 4).map(String);
}

function positionalDistractors(
  correct: string,
  n: number,
  rng: RNG,
  rule: ValidationRule,
): string[] {
  // Compute the valid range of question numbers for this rule
  let minPos = 1;
  let maxPos = n;
  if (rule.type === "closest_after") {
    minPos = rule.afterIndex + 2;
  }
  if (rule.type === "closest_before") {
    maxPos = rule.beforeIndex;
  }
  if (rule.type === "count_answer_after") {
    minPos = rule.afterIndex + 2;
  }
  if (rule.type === "count_answer_before") {
    maxPos = rule.beforeIndex;
  }

  const pool: string[] = [];
  for (let i = minPos; i <= maxPos; i++) {
    const s = String(i);
    if (s !== correct) pool.push(s);
  }
  if (correct !== "None") pool.push("None");
  return rng.shuffle(pool).slice(0, 4);
}

function pairDistractors(correct: string, n: number, rng: RNG): string[] {
  const pool: string[] = [];
  for (let i = 1; i < n; i++) {
    const pair = `${i} and ${i + 1}`;
    if (pair !== correct) pool.push(pair);
  }
  if (correct !== "None") pool.push("None");
  return rng.shuffle(pool).slice(0, 4);
}

// ── only_true_statement claims ──

function buildClaims(qi: number, solution: AnswerLetter[], n: number, rng: RNG): StatementOption[] {
  const targetIdx = L2I[solution[qi]];
  const options: StatementOption[] = new Array(5);

  const trueClaim = makeTrueClaim(solution, n, rng);
  const trueLabel = claimLabel(trueClaim);
  options[targetIdx] = { label: trueLabel, claim: trueClaim };

  const usedLabels = new Set([trueLabel]);
  for (let i = 0; i < 5; i++) {
    if (i === targetIdx) continue;
    for (let attempt = 0; attempt < 30; attempt++) {
      const fc = makeFalseClaim(solution, n, rng);
      const label = claimLabel(fc);
      if (!usedLabels.has(label)) {
        usedLabels.add(label);
        options[i] = { label, claim: fc };
        break;
      }
    }
    if (!options[i]) {
      const fc = makeFalseClaim(solution, n, rng);
      options[i] = { label: claimLabel(fc), claim: fc };
    }
  }

  return options;
}

function makeTrueClaim(sol: AnswerLetter[], n: number, rng: RNG): Claim {
  const types = [
    "count_answer_equals",
    "count_consonant_answers_equals",
    "count_vowel_answers_equals",
    "count_answer_after_equals",
    "count_answer_before_equals",
  ] as const;
  const type = rng.pick(types);

  switch (type) {
    case "count_answer_equals": {
      const answer = rng.pick(LETTERS);
      return { type, answer, value: sol.filter((a) => a === answer).length };
    }
    case "count_consonant_answers_equals":
      return { type, value: sol.filter((a) => a !== "A" && a !== "E").length };
    case "count_vowel_answers_equals":
      return { type, value: sol.filter((a) => a === "A" || a === "E").length };
    case "count_answer_after_equals": {
      const answer = rng.pick(LETTERS);
      const afterIndex = rng.int(0, n - 2);
      return {
        type,
        answer,
        afterIndex,
        value: sol.slice(afterIndex + 1).filter((a) => a === answer).length,
      };
    }
    case "count_answer_before_equals": {
      const answer = rng.pick(LETTERS);
      const beforeIndex = rng.int(1, n - 1);
      return {
        type,
        answer,
        beforeIndex,
        value: sol.slice(0, beforeIndex).filter((a) => a === answer).length,
      };
    }
  }
  return { type: "count_answer_equals", answer: "A", value: 0 };
}

function makeFalseClaim(sol: AnswerLetter[], n: number, rng: RNG): Claim {
  for (let attempt = 0; attempt < 30; attempt++) {
    const base = makeTrueClaim(sol, n, rng);
    const offset = rng.pick([-2, -1, 1, 2]);
    const newVal = (base as { value: number }).value + offset;
    if (newVal < 0 || newVal > n) continue;
    const falseClaim = { ...base, value: newVal };
    if (!evaluateClaim(falseClaim, sol)) return falseClaim;
  }
  return { type: "count_answer_equals", answer: "A", value: n + 1 };
}

function claimLabel(claim: Claim): string {
  switch (claim.type) {
    case "count_answer_equals":
      return `How many questions have answer ${claim.answer}? ${claim.value}`;
    case "count_consonant_answers_equals":
      return `How many questions have a consonant as the answer? ${claim.value}`;
    case "count_vowel_answers_equals":
      return `How many questions have a vowel as the answer? ${claim.value}`;
    case "count_answer_after_equals":
      return `How many questions after #${claim.afterIndex + 1} have answer ${claim.answer}? ${claim.value}`;
    case "count_answer_before_equals":
      return `How many questions before #${claim.beforeIndex + 1} have answer ${claim.answer}? ${claim.value}`;
  }
  claim satisfies never;
  return "";
}

// ── Question text ──

function questionText(rule: ValidationRule): string {
  switch (rule.type) {
    case "count_answer":
      return `How many questions have answer ${rule.answer}?`;
    case "count_answer_before":
      return `How many questions before #${rule.beforeIndex + 1} have answer ${rule.answer}?`;
    case "count_answer_after":
      return `How many questions after #${rule.afterIndex + 1} have answer ${rule.answer}?`;
    case "count_vowel_answers":
      return "How many questions have a vowel as the answer?";
    case "count_consonant_answers":
      return "How many questions have a consonant as the answer?";
    case "most_common_count":
      return "How many times does the most common answer occur?";
    case "closest_after":
      return `Which is the closest question after #${rule.afterIndex + 1} that has answer ${rule.answer}?`;
    case "closest_before":
      return `Which is the closest question before #${rule.beforeIndex + 1} that has answer ${rule.answer}?`;
    case "first_with_answer":
      return `Which is the first question with answer ${rule.answer}?`;
    case "last_with_answer":
      return `Which is the last question with answer ${rule.answer}?`;
    case "previous_same_answer":
      return "Which is the previous question that has the same answer as this one?";
    case "next_same_answer":
      return "Which is the next question that has the same answer as this one?";
    case "only_same_answer":
      return "The only other question with the same answer as this one is?";
    case "same_answer_as":
      return "The answer to this question is the same as the answer to question?";
    case "only_odd_with_answer":
      return `The only odd-numbered question with answer ${rule.answer} is?`;
    case "consecutive_identical":
      return "The only two consecutive questions with identical answers are?";
    case "answer_of_question":
      return `What is the answer to question #${rule.questionIndex + 1}?`;
    case "least_common_answer":
      return "Which is the least common answer?";
    case "most_common_answer":
      return "Which is the most common answer?";
    case "unique_answer":
      return "The answer that is not the answer to any other question is?";
    case "equal_count_as":
      return `The number of questions with answer ${rule.answer} equals the number of questions with answer?`;
    case "answer_is_self":
      return "What is the answer to this question?";
    case "letter_distance":
      return `How many letters away is the answer to this question from the answer to question #${rule.otherQuestionIndex + 1}?`;
    case "only_true_statement":
      return "Which statement is the only true statement?";
  }
  rule satisfies never;
  return "";
}
