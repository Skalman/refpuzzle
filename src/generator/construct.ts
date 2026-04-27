/**
 * Constructive puzzle generator v2 — builds a deduction chain intentionally.
 *
 * Strategy:
 * 1. Pick random solution
 * 2. Place a counting entry point (crackable via lookahead)
 * 3. Build answer_of_question chain from the entry point (forces cascade)
 * 4. Fill remaining slots with variety rules (positional, letter_distance, etc.)
 * 5. Check structural constraints + uniqueness (skip expensive solvability check)
 */
import type {
  AnswerLetter,
  Puzzle,
  QuestionDef,
  OptionDef,
  ValidationRule,
  Claim,
  StatementOption,
} from "../engine/types.ts";
import type { Marks } from "../engine/types.ts";
import { LETTERS, L2I, flattenPuzzle } from "../engine/types.ts";
import { evaluate, evaluateClaim } from "../engine/evaluators.ts";
import { findHint, findActionFast } from "../engine/hints.ts";
import { solve } from "./solver.ts";
import { RNG } from "./rng.ts";
import type { DifficultyProfile } from "./difficulty.ts";

interface GenerateResult {
  puzzle: Puzzle;
  solution: AnswerLetter[];
}

export function generateConstructive(
  profile: DifficultyProfile,
  rng: RNG,
  maxAttempts = 500,
): GenerateResult | null {
  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    const result = tryConstructive(profile, rng);
    if (result) return result;
  }
  return null;
}

const CONSTRAINED_TYPES = new Set<string>(["unique_answer", "equal_count_as", "answer_is_self"]);

// Rule types by category
const ENTRY_TYPES: ValidationRule["type"][] = [
  "count_answer",
  "count_answer_before",
  "count_answer_after",
  "count_vowel_answers",
  "count_consonant_answers",
];

const POSITIONAL_TYPES: ValidationRule["type"][] = [
  "first_with_answer",
  "last_with_answer",
  "closest_after",
  "closest_before",
];

const VARIETY_TYPES: ValidationRule["type"][] = [
  "letter_distance",
  "consecutive_identical",
  "most_common_count",
  "previous_same_answer",
  "next_same_answer",
  "only_same_answer",
  "same_answer_as",
  "only_odd_with_answer",
  "least_common_answer",
  "most_common_answer",
  "unique_answer",
  "equal_count_as",
  "answer_is_self",
  "only_true_statement",
];

const STRUCTURAL_TYPES = new Set<ValidationRule["type"]>([
  "consecutive_identical",
  "unique_answer",
  "only_same_answer",
  "only_odd_with_answer",
  "equal_count_as",
]);

function typeCap(type: ValidationRule["type"]): number {
  if (type === "letter_distance") return 1;
  if (type === "answer_of_question") return 2;
  return 3;
}

function symmetricGroup(type: ValidationRule["type"]): string | null {
  switch (type) {
    case "first_with_answer":
    case "last_with_answer":
      return "first_last";
    case "closest_after":
    case "closest_before":
      return "closest";
    case "next_same_answer":
    case "previous_same_answer":
      return "next_prev_same";
    case "count_answer_before":
    case "count_answer_after":
      return "count_before_after";
    case "count_vowel_answers":
    case "count_consonant_answers":
      return "count_vowel_consonant";
    case "least_common_answer":
    case "most_common_answer":
      return "least_most_common";
    default:
      return null;
  }
}

function allowed(
  types: ValidationRule["type"][],
  profile: DifficultyProfile,
): ValidationRule["type"][] {
  return types.filter((t) => profile.allowedTypes.includes(t));
}

