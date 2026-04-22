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

export interface GenerateResult {
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

const CONSTRAINED_TYPES = new Set<string>([
  "least_common_answer",
  "most_common_answer",
  "unique_answer",
  "equal_count_as",
  "answer_is_self",
]);

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

function allowed(types: ValidationRule["type"][], profile: DifficultyProfile): ValidationRule["type"][] {
  return types.filter((t) => profile.allowedTypes.includes(t));
}

function tryConstructive(profile: DifficultyProfile, rng: RNG): GenerateResult | null {
  const n = profile.questionCount;

  // 1. Random solution
  const solution: AnswerLetter[] = Array.from({ length: n }, () => rng.pick(LETTERS));

  // 2. Shuffle question indices — we'll assign rules in this order
  const slots = Array.from({ length: n }, (_, i) => i);
  rng.shuffle(slots);

  const rules: (ValidationRule | null)[] = new Array(n).fill(null);
  const assigned = new Set<number>(); // questions with rules assigned
  const usedRuleKeys = new Set<string>(); // for dedup (no duplicate question text)

  const avEntry = allowed(ENTRY_TYPES, profile);
  const avPositional = allowed(POSITIONAL_TYPES, profile);
  const avVariety = allowed(VARIETY_TYPES, profile);

  function placeRule(type: ValidationRule["type"], slotIdx: number): boolean {
    const qi = slots[slotIdx];
    for (let attempt = 0; attempt < 10; attempt++) {
      const rule = makeRule(type, qi, n, solution, assigned, rng);
      if (!rule) continue;
      const key = questionText(rule);
      if (usedRuleKeys.has(key)) continue;
      if (!checkStructural(rule, qi, solution)) continue;
      rules[qi] = rule;
      assigned.add(qi);
      usedRuleKeys.add(key);
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

  // Phase 2: 2 answer_of_question (cascade backbone — enough but not too many)
  const chainCount = Math.min(2, n - assigned.size);
  for (let c = 0; c < chainCount; c++) {
    if (!placeRule("answer_of_question", assigned.size)) return null;
  }

  // Phase 3: 2-3 positional rules (strong lookahead entry points)
  const posCount = Math.min(avPositional.length > 0 ? Math.max(2, Math.floor(n / 5)) : 0, n - assigned.size);
  for (let p = 0; p < posCount; p++) placeFrom(avPositional);

  // Phase 4: Guaranteed exotic types for variety
  const exoticSlots: ValidationRule["type"][] = [];
  if (allowed(["letter_distance"], profile).length > 0) exoticSlots.push("letter_distance");
  if (allowed(["only_true_statement"], profile).length > 0) exoticSlots.push("only_true_statement");
  if (allowed(["consecutive_identical"], profile).length > 0) exoticSlots.push("consecutive_identical");
  for (const type of exoticSlots) {
    if (assigned.size >= n) break;
    placeRule(type, assigned.size); // ok if it fails (structural constraint)
  }

  // Phase 5: Fill remaining with diverse mix (no answer_of_question — already have enough)
  const fillPool: ValidationRule["type"][] = [
    ...avEntry,
    ...avPositional,
    ...avVariety,
  ].filter((t) => profile.allowedTypes.includes(t) && t !== "answer_of_question");

  while (assigned.size < n) {
    let placed = false;
    for (let attempt = 0; attempt < 20; attempt++) {
      if (placeFrom(fillPool)) { placed = true; break; }
    }
    if (!placed) {
      // Fallback: answer_of_question or answer_is_self
      if (!placeRule("answer_of_question", assigned.size) &&
          !placeRule("answer_is_self", assigned.size)) return null;
    }
  }

  // 3. Build and validate puzzle
  const finalRules = rules as ValidationRule[];
  const questions: QuestionDef[] = finalRules.map((rule, i) => ({
    text: questionText(rule),
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

  // 5. Check solvability
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
    const fast = findActionFast(puzzle, answers, marks, n);
    if (fast) { applyAction(fast, marks, answers); continue; }
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

// ── Rule factory ──

function makeRule(
  type: ValidationRule["type"],
  qi: number,
  n: number,
  solution: AnswerLetter[],
  assigned: Set<number>,
  rng: RNG,
): ValidationRule | null {
  switch (type) {
    case "count_answer":
      return { type, answer: rng.pick(LETTERS) };
    case "count_answer_before":
      if (n < 4) return null;
      return { type, answer: rng.pick(LETTERS), beforeIndex: rng.int(2, n - 1) };
    case "count_answer_after":
      if (n < 4) return null;
      return { type, answer: rng.pick(LETTERS), afterIndex: rng.int(0, Math.max(0, n - 3)) };
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
        return { type, otherQuestionIndex: rng.pick(pool) };
      }
      return { type, otherQuestionIndex: rng.pick(targets) };
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
    case "next_same_answer":
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

// ── Structural checks (same as assemble.ts) ──

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

// ── Options, text, distractors (same logic as assemble.ts) ──

function letterCounts(sol: AnswerLetter[]): number[] {
  const c = [0, 0, 0, 0, 0];
  for (const a of sol) c[L2I[a]]++;
  return c;
}

function engineerOptions(rule: ValidationRule, qi: number, solution: AnswerLetter[], n: number, rng: RNG): OptionDef[] {
  if (CONSTRAINED_TYPES.has(rule.type)) return LETTERS.map((l) => ({ label: l }));
  if (rule.type === "only_true_statement") return buildClaims(qi, solution, n, rng);
  const correct = computeValue(rule, qi, solution);
  const targetIdx = L2I[solution[qi]];
  const distractors = makeDistractors(rule, correct, n, rng);
  const opts: OptionDef[] = new Array(5);
  opts[targetIdx] = { label: correct };
  let di = 0;
  for (let i = 0; i < 5; i++) if (i !== targetIdx) opts[i] = { label: distractors[di++] };
  return opts;
}

function computeValue(rule: ValidationRule, qi: number, sol: AnswerLetter[]): string {
  switch (rule.type) {
    case "answer_of_question": return sol[rule.questionIndex];
    case "count_answer": return String(sol.filter((a) => a === rule.answer).length);
    case "count_answer_before": return String(sol.slice(0, rule.beforeIndex).filter((a) => a === rule.answer).length);
    case "count_answer_after": return String(sol.slice(rule.afterIndex + 1).filter((a) => a === rule.answer).length);
    case "count_vowel_answers": return String(sol.filter((a) => a === "A" || a === "E").length);
    case "count_consonant_answers": return String(sol.filter((a) => a !== "A" && a !== "E").length);
    case "most_common_count": return String(Math.max(...letterCounts(sol)));
    case "closest_after": for (let i = rule.afterIndex + 1; i < sol.length; i++) if (sol[i] === rule.answer) return String(i + 1); return "None";
    case "closest_before": for (let i = rule.beforeIndex - 1; i >= 0; i--) if (sol[i] === rule.answer) return String(i + 1); return "None";
    case "first_with_answer": for (let i = 0; i < sol.length; i++) if (sol[i] === rule.answer) return String(i + 1); return "None";
    case "last_with_answer": for (let i = sol.length - 1; i >= 0; i--) if (sol[i] === rule.answer) return String(i + 1); return "None";
    case "previous_same_answer": for (let i = qi - 1; i >= 0; i--) if (sol[i] === sol[qi]) return String(i + 1); return "None";
    case "next_same_answer": for (let i = qi + 1; i < sol.length; i++) if (sol[i] === sol[qi]) return String(i + 1); return "None";
    case "only_same_answer": for (let i = 0; i < sol.length; i++) if (i !== qi && sol[i] === sol[qi]) return String(i + 1); return "None";
    case "same_answer_as": for (let i = 0; i < sol.length; i++) if (i !== qi && sol[i] === sol[qi]) return String(i + 1); return "None";
    case "only_odd_with_answer": for (let i = 0; i < sol.length; i++) if ((i + 1) % 2 === 1 && sol[i] === rule.answer) return String(i + 1); return "None";
    case "consecutive_identical": for (let i = 0; i < sol.length - 1; i++) if (sol[i] === sol[i + 1]) return `${i + 1} and ${i + 2}`; return "None";
    case "letter_distance": return String(Math.abs(L2I[sol[qi]] - L2I[sol[rule.otherQuestionIndex]]));
  }
  throw new Error(`computeValue: ${rule.type}`);
}

function makeDistractors(rule: ValidationRule, correct: string, n: number, rng: RNG): string[] {
  if (rule.type === "answer_of_question") return rng.shuffle((LETTERS as readonly string[]).filter((v) => v !== correct));
  if (rule.type === "letter_distance") return rng.shuffle(["0", "1", "2", "3", "4"].filter((v) => v !== correct)).slice(0, 4);
  if (rule.type === "consecutive_identical") {
    const pool: string[] = [];
    for (let i = 1; i < n; i++) { const p = `${i} and ${i + 1}`; if (p !== correct) pool.push(p); }
    if (correct !== "None") pool.push("None");
    return rng.shuffle(pool).slice(0, 4);
  }
  if (["count_answer", "count_answer_before", "count_answer_after", "count_vowel_answers", "count_consonant_answers", "most_common_count"].includes(rule.type)) {
    let max = n;
    if (rule.type === "count_answer_before") max = rule.beforeIndex;
    if (rule.type === "count_answer_after") max = n - rule.afterIndex - 1;
    const pool: number[] = [];
    for (let i = 0; i <= Math.max(max, 4); i++) if (i !== Number(correct)) pool.push(i);
    return rng.shuffle(pool).slice(0, 4).map(String);
  }
  // Positional
  let minPos = 1, maxPos = n;
  if (rule.type === "closest_after") minPos = rule.afterIndex + 2;
  if (rule.type === "closest_before") maxPos = rule.beforeIndex;
  const pool: string[] = [];
  for (let i = minPos; i <= maxPos; i++) { const s = String(i); if (s !== correct) pool.push(s); }
  if (correct !== "None") pool.push("None");
  return rng.shuffle(pool).slice(0, 4);
}

function buildClaims(qi: number, solution: AnswerLetter[], n: number, rng: RNG): StatementOption[] {
  const targetIdx = L2I[solution[qi]];
  const options: StatementOption[] = new Array(5);
  const trueClaim = makeTrueClaim(solution, n, rng);
  options[targetIdx] = { label: claimLabel(trueClaim), claim: trueClaim };
  const usedLabels = new Set([options[targetIdx].label]);
  for (let i = 0; i < 5; i++) {
    if (i === targetIdx) continue;
    for (let att = 0; att < 30; att++) {
      const fc = makeFalseClaim(solution, n, rng);
      const label = claimLabel(fc);
      if (!usedLabels.has(label)) { usedLabels.add(label); options[i] = { label, claim: fc }; break; }
    }
    if (!options[i]) { const fc = makeFalseClaim(solution, n, rng); options[i] = { label: claimLabel(fc), claim: fc }; }
  }
  return options;
}
function makeTrueClaim(sol: AnswerLetter[], n: number, rng: RNG): Claim {
  const t = rng.int(0, 4);
  if (t === 0) { const a = rng.pick(LETTERS); return { type: "count_answer_equals", answer: a, value: sol.filter((x) => x === a).length }; }
  if (t === 1) return { type: "count_consonant_answers_equals", value: sol.filter((a) => a !== "A" && a !== "E").length };
  if (t === 2) return { type: "count_vowel_answers_equals", value: sol.filter((a) => a === "A" || a === "E").length };
  if (t === 3) { const a = rng.pick(LETTERS); const ai = rng.int(0, n - 2); return { type: "count_answer_after_equals", answer: a, afterIndex: ai, value: sol.slice(ai + 1).filter((x) => x === a).length }; }
  const a = rng.pick(LETTERS); const bi = rng.int(1, n - 1);
  return { type: "count_answer_before_equals", answer: a, beforeIndex: bi, value: sol.slice(0, bi).filter((x) => x === a).length };
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
  return { type: "count_answer_equals", answer: "A", value: n + 1 };
}
function claimLabel(c: Claim): string {
  switch (c.type) {
    case "count_answer_equals": return `How many questions have answer ${c.answer}? ${c.value}`;
    case "count_consonant_answers_equals": return `How many questions have a consonant as the answer? ${c.value}`;
    case "count_vowel_answers_equals": return `How many questions have a vowel as the answer? ${c.value}`;
    case "count_answer_after_equals": return `How many questions after #${c.afterIndex + 1} have answer ${c.answer}? ${c.value}`;
    case "count_answer_before_equals": return `How many questions before #${c.beforeIndex + 1} have answer ${c.answer}? ${c.value}`;
  }
  return "";
}

function questionText(rule: ValidationRule): string {
  switch (rule.type) {
    case "count_answer": return `How many questions have answer ${rule.answer}?`;
    case "count_answer_before": return `How many questions before #${rule.beforeIndex + 1} have answer ${rule.answer}?`;
    case "count_answer_after": return `How many questions after #${rule.afterIndex + 1} have answer ${rule.answer}?`;
    case "count_vowel_answers": return "How many questions have a vowel as the answer?";
    case "count_consonant_answers": return "How many questions have a consonant as the answer?";
    case "most_common_count": return "How many times does the most common answer occur?";
    case "closest_after": return `Which is the closest question after #${rule.afterIndex + 1} that has answer ${rule.answer}?`;
    case "closest_before": return `Which is the closest question before #${rule.beforeIndex + 1} that has answer ${rule.answer}?`;
    case "first_with_answer": return `Which is the first question with answer ${rule.answer}?`;
    case "last_with_answer": return `Which is the last question with answer ${rule.answer}?`;
    case "previous_same_answer": return "Which is the previous question that has the same answer as this one?";
    case "next_same_answer": return "Which is the next question that has the same answer as this one?";
    case "only_same_answer": return "The only other question with the same answer as this one is?";
    case "same_answer_as": return "The answer to this question is the same as the answer to question?";
    case "only_odd_with_answer": return `The only odd-numbered question with answer ${rule.answer} is?`;
    case "consecutive_identical": return "The only two consecutive questions with identical answers are?";
    case "answer_of_question": return `What is the answer to question #${rule.questionIndex + 1}?`;
    case "least_common_answer": return "Which is the least common answer?";
    case "most_common_answer": return "Which is the most common answer?";
    case "unique_answer": return "The answer that is not the answer to any other question is?";
    case "equal_count_as": return `The number of questions with answer ${rule.answer} equals the number of questions with answer?`;
    case "answer_is_self": return "What is the answer to this question?";
    case "letter_distance": return `How many letters away is the answer to this question from the answer to question #${rule.otherQuestionIndex + 1}?`;
    case "only_true_statement": return "Which statement is the only true statement?";
  }
  return "";
}