function tryConstructive(profile: DifficultyProfile, rng: RNG): GenerateResult | null {
  const n = profile.questionCount;

  // 1. Random solution
  const solution: AnswerLetter[] = Array.from({ length: n }, () => rng.pick(LETTERS));

  // Bias toward exactly 1 consecutive pair for levels that allow consecutive_identical
  if (profile.allowedTypes.includes("consecutive_identical") && rng.int(0, 1) === 0) {
    const pairPositions: number[] = [];
    for (let i = 0; i < n - 1; i++) if (solution[i] === solution[i + 1]) pairPositions.push(i);
    if (pairPositions.length === 0) {
      const pos = rng.int(0, n - 2);
      solution[pos + 1] = solution[pos];
    } else if (pairPositions.length > 1) {
      const keep = rng.int(0, pairPositions.length - 1);
      for (let k = 0; k < pairPositions.length; k++) {
        if (k === keep) continue;
        const pos = pairPositions[k] + 1;
        for (;;) {
          const nl = rng.pick(LETTERS);
          if (nl !== solution[pos - 1] && (pos + 1 >= n || nl !== solution[pos + 1])) {
            solution[pos] = nl;
            break;
          }
        }
      }
    }
  }

  // 2. Shuffle question indices — we'll assign rules in this order
  const slots = Array.from({ length: n }, (_, i) => i);
  rng.shuffle(slots);

  const rules: (ValidationRule | null)[] = new Array(n).fill(null);
  const assigned = new Set<number>(); // questions with rules assigned
  const usedRuleKeys = new Set<string>(); // for dedup (no duplicate question text)

  const avEntry = allowed(ENTRY_TYPES, profile);
  const avPositional = allowed(POSITIONAL_TYPES, profile);
  const avVariety = allowed(VARIETY_TYPES, profile);

  const kindCounts: Record<string, number> = {};
  const groupCounts: Record<string, number> = {};

  // Variant: 25% of the time for levels with letter_distance,
  // trade letter_distance for a 3rd answer_of chain
  const extraChain = profile.allowedTypes.includes("letter_distance")
    && rng.int(0, 3) === 0;

  const capsOverride: Record<string, number> = {};
  if (extraChain) {
    capsOverride["answer_of_question"] = 3;
    capsOverride["letter_distance"] = 0;
  }

  function placeRule(type: ValidationRule["type"], slotIdx: number): boolean {
    const cap = capsOverride[type] ?? typeCap(type);
    if ((kindCounts[type] ?? 0) >= cap) return false;
    const group = symmetricGroup(type);
    if (group !== null && (groupCounts[group] ?? 0) >= 3) return false;
    const qi = slots[slotIdx];
    if (!solutionCompatible(type, qi, solution, n)) return false;
    for (let attempt = 0; attempt < 10; attempt++) {
      const rule = makeRule(type, qi, n, solution, assigned, rng);
      if (!rule) continue;
      const key = JSON.stringify(rule);
      if (usedRuleKeys.has(key)) continue;
      if (!checkStructural(rule, qi, solution)) continue;
      rules[qi] = rule;
      assigned.add(qi);
      usedRuleKeys.add(key);
      kindCounts[type] = (kindCounts[type] ?? 0) + 1;
      if (group !== null) groupCounts[group] = (groupCounts[group] ?? 0) + 1;
      return true;
    }
    return false;
  }

  function placeFrom(types: ValidationRule["type"][]): boolean {
    if (types.length === 0 || assigned.size >= n) return false;
    return placeRule(rng.pick(types), assigned.size);
  }

  // Phase 1: Counting entry point (crackable via lookahead)
  if (avEntry.length === 0) return null;
  if (!placeFrom(avEntry)) return null;

  // Phase 2: answer_of_question backbone
  const chainCount = Math.min(
    extraChain ? 3 : n <= 5 && rng.int(0, 1) === 0 ? 1 : 2,
    n - assigned.size,
  );
  for (let c = 0; c < chainCount; c++) {
    if (!placeRule("answer_of_question", assigned.size)) return null;
  }

  // Phase 2b: occasionally place prev/next_same (need specific slot positions)
  if (rng.int(0, 1) === 0 && assigned.size < n) {
    const candidates: [ValidationRule["type"], number][] = [
      ["previous_same_answer", n - 1],
      ["next_same_answer", 0],
    ];
    const [kind, neededQi] = candidates[rng.int(0, 1)];
    if (profile.allowedTypes.includes(kind) && !assigned.has(neededQi)) {
      const idx = slots.indexOf(neededQi, assigned.size);
      if (idx >= 0) {
        [slots[assigned.size], slots[idx]] = [slots[idx], slots[assigned.size]];
        placeRule(kind, assigned.size);
      }
    }
  }

  // Phase 3: 2-3 positional rules (strong lookahead entry points)
  const posCount = Math.min(
    avPositional.length > 0 ? Math.max(2, Math.floor(n / 5)) : 0,
    n - assigned.size,
  );
  for (let p = 0; p < posCount; p++) placeFrom(avPositional);

  // Phase 4: Guaranteed exotic types for variety
  const exoticSlots: ValidationRule["type"][] = [];
  if (allowed(["letter_distance"], profile).length > 0) exoticSlots.push("letter_distance");
  if (allowed(["only_true_statement"], profile).length > 0) exoticSlots.push("only_true_statement");
  if (allowed(["consecutive_identical"], profile).length > 0)
    exoticSlots.push("consecutive_identical");
  for (const type of exoticSlots) {
    if (assigned.size >= n) break;
    placeRule(type, assigned.size);
  }

  // Phase 5: Fill remaining, reserving slots for structural rules
  const avStructural = avVariety.filter((t) => STRUCTURAL_TYPES.has(t));
  const structuralReserve = Math.min(avStructural.length > 0 ? 1 : 0, n - assigned.size);
  const fillTarget = n - structuralReserve;

  const fillPool: ValidationRule["type"][] = [...avEntry, ...avPositional, ...avVariety].filter(
    (t) => profile.allowedTypes.includes(t) && t !== "answer_of_question",
  );

  while (assigned.size < fillTarget) {
    let placed = false;
    for (let attempt = 0; attempt < 20; attempt++) {
      if (placeFrom(fillPool)) {
        placed = true;
        break;
      }
    }
    if (!placed) {
      if (
        !placeRule("answer_of_question", assigned.size) &&
        !placeRule("answer_is_self", assigned.size)
      )
        return null;
    }
  }

  // Phase 6: Structural rules — inspect solution, pick matching types
  for (let s = 0; s < structuralReserve && assigned.size < n; s++) {
    const qi = slots[assigned.size];
    const fitting = avStructural.filter((t) => solutionCompatible(t, qi, solution, n));
    rng.shuffle(fitting);
    let placed = false;
    for (const type of fitting) {
      if (placeRule(type, assigned.size)) {
        placed = true;
        break;
      }
    }
    if (!placed) {
      for (let attempt = 0; attempt < 20; attempt++) {
        if (placeFrom(fillPool)) {
          placed = true;
          break;
        }
      }
      if (
        !placed &&
        !placeRule("answer_of_question", assigned.size) &&
        !placeRule("answer_is_self", assigned.size)
      )
        return null;
    }
  }

  // 3. Build and validate puzzle
  const finalRules: ValidationRule[] = rules.filter((r): r is ValidationRule => r !== null);
  const questions: QuestionDef[] = finalRules.map((rule, i) => ({
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
    if (!evaluate(fp.rules[i], i, solution[i], solution, fp)) return null;
  }

  // 4. Check uniqueness
  const solutions = solve(puzzle, undefined, 2);
  if (solutions.length !== 1) return null;

  // 5. Check solvability (with distractor repair on failure)
  const stuckState = runHintEngine(puzzle, n);
  if (stuckState.solved) return { puzzle, solution: solutions[0] };

  for (let retry = 0; retry < 3; retry++) {
    repairDistractors(puzzle, solution, stuckState.answers, n, rng);
    const fp2 = flattenPuzzle(puzzle);
    let evalOk = true;
    for (let i = 0; i < n; i++) {
      if (!evaluate(fp2.rules[i], i, solution[i], solution, fp2)) {
        evalOk = false;
        break;
      }
    }
    if (!evalOk) return null;
    const sols2 = solve(puzzle, undefined, 2);
    if (sols2.length !== 1) continue;
    if (checkSolvable(puzzle, n)) return { puzzle, solution: sols2[0] };
  }

  return null;
}

function checkSolvable(puzzle: Puzzle, n: number): boolean {
  return runHintEngine(puzzle, n).solved;
}

function runHintEngine(
  puzzle: Puzzle,
  n: number,
): { solved: boolean; answers: (AnswerLetter | null)[] } {
  const marks: Marks[] = Array.from(
    { length: n },
    () => ["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"] as Marks,
  );
  const answers: (AnswerLetter | null)[] = new Array(n).fill(null);
  for (let step = 0; step < n * 15; step++) {
    if (answers.every((a) => a != null)) return { solved: true, answers };
    const fast = findActionFast(puzzle, answers, marks, n);
    if (fast) {
      applyAction(fast, marks, answers);
      continue;
    }
    const hint = findHint(puzzle, marks);
    if (!hint?.action) return { solved: false, answers };
    applyAction(hint.action, marks, answers);
  }
  return { solved: false, answers };
}

function repairDistractors(
  puzzle: Puzzle,
  solution: AnswerLetter[],
  stuckAnswers: (AnswerLetter | null)[],
  n: number,
  rng: RNG,
): void {
  for (let qi = 0; qi < n; qi++) {
    if (stuckAnswers[qi] != null) continue;
    const rule = puzzle.questions[qi].rule;
    const correctOi = L2I[solution[qi]];

    if (CONSTRAINED_TYPES.has(rule.type)) continue;
    if (rule.type === "only_true_statement") continue;

    const opts = puzzle.questions[qi].options;

    if (rule.type === "answer_of_question") {
      const target = stuckAnswers[rule.questionIndex];
      if (target != null) {
        const correctIdx = L2I[target];
        const pool = rng.shuffle([0, 1, 2, 3, 4].filter((i) => i !== correctIdx));
        let di = 0;
        for (let oi = 0; oi < 5; oi++) {
          if (oi !== correctOi) opts[oi] = { value: pool[di++] };
        }
      }
      continue;
    }

    const correctVal = opts[correctOi].value;

    if (rule.type === "letter_distance" && rule.questionIndex != null) {
      const other = stuckAnswers[rule.questionIndex];
      if (other != null) {
        const correctDist = Math.abs(L2I[solution[qi]] - L2I[other]);
        const pool = rng.shuffle([0, 1, 2, 3, 4].filter((v) => v !== correctDist));
        let di = 0;
        for (let oi = 0; oi < 5; oi++) {
          if (oi !== correctOi) opts[oi] = { value: pool[di++] };
        }
      }
      continue;
    }

    if (isCountingType(rule.type)) {
      const distractors = repairCountingDistractors(rule, correctVal, stuckAnswers, n, rng);
      let di = 0;
      for (let oi = 0; oi < 5; oi++) {
        if (oi !== correctOi) opts[oi] = { value: distractors[di++] };
      }
      continue;
    }

    if (rule.type === "consecutive_identical") {
      const distractors = repairPairDistractors(correctVal, stuckAnswers, n, rng);
      let di = 0;
      for (let oi = 0; oi < 5; oi++) {
        if (oi !== correctOi) opts[oi] = { value: distractors[di++] };
      }
      continue;
    }

    // Positional rules — generate new distractors
    const newDistractors = repairPositionalDistractors(rule, correctVal, qi, stuckAnswers, n, rng);
    let di = 0;
    for (let oi = 0; oi < 5; oi++) {
      if (oi !== correctOi) opts[oi] = { value: newDistractors[di++] };
    }
  }
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

function repairCountingDistractors(
  rule: ValidationRule,
  correctVal: number | null,
  answers: (AnswerLetter | null)[],
  n: number,
  rng: RNG,
): (number | null)[] {
  const from = rule.type === "count_answer_after" ? rule.afterIndex + 1 : 0;
  const to = rule.type === "count_answer_before" ? rule.beforeIndex : n;

  let confirmed = 0;
  let unknown = 0;
  for (let i = from; i < to; i++) {
    if (answers[i] == null) {
      unknown++;
    } else if (
      rule.type === "count_vowel_answers"
        ? answers[i] === "A" || answers[i] === "E"
        : rule.type === "count_consonant_answers"
          ? answers[i] !== "A" && answers[i] !== "E"
          : "answer" in rule && answers[i] === rule.answer
    ) {
      confirmed++;
    }
  }

  const pool: number[] = [];
  for (let v = 0; v < confirmed; v++) if (v !== correctVal) pool.push(v);
  for (let v = confirmed + unknown + 1; v <= n; v++) if (v !== correctVal) pool.push(v);
  const max =
    rule.type === "count_answer_before"
      ? rule.beforeIndex
      : rule.type === "count_answer_after"
        ? n - rule.afterIndex - 1
        : n;
  for (let v = 0; v <= Math.max(max, 4); v++) {
    if (v !== correctVal && !pool.includes(v)) pool.push(v);
  }
  return rng.shuffle(pool).slice(0, 4);
}

function repairPairDistractors(
  correctVal: number | null,
  answers: (AnswerLetter | null)[],
  n: number,
  rng: RNG,
): (number | null)[] {
  const pool: (number | null)[] = [];
  for (let i = 0; i < n - 1; i++) {
    if (i === correctVal) continue;
    if (answers[i] != null && answers[i + 1] != null && answers[i] !== answers[i + 1]) {
      pool.unshift(i);
    } else {
      pool.push(i);
    }
  }
  if (correctVal != null) pool.push(null);
  return rng.shuffle(pool).slice(0, 4);
}

function repairPositionalDistractors(
  rule: ValidationRule,
  correctVal: number | null,
  qi: number,
  answers: (AnswerLetter | null)[],
  n: number,
  rng: RNG,
): (number | null)[] {
  const answer = "answer" in rule ? rule.answer : undefined;
  let minPos = 0;
  let maxPos = n - 1;
  if (rule.type === "closest_after") minPos = rule.afterIndex + 1;
  if (rule.type === "closest_before") maxPos = rule.beforeIndex - 1;
  if (rule.type === "previous_same_answer") maxPos = qi - 1;
  if (rule.type === "next_same_answer") minPos = qi + 1;

  const pool: (number | null)[] = [];

  if (answer) {
    for (let i = minPos; i <= maxPos; i++) {
      if (i === correctVal) continue;
      if (answers[i] != null && answers[i] !== answer) {
        pool.unshift(i);
      } else {
        pool.push(i);
      }
    }
    const hasMatch = answers.slice(minPos, maxPos + 1).some((a) => a === answer);
    if (correctVal != null && hasMatch) pool.unshift(null);
    else if (correctVal != null) pool.push(null);
  } else {
    for (let i = minPos; i <= maxPos; i++) {
      if (i !== correctVal) pool.push(i);
    }
    if (correctVal != null) pool.push(null);
  }

  return rng.shuffle(pool).slice(0, 4);
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

// ── Rule factory ──

function makeRule(
  type: ValidationRule["type"],
  qi: number,
  n: number,
  _solution: AnswerLetter[],
  assigned: Set<number>,
  rng: RNG,
): ValidationRule | null {
  switch (type) {
    case "count_answer":
      return { type, answer: rng.pick(LETTERS) };
    case "count_answer_before":
      if (n < 6) return null;
      return { type, answer: rng.pick(LETTERS), beforeIndex: rng.int(4, n - 1) };
    case "count_answer_after":
      if (n < 6) return null;
      return { type, answer: rng.pick(LETTERS), afterIndex: rng.int(0, Math.max(0, n - 5)) };
    case "count_vowel_answers":
    case "count_consonant_answers":
    case "most_common_count":
      return { type };
    case "answer_of_question": {
      const targets = [...assigned].filter((j) => j !== qi);
      if (targets.length === 0) return null;
      return { type, questionIndex: rng.pick(targets) };
    }
    case "letter_distance": {
      const targets = [...assigned].filter((j) => j !== qi);
      if (targets.length === 0) {
        // Can point at any other question
        const pool = [];
        for (let j = 0; j < n; j++) if (j !== qi) pool.push(j);
        return { type, questionIndex: rng.pick(pool) };
      }
      return { type, questionIndex: rng.pick(targets) };
    }
    case "closest_after":
      return { type, afterIndex: rng.int(0, Math.max(0, n - 5)), answer: rng.pick(LETTERS) };
    case "closest_before":
      if (n < 5) return null;
      return { type, beforeIndex: rng.int(4, n - 1), answer: rng.pick(LETTERS) };
    case "first_with_answer":
    case "last_with_answer":
      return { type, answer: rng.pick(LETTERS) };
    case "previous_same_answer":
      if (qi < 4) return null;
      return { type };
    case "next_same_answer":
      if (qi + 5 > n) return null;
      return { type };
    case "only_same_answer":
    case "same_answer_as":
    case "consecutive_identical":
    case "least_common_answer":
    case "most_common_answer":
    case "unique_answer":
      return { type };
    case "equal_count_as":
      return { type, answer: rng.pick(LETTERS) };
    case "only_odd_with_answer":
      return { type, answer: rng.pick(LETTERS) };
    case "answer_is_self":
      return { type };
    case "only_true_statement":
      return { type };
  }
  return null;
}

// ── Structural checks ──

function checkStructural(rule: ValidationRule, qi: number, sol: AnswerLetter[]): boolean {
  switch (rule.type) {
    case "only_same_answer": {
      let m = 0;
      for (let i = 0; i < sol.length; i++) if (i !== qi && sol[i] === sol[qi]) m++;
      return m === 1;
    }
    case "consecutive_identical": {
      let p = 0;
      for (let i = 0; i < sol.length - 1; i++) if (sol[i] === sol[i + 1]) p++;
      return p === 1;
    }
    case "only_odd_with_answer": {
      let m = 0;
      for (let i = 0; i < sol.length; i++) if ((i + 1) % 2 === 1 && sol[i] === rule.answer) m++;
      return m === 1;
    }
    case "unique_answer":
      return sol.filter((a) => a === sol[qi]).length === 1;
    case "equal_count_as": {
      const rc = sol.filter((a) => a === rule.answer).length;
      return LETTERS.some((l) => l !== rule.answer && sol.filter((a) => a === l).length === rc);
    }
    default:
      return true;
  }
}

function solutionHasStructural(
  type: ValidationRule["type"],
  qi: number,
  solution: AnswerLetter[],
  n: number,
): boolean {
  switch (type) {
    case "consecutive_identical": {
      let pairs = 0;
      for (let i = 0; i < n - 1; i++) if (solution[i] === solution[i + 1]) pairs++;
      return pairs === 1;
    }
    case "unique_answer":
      return solution.slice(0, n).filter((a) => a === solution[qi]).length === 1;
    case "only_same_answer": {
      let m = 0;
      for (let i = 0; i < n; i++) if (i !== qi && solution[i] === solution[qi]) m++;
      return m === 1;
    }
    case "only_odd_with_answer": {
      for (const letter of LETTERS) {
        let m = 0;
        for (let i = 0; i < n; i++) if ((i + 1) % 2 === 1 && solution[i] === letter) m++;
        if (m === 1) return true;
      }
      return false;
    }
    case "equal_count_as": {
      const c = [0, 0, 0, 0, 0];
      for (let i = 0; i < n; i++) c[L2I[solution[i]]]++;
      for (let a = 0; a < 5; a++) for (let b = a + 1; b < 5; b++) if (c[a] === c[b]) return true;
      return false;
    }
  }
  return false;
}

function solutionCompatible(
  type: ValidationRule["type"],
  qi: number,
  solution: AnswerLetter[],
  n: number,
): boolean {
  switch (type) {
    case "least_common_answer": {
      const c = letterCounts(solution.slice(0, n));
      return c.filter((v) => v === Math.min(...c)).length === 1;
    }
    case "most_common_answer": {
      const c = letterCounts(solution.slice(0, n));
      return c.filter((v) => v === Math.max(...c)).length === 1;
    }
    case "same_answer_as": {
      for (let i = 0; i < n; i++) if (i !== qi && solution[i] === solution[qi]) return true;
      return false;
    }
    case "equal_count_as": {
      const c = letterCounts(solution.slice(0, n));
      const qiCount = c[L2I[solution[qi]]];
      return LETTERS.some((l) => l !== solution[qi] && c[L2I[l]] === qiCount);
    }
  }
  if (STRUCTURAL_TYPES.has(type)) return solutionHasStructural(type, qi, solution, n);
  return true;
}

// ── Options, text, distractors ──

function letterCounts(sol: AnswerLetter[]): number[] {
  const c = [0, 0, 0, 0, 0];
  for (const a of sol) c[L2I[a]]++;
  return c;
}

function engineerOptions(
  rule: ValidationRule,
  qi: number,
  solution: AnswerLetter[],
  n: number,
  rng: RNG,
): OptionDef[] {
  // Constrained types: value is the letter index (0=A, 1=B, etc.)
  if (CONSTRAINED_TYPES.has(rule.type)) return LETTERS.map((_l, i) => ({ value: i }));
  if (rule.type === "only_true_statement") return buildClaims(qi, solution, n, rng);
  if (rule.type === "least_common_answer" || rule.type === "most_common_answer") {
    const counts = letterCounts(solution.slice(0, n));
    const target = rule.type === "least_common_answer" ? Math.min(...counts) : Math.max(...counts);
    const correctIdx = counts.indexOf(target);
    const targetIdx = L2I[solution[qi]];
    const pool = rng.shuffle([0, 1, 2, 3, 4].filter((i) => i !== correctIdx));
    const opts: OptionDef[] = new Array(5);
    opts[targetIdx] = { value: correctIdx };
    let di = 0;
    for (let i = 0; i < 5; i++) if (i !== targetIdx) opts[i] = { value: pool[di++] };
    return opts;
  }
  const correct = computeValue(rule, qi, solution);
  const targetIdx = L2I[solution[qi]];
  const distractors = makeDistractors(rule, correct, qi, n, rng);
  const opts: OptionDef[] = new Array(5);
  opts[targetIdx] = { value: correct };
  let di = 0;
  for (let i = 0; i < 5; i++) if (i !== targetIdx) opts[i] = { value: distractors[di++] };
  return opts;
}

function computeValue(rule: ValidationRule, qi: number, sol: AnswerLetter[]): number | null {
  switch (rule.type) {
    case "answer_of_question":
      return L2I[sol[rule.questionIndex]];
    case "count_answer":
      return sol.filter((a) => a === rule.answer).length;
    case "count_answer_before":
      return sol.slice(0, rule.beforeIndex).filter((a) => a === rule.answer).length;
    case "count_answer_after":
      return sol.slice(rule.afterIndex + 1).filter((a) => a === rule.answer).length;
    case "count_vowel_answers":
      return sol.filter((a) => a === "A" || a === "E").length;
    case "count_consonant_answers":
      return sol.filter((a) => a !== "A" && a !== "E").length;
    case "most_common_count":
      return Math.max(...letterCounts(sol));
    case "closest_after":
      for (let i = rule.afterIndex + 1; i < sol.length; i++)
        if (sol[i] === rule.answer) return i;
      return null;
    case "closest_before":
      for (let i = rule.beforeIndex - 1; i >= 0; i--)
        if (sol[i] === rule.answer) return i;
      return null;
    case "first_with_answer":
      for (let i = 0; i < sol.length; i++) if (sol[i] === rule.answer) return i;
      return null;
    case "last_with_answer":
      for (let i = sol.length - 1; i >= 0; i--) if (sol[i] === rule.answer) return i;
      return null;
    case "previous_same_answer":
      for (let i = qi - 1; i >= 0; i--) if (sol[i] === sol[qi]) return i;
      return null;
    case "next_same_answer":
      for (let i = qi + 1; i < sol.length; i++) if (sol[i] === sol[qi]) return i;
      return null;
    case "only_same_answer":
      for (let i = 0; i < sol.length; i++) if (i !== qi && sol[i] === sol[qi]) return i;
      return null;
    case "same_answer_as":
      for (let i = 0; i < sol.length; i++) if (i !== qi && sol[i] === sol[qi]) return i;
      return null;
    case "only_odd_with_answer":
      for (let i = 0; i < sol.length; i++)
        if ((i + 1) % 2 === 1 && sol[i] === rule.answer) return i;
      return null;
    case "consecutive_identical":
      for (let i = 0; i < sol.length - 1; i++)
        if (sol[i] === sol[i + 1]) return i;
      return null;
    case "letter_distance":
      return Math.abs(L2I[sol[qi]] - L2I[sol[rule.questionIndex]]);
  }
  throw new Error(`computeValue: ${rule.type}`);
}

function makeDistractors(rule: ValidationRule, correct: number | null, qi: number, n: number, rng: RNG): (number | null)[] {
  if (rule.type === "answer_of_question")
    return rng.shuffle([0, 1, 2, 3, 4].filter((v) => v !== correct));
  if (rule.type === "letter_distance")
    return rng.shuffle([0, 1, 2, 3, 4].filter((v) => v !== correct)).slice(0, 4);
  if (rule.type === "consecutive_identical") {
    const pool: (number | null)[] = [];
    for (let i = 0; i < n - 1; i++) {
      if (i !== correct) pool.push(i);
    }
    if (correct != null) pool.push(null);
    return rng.shuffle(pool).slice(0, 4);
  }
  if (
    [
      "count_answer",
      "count_answer_before",
      "count_answer_after",
      "count_vowel_answers",
      "count_consonant_answers",
      "most_common_count",
    ].includes(rule.type)
  ) {
    let max = n;
    if (rule.type === "count_answer_before") max = rule.beforeIndex;
    if (rule.type === "count_answer_after") max = n - rule.afterIndex - 1;
    const pool: number[] = [];
    for (let i = 0; i <= Math.max(max, 4); i++) if (i !== correct) pool.push(i);
    return rng.shuffle(pool).slice(0, 4);
  }
  if (rule.type === "only_odd_with_answer") {
    const pool: (number | null)[] = [];
    for (let i = 0; i < n; i++) {
      if ((i + 1) % 2 === 1 && i !== correct) pool.push(i);
    }
    if (correct != null) pool.push(null);
    return rng.shuffle(pool).slice(0, 4);
  }
  // Positional (0-based indices)
  let minPos = 0,
    maxPos = n - 1;
  if (rule.type === "closest_after") minPos = rule.afterIndex + 1;
  if (rule.type === "closest_before") maxPos = rule.beforeIndex - 1;
  if (rule.type === "previous_same_answer") maxPos = qi - 1;
  if (rule.type === "next_same_answer") minPos = qi + 1;
  const pool: (number | null)[] = [];
  for (let i = minPos; i <= maxPos; i++) {
    if (i !== correct) pool.push(i);
  }
  if (correct != null) pool.push(null);
  return rng.shuffle(pool).slice(0, 4);
}

function buildClaims(qi: number, solution: AnswerLetter[], n: number, rng: RNG): StatementOption[] {
  const targetIdx = L2I[solution[qi]];
  const options: StatementOption[] = new Array(5);
  const trueClaim = makeTrueClaim(solution, n, rng);
  options[targetIdx] = { value: null, claim: trueClaim };
  const usedKeys = new Set([claimKey(trueClaim)]);
  for (let i = 0; i < 5; i++) {
    if (i === targetIdx) continue;
    for (let att = 0; att < 30; att++) {
      const fc = makeFalseClaim(solution, n, rng);
      const key = claimKey(fc);
      if (!usedKeys.has(key)) {
        usedKeys.add(key);
        options[i] = { value: null, claim: fc };
        break;
      }
    }
    if (!options[i]) {
      const fc = makeFalseClaim(solution, n, rng);
      options[i] = { value: null, claim: fc };
    }
  }
  return options;
}
function makeTrueClaim(sol: AnswerLetter[], n: number, rng: RNG): Claim {
  const t = rng.int(0, 4);
  if (t === 0) {
    const a = rng.pick(LETTERS);
    return { type: "count_answer", answer: a, value: sol.filter((x) => x === a).length };
  }
  if (t === 1)
    return {
      type: "count_consonant_answers",
      value: sol.filter((a) => a !== "A" && a !== "E").length,
    };
  if (t === 2)
    return {
      type: "count_vowel_answers",
      value: sol.filter((a) => a === "A" || a === "E").length,
    };
  if (t === 3) {
    const a = rng.pick(LETTERS);
    const ai = rng.int(0, n - 2);
    return {
      type: "count_answer_after",
      answer: a,
      afterIndex: ai,
      value: sol.slice(ai + 1).filter((x) => x === a).length,
    };
  }
  const a = rng.pick(LETTERS);
  const bi = rng.int(1, n - 1);
  return {
    type: "count_answer_before",
    answer: a,
    beforeIndex: bi,
    value: sol.slice(0, bi).filter((x) => x === a).length,
  };
}
function makeFalseClaim(sol: AnswerLetter[], n: number, rng: RNG): Claim {
  for (let i = 0; i < 30; i++) {
    const base = makeTrueClaim(sol, n, rng);
    const offset = rng.pick([-2, -1, 1, 2]);
    const newVal = (base as { value: number }).value + offset;
    if (newVal < 0 || newVal > n) continue;
    const fc = { ...base, value: newVal };
    if (!evaluateClaim(fc, sol)) return fc;
  }
  return { type: "count_answer", answer: "A", value: n + 1 };
}
function claimKey(c: Claim): string {
  return JSON.stringify(c);
}
