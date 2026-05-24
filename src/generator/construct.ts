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
  Answer,
  Puzzle,
  QuestionDef,
  OptionDef,
  QuestionType,
  Claim,
  StatementOption,
} from "../engine/types.ts";
import { LETTERS, L2I, flattenPuzzle } from "../engine/types.ts";
import type { State } from "../engine/types.ts";
import { checkAnswers, checkClaimFast } from "../engine/check-answer.ts";
import { deduce } from "../engine/deduce.ts";
import type { DeduceAction } from "../engine/deduce.ts";
import { lookahead } from "../engine/lookahead.ts";
import { solve } from "./solve-brute.ts";
import { checkForm } from "../engine/check-form.ts";
import { RNG } from "./rng.ts";
import type { DifficultyProfile } from "./difficulty.ts";

interface GenerateResult {
  puzzle: Puzzle;
  solution: Answer[];
}

interface ConstructResult {
  types: QuestionType[];
  solution: Answer[];
  n: number;
  oc: number;
  level: number;
  name: string;
}

import {
  traceConstructFailed,
  traceAttempt,
  traceQuestion,
  tracePhase as tracePhaseImpl,
  traceSuccess,
  traceFailed,
  traceSolve,
  traceHint,
  traceUniqueness,
  traceBatch,
  traceLookahead as traceLookaheadImpl,
  traceRepair,
  traceRepairNoChange,
} from "./trace.ts";

import { formatTypeTag } from "../engine/format.ts";

export function generateConstructive(
  profile: DifficultyProfile,
  rng: RNG,
  maxAttempts = 500,
  tracing = false,
): GenerateResult | null {
  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    const cr = tryConstruct(profile, rng, tracing);
    if (!cr) {
      if (tracing) traceConstructFailed(attempt + 1);
      continue;
    }
    const puzzle = fillOptions(cr, rng);
    if (!puzzle) continue;
    if (tracing) {
      traceAttempt(attempt + 1, cr.solution);
      const oc = cr.oc;
      for (let i = 0; i < cr.n; i++) {
        const opts = puzzle.questions[i].options.slice(0, oc);
        const vals = opts.map((o) => o.value);
        const claims =
          cr.types[i].type === "TrueStmt"
            ? opts.map((o) => ("claim" in o ? o.claim : null))
            : undefined;
        traceQuestion(i, formatTypeTag(cr.types[i]), vals, claims);
      }
    }
    const result = validateAndRepair(puzzle, cr.solution, cr.n, rng, tracing);
    if (result) {
      if (tracing) traceSuccess(attempt + 1);
      return result;
    }
    if (tracing) traceFailed(attempt + 1);
  }
  return null;
}

function hasIdentityOptions(t: QuestionType["type"]): boolean {
  return t === "NoOtherHasAnswer" || t === "AnswerIsSelf";
}

// Rule types by category
const ENTRY_TYPES: QuestionType["type"][] = [
  "CountAnswer",
  "CountAnswerBefore",
  "CountAnswerAfter",
  "CountVowel",
  "CountConsonant",
];

const POSITIONAL_TYPES: QuestionType["type"][] = [
  "FirstWith",
  "LastWith",
  "ClosestAfter",
  "ClosestBefore",
];

const VARIETY_TYPES: QuestionType["type"][] = [
  "LetterDist",
  "ConsecIdent",
  "MostCommonCount",
  "PrevSame",
  "NextSame",
  "OnlySame",
  "SameAs",
  "SameAsWhich",
  "OnlyOdd",
  "OnlyEven",
  "LeastCommon",
  "MostCommon",
  "NoOtherHasAnswer",
  "EqualCount",
  "AnswerIsSelf",
  "TrueStmt",
];

const STRUCTURAL_TYPES = new Set<QuestionType["type"]>([
  "ConsecIdent",
  "NoOtherHasAnswer",
  "OnlySame",
  "OnlyOdd",
  "OnlyEven",
]);

const TYPE_ORDER: Record<string, number> = {
  CountAnswer: 0,
  CountAnswerBefore: 1,
  CountAnswerAfter: 2,
  CountVowel: 3,
  CountConsonant: 4,
  MostCommonCount: 5,
  ClosestAfter: 6,
  ClosestBefore: 7,
  FirstWith: 8,
  LastWith: 9,
  PrevSame: 10,
  NextSame: 11,
  OnlySame: 12,
  SameAs: 13,
  OnlyOdd: 14,
  OnlyEven: 15,
  ConsecIdent: 16,
  AnswerOf: 17,
  LeastCommon: 18,
  MostCommon: 19,
  NoOtherHasAnswer: 20,
  EqualCount: 21,
  AnswerIsSelf: 22,
  LetterDist: 23,
  TrueStmt: 24,
  SameAsWhich: 25,
};

function sortDedup(types: QuestionType["type"][]): QuestionType["type"][] {
  const sorted = [...new Set(types)];
  sorted.sort((a, b) => (TYPE_ORDER[a] ?? 99) - (TYPE_ORDER[b] ?? 99));
  return sorted;
}

function typeCap(type: QuestionType["type"]): number {
  if (type === "LetterDist") return 1;
  if (type === "AnswerOf") return 2;
  return 3;
}

function symmetricGroup(type: QuestionType["type"]): string | null {
  switch (type) {
    case "FirstWith":
    case "LastWith":
      return "first_last";
    case "ClosestAfter":
    case "ClosestBefore":
      return "closest";
    case "NextSame":
    case "PrevSame":
      return "next_prev_same";
    case "CountAnswerBefore":
    case "CountAnswerAfter":
      return "count_before_after";
    case "CountVowel":
    case "CountConsonant":
      return "count_vowel_consonant";
    case "LeastCommon":
    case "MostCommon":
      return "least_most_common";
    default:
      return null;
  }
}

function allowed(
  types: QuestionType["type"][],
  profile: DifficultyProfile,
): QuestionType["type"][] {
  return types.filter((t) => profile.allowedTypes.includes(t));
}

function tryConstruct(
  profile: DifficultyProfile,
  rng: RNG,
  tracing = false,
): ConstructResult | null {
  const n = profile.questionCount;
  const oc = profile.optionCount;
  const validLetters = LETTERS.slice(0, oc);

  // 1. Random solution
  const solution: Answer[] = Array.from({ length: n }, () => rng.pick(validLetters));

  // Bias toward exactly 1 consecutive pair for levels that allow consecutive_identical
  if (profile.allowedTypes.includes("ConsecIdent") && rng.int(0, 1) === 0) {
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
          const nl = rng.pick(validLetters);
          if (nl !== solution[pos - 1] && (pos + 1 >= n || nl !== solution[pos + 1])) {
            solution[pos] = nl;
            break;
          }
        }
      }
    }
  }

  // 2. Shuffle question indices — we'll assign rules in this order
  const slots = rng.shuffle(Array.from({ length: n }, (_, i) => i));

  const rules: (QuestionType | null)[] = new Array(n).fill(null);
  const assigned = new Set<number>(); // questions with rules assigned
  const usedRuleKeys = new Set<string>(); // for dedup (no duplicate question text)

  const avEntry = allowed(ENTRY_TYPES, profile);
  const avPositional = allowed(POSITIONAL_TYPES, profile);
  const avVariety = allowed(VARIETY_TYPES, profile);

  const kindCounts: Record<string, number> = {};
  const groupCounts: Record<string, number> = {};
  const groupCaps: Record<string, number> = {};

  // Vowel/consonant group cap: 1 for L3, 50% 1 / 50% 2 for L4-L5
  if (n <= 8) {
    groupCaps["count_vowel_consonant"] = 1;
  } else if (rng.int(0, 1) === 0) {
    groupCaps["count_vowel_consonant"] = 1;
  } else {
    groupCaps["count_vowel_consonant"] = 2;
  }

  // Count-letter cap: prevent multiple count rules for the same letter
  const countLetterCap = rng.int(0, 3) === 0 ? 2 : 1;
  const countLetterCounts: Record<string, number> = {};
  const COUNT_RULE_TYPES = new Set(["CountAnswer", "CountAnswerBefore", "CountAnswerAfter"]);

  // Variant: 25% of the time for levels with letter_distance,
  // trade letter_distance for a 3rd answer_of chain
  const extraChain = profile.allowedTypes.includes("LetterDist") && rng.int(0, 3) === 0;

  const capsOverride: Record<string, number> = {};
  if (extraChain) {
    capsOverride["AnswerOf"] = 3;
    capsOverride["LetterDist"] = 0;
  }

  function placeRule(type: QuestionType["type"], slotIdx: number): boolean {
    const cap = capsOverride[type] ?? typeCap(type);
    if ((kindCounts[type] ?? 0) >= cap) return false;
    const group = symmetricGroup(type);
    if (group !== null && (groupCounts[group] ?? 0) >= (groupCaps[group] ?? 3)) return false;
    const qi = slots[slotIdx];
    if (!solutionCompatible(type, qi, solution, n, oc)) return false;
    for (let attempt = 0; attempt < 10; attempt++) {
      const rule = makeRule(type, qi, n, oc, solution, assigned, rng);
      if (!rule) continue;
      if (COUNT_RULE_TYPES.has(rule.type) && "answer" in rule) {
        const letter = rule.answer as string;
        if ((countLetterCounts[letter] ?? 0) >= countLetterCap) continue;
      }
      const key = JSON.stringify(rule);
      if (usedRuleKeys.has(key)) continue;
      if (!checkStructural(rule, qi, solution)) continue;
      if (COUNT_RULE_TYPES.has(rule.type) && "answer" in rule) {
        const letter = rule.answer as string;
        countLetterCounts[letter] = (countLetterCounts[letter] ?? 0) + 1;
      }
      rules[qi] = rule;
      assigned.add(qi);
      usedRuleKeys.add(key);
      kindCounts[type] = (kindCounts[type] ?? 0) + 1;
      if (group !== null) groupCounts[group] = (groupCounts[group] ?? 0) + 1;
      return true;
    }
    return false;
  }

  function placeFrom(types: QuestionType["type"][]): boolean {
    if (types.length === 0 || assigned.size >= n) return false;
    return placeRule(rng.pick(types), assigned.size);
  }

  function tracePhase(name: string): void {
    if (!tracing) return;
    const placed = slots
      .filter((qi) => assigned.has(qi))
      .sort((a, b) => a - b)
      .map((qi) => ({ qi, type: formatTypeTag(rules[qi]!) }));
    tracePhaseImpl(name, placed);
  }

  // Phase 1: Counting entry point (skip 50% for small puzzles)
  const skipCounting = n <= 3 && rng.int(0, 1) === 0;
  if (!skipCounting) {
    if (avEntry.length === 0 || !placeFrom(avEntry)) return null;
  }

  tracePhase("p1");

  // Phase 2: answer_of_question backbone
  const chainCount = Math.min(
    extraChain ? 3 : n <= 3 ? (rng.int(0, 1) === 0 ? 1 : 0) : n <= 5 && rng.int(0, 1) === 0 ? 1 : 2,
    n - assigned.size,
  );
  for (let c = 0; c < chainCount; c++) {
    if (!placeRule("AnswerOf", assigned.size)) return null;
  }

  tracePhase("p2");

  // Phase 3: occasionally place prev/next_same (need specific slot positions)
  const p2bCheck = rng.int(0, 1);
  if (p2bCheck === 0 && assigned.size < n) {
    const candidates: [QuestionType["type"], number][] = [
      ["PrevSame", n - 1],
      ["NextSame", 0],
    ];
    const candIdx = rng.int(0, 1);
    const [kind, neededQi] = candidates[candIdx];
    if (profile.allowedTypes.includes(kind) && !assigned.has(neededQi)) {
      const idx = slots.indexOf(neededQi, assigned.size);
      if (idx >= 0) {
        [slots[assigned.size], slots[idx]] = [slots[idx], slots[assigned.size]];
        placeRule(kind, assigned.size);
      }
    }
  }

  tracePhase("p3");

  // Phase 4: Positional types (skip for tiny puzzles)
  const posCount = Math.min(
    avPositional.length > 0 && n > 3 ? Math.max(2, Math.floor(n / 5)) : 0,
    n - assigned.size,
  );
  for (let p = 0; p < posCount; p++) placeFrom(avPositional);

  tracePhase("p4");

  // Phase 5: Exotic guaranteed types
  const exoticSlots: QuestionType["type"][] = [];
  if (allowed(["LetterDist"], profile).length > 0) exoticSlots.push("LetterDist");
  if (allowed(["TrueStmt"], profile).length > 0) exoticSlots.push("TrueStmt");
  if (allowed(["ConsecIdent"], profile).length > 0) exoticSlots.push("ConsecIdent");
  for (const type of exoticSlots) {
    if (assigned.size >= n) break;
    placeRule(type, assigned.size);
  }

  tracePhase("p5");

  // Phase 6: Fill remaining, reserving slots for structural types
  const avStructural = avVariety.filter((t) => STRUCTURAL_TYPES.has(t));
  const structuralReserve = Math.min(avStructural.length > 0 ? 1 : 0, n - assigned.size);
  const fillTarget = n - structuralReserve;

  const fillPool = sortDedup(
    [...avEntry, ...avPositional, ...avVariety].filter(
      (t) => profile.allowedTypes.includes(t) && t !== "AnswerOf",
    ),
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
      if (!placeRule("AnswerOf", assigned.size) && !placeRule("AnswerIsSelf", assigned.size))
        return null;
    }
  }

  tracePhase("p6");

  // Phase 7: Structural types (need specific solution properties)
  for (let s = 0; s < structuralReserve && assigned.size < n; s++) {
    const qi = slots[assigned.size];
    const fitting = rng.shuffle(
      avStructural.filter((t) => solutionCompatible(t, qi, solution, n, oc)),
    );
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
        !placeRule("AnswerOf", assigned.size) &&
        !placeRule("AnswerIsSelf", assigned.size)
      )
        return null;
    }
  }

  tracePhase("p7");

  const finalRules: QuestionType[] = rules.filter((t): t is QuestionType => t !== null);
  return { types: finalRules, solution, n, oc, level: profile.level, name: profile.name };
}

function fillOptions(cr: ConstructResult, rng: RNG): Puzzle | null {
  const questions: QuestionDef[] = [];
  for (let i = 0; i < cr.types.length; i++) {
    const options = engineerOptions(cr.types[i], i, cr.solution, cr.n, cr.oc, rng);
    if (!options) return null;
    questions.push({ options, questionType: cr.types[i] });
  }
  return {
    id: `level-${String(cr.level)}`,
    title: cr.name,
    difficulty: String(cr.level),
    questions,
    optionCount: cr.oc,
  };
}

function validateAndRepair(
  puzzle: Puzzle,
  solution: Answer[],
  n: number,
  rng: RNG,
  tracing = false,
): GenerateResult | null {
  const formErrors = checkForm(puzzle, solution);
  if (formErrors.length > 0) {
    if (tracing) {
      for (const e of formErrors) console.error(`FORM ERROR Q${String(e.qi + 1)}: ${e.message}`);
    }
    return null;
  }

  const fp = flattenPuzzle(puzzle);
  if (!checkAnswers(fp, solution)) return null;

  // Step 1: Can the hint engine solve it?
  if (tracing) traceSolve("initial");
  const stuckState = runHintEngine(puzzle, n, tracing);
  if (tracing) {
    const answered = stuckState.answers.filter((a) => a != null).length;
    traceHint(stuckState.solved, answered, n);
  }
  if (stuckState.solved) {
    const solutions = solve(puzzle, undefined, 2);
    if (tracing) traceUniqueness(solutions.length);
    if (solutions.length === 1) return { puzzle, solution: solutions[0] };
    return null;
  }

  // Step 3: Repair — tweak candidates cumulatively (no revert, matching Rust)
  const candidates = rankRepairCandidates(puzzle, stuckState.answers, n);
  const answeredBefore = stuckState.answers.filter((a) => a != null).length;

  for (const qi of candidates) {
    const before = puzzle.questions[qi].options.map((o) => o.value);
    repairOneQuestion(puzzle, qi, solution, stuckState.eliminated, rng);

    let changed = false;
    for (let i = 0; i < before.length; i++) {
      if (puzzle.questions[qi].options[i].value !== before[i]) {
        changed = true;
        break;
      }
    }
    if (!changed) {
      if (tracing) traceRepairNoChange(qi);
      continue;
    }

    const fp2 = flattenPuzzle(puzzle);
    const probe = deduce(fp2, stuckState);
    if (tracing) {
      const after = puzzle.questions[qi].options.map((o) => o.value);
      traceRepair(qi, before, after, probe.length);
    }
    if (probe.length === 0) continue;

    if (tracing) traceSolve("after_repair");
    const solvedAfterRepair =
      answeredBefore > 0
        ? runHintEngineFrom(puzzle, n, stuckState, tracing).solved
        : runHintEngine(puzzle, n, tracing).solved;
    if (solvedAfterRepair) break;
  }

  if (tracing) traceSolve("final");
  if (!runHintEngine(puzzle, n, tracing).solved) return null;

  const solutions = solve(puzzle, undefined, 2);
  if (tracing) traceUniqueness(solutions.length);
  if (solutions.length === 1) return { puzzle, solution: solutions[0] };

  return null;
}

function runHintEngineFrom(
  puzzle: Puzzle,
  n: number,
  initState: State,
  tracing = false,
): { solved: boolean; answers: (Answer | null)[]; eliminated: number[] } {
  return runHintEngineImpl(puzzle, n, initState.answers.slice(0, n), initState.eliminated.slice(0, n), tracing);
}

function runHintEngine(
  puzzle: Puzzle,
  n: number,
  tracing = false,
): { solved: boolean; answers: (Answer | null)[]; eliminated: number[] } {
  const fp = flattenPuzzle(puzzle);
  const phantomMask = 0b11111 & ~((1 << fp.optionCount) - 1);
  return runHintEngineImpl(
    puzzle,
    n,
    new Array(n).fill(null),
    new Array(n).fill(phantomMask),
    tracing,
  );
}

function runHintEngineImpl(
  puzzle: Puzzle,
  n: number,
  answers: (Answer | null)[],
  eliminated: number[],
  tracing: boolean,
): { solved: boolean; answers: (Answer | null)[]; eliminated: number[] } {
  const fp = flattenPuzzle(puzzle);
  const state: State = { answers, eliminated };
  let batch = 0;
  for (let step = 0; step < n * 15; step++) {
    if (state.answers.every((a) => a != null)) return { solved: true, answers: state.answers, eliminated: state.eliminated };
    const drs = deduce(fp, state);
    if (drs.length > 0) {
      if (tracing) traceBatch(batch, drs);
      batch++;
      for (const dr of drs) applyDeduceAction(dr.action, state.answers, state.eliminated);
      continue;
    }
    if (fp.optionCount < 5) return { solved: false, answers: state.answers, eliminated: state.eliminated };
    const lr = lookahead(fp, state, 6, true);
    if (lr) {
      if (tracing) traceLookaheadImpl(lr);
      state.eliminated[lr.eliminateQi] |= 1 << lr.eliminateOi;
      continue;
    }
    return { solved: false, answers: state.answers, eliminated: state.eliminated };
  }
  return { solved: false, answers: state.answers, eliminated: state.eliminated };
}

function rankRepairCandidates(
  puzzle: Puzzle,
  stuckAnswers: (Answer | null)[],
  n: number,
): number[] {
  const scored: [number, number][] = [];
  for (let qi = 0; qi < n; qi++) {
    if (stuckAnswers[qi] != null) continue;
    const rule = puzzle.questions[qi].questionType;
    if (hasIdentityOptions(rule.type) || rule.type === "TrueStmt") continue;
    let score: number;
    if (isCountingType(rule.type)) {
      score = 3;
    } else if (rule.type === "AnswerOf" || rule.type === "LetterDist") {
      score = stuckAnswers[rule.questionIndex] != null ? 2 : 0;
    } else {
      score = 1;
    }
    if (score > 0) scored.push([qi, score]);
  }
  scored.sort((a, b) => b[1] - a[1]);
  return scored.map(([qi]) => qi);
}

function isCountingType(type: string): boolean {
  return [
    "CountAnswer",
    "CountAnswerBefore",
    "CountAnswerAfter",
    "CountVowel",
    "CountConsonant",
    "MostCommonCount",
  ].includes(type);
}

function repairOneQuestion(
  puzzle: Puzzle,
  qi: number,
  solution: Answer[],
  stuckElim: number[],
  rng: RNG,
): void {
  const n = puzzle.questions.length;
  const oc = puzzle.optionCount ?? 5;
  const rule = puzzle.questions[qi].questionType;
  const correctOi = L2I[solution[qi]];
  const elim = stuckElim[qi];
  const opts = puzzle.questions[qi].options;

  if (rule.type === "AnswerOf") {
    const correctAnswer = solution[rule.questionIndex];
    const pool = rng.shuffle(LETTERS.slice(0, oc).filter((l) => l !== correctAnswer));
    let di = 0;
    for (let oi = 0; oi < oc; oi++) {
      if (oi !== correctOi && ((elim >> oi) & 1) === 0 && di < pool.length) {
        opts[oi] = { value: L2I[pool[di++]] };
      }
    }
    return;
  }

  const correctVal = opts[correctOi].value;

  if (
    rule.type === "LetterDist" ||
    rule.type === "LeastCommon" ||
    rule.type === "MostCommon" ||
    rule.type === "NoOtherHasAnswer"
  ) {
    replaceClosestWithFurthest(opts, correctOi, correctVal, elim, oc, 0, 4);
    return;
  }

  if (isCountingType(rule.type)) {
    const vals = validValues(rule, n);
    const max = vals.length > 0 ? (vals[vals.length - 1] ?? 0) : 0;
    replaceClosestWithFurthest(opts, correctOi, correctVal, elim, oc, 0, max);
    return;
  }

  if (rule.type === "SameAsWhich") {
    const refAns = solution[rule.questionIndex];
    const bestOi = findClosestOption(opts, correctOi, correctVal, elim, oc);
    if (bestOi == null) return;
    const oldVal = opts[bestOi].value;
    let bestNew = oldVal;
    let bestDist = 0;
    for (let j = 0; j < n; j++) {
      if (j === qi || j === rule.questionIndex || solution[j] === refAns) continue;
      if (j === correctVal || j === oldVal) continue;
      if (isInUse(opts, bestOi, j, oc)) continue;
      const d = absDiff(j, correctVal);
      if (d > bestDist) {
        bestDist = d;
        bestNew = j;
      }
    }
    opts[bestOi] = { value: bestNew };
    return;
  }

  // General case: positional, ConsecIdent, OnlyOdd/Even, EqualCount, SameAs, etc.
  const bestOi = findClosestOption(opts, correctOi, correctVal, elim, oc, 1);
  if (bestOi == null) return;

  let minVal = 0;
  let maxVal = n - 1;
  let step = 1;
  const excludeSelf = rule.type === "OnlySame" || rule.type === "SameAs";
  let excludeRef = -2;

  if (rule.type === "ConsecIdent") {
    maxVal = Math.max(n - 2, 0);
  } else if (rule.type === "PrevSame") {
    maxVal = qi - 1;
  } else if (rule.type === "NextSame") {
    minVal = qi + 1;
  } else if (rule.type === "OnlyOdd") {
    minVal = 0;
    step = 2;
  } else if (rule.type === "OnlyEven") {
    minVal = 1;
    step = 2;
  } else if (rule.type === "EqualCount") {
    minVal = 0;
    maxVal = 4;
    excludeRef = L2I[rule.answer];
  } else if (rule.type === "ClosestAfter") {
    minVal = rule.afterIndex + 1;
  } else if (rule.type === "ClosestBefore") {
    maxVal = rule.beforeIndex - 1;
  }

  const oldVal = opts[bestOi].value;
  let bestNew = oldVal;
  let bestDist = 0;

  // Try all values in range + null (None)
  const candidates: (number | null)[] = [];
  for (let v = minVal; v <= maxVal; v += step) candidates.push(v);
  candidates.push(null);

  for (const v of candidates) {
    if (v === correctVal || v === oldVal) continue;
    if (excludeSelf && v === qi) continue;
    if (v === excludeRef) continue;
    if (isInUse(opts, bestOi, v, oc)) continue;
    let d: number;
    if (v == null || correctVal == null) {
      d = maxVal + 1;
    } else {
      d = absDiff(v, correctVal);
    }
    if (d > bestDist) {
      bestDist = d;
      bestNew = v;
    }
  }
  opts[bestOi] = { value: bestNew };
}

function absDiff(a: number | null, b: number | null): number {
  return Math.abs((a ?? -1) - (b ?? -1));
}

function findClosestOption(
  opts: OptionDef[],
  correctOi: number,
  correctVal: number | null,
  elim: number,
  oc: number,
  nullDist = -1,
): number | null {
  let bestOi: number | null = null;
  let bestDist = Infinity;
  for (let oi = 0; oi < oc; oi++) {
    if (oi === correctOi || ((elim >> oi) & 1) !== 0) continue;
    const v = opts[oi].value;
    const dist =
      nullDist >= 0 && (v == null || correctVal == null) ? nullDist : absDiff(v, correctVal);
    if (dist < bestDist) {
      bestDist = dist;
      bestOi = oi;
    }
  }
  return bestOi;
}

function isInUse(opts: OptionDef[], skipOi: number, value: number | null, oc: number): boolean {
  for (let k = 0; k < oc; k++) {
    if (k !== skipOi && opts[k].value === value) return true;
  }
  return false;
}

function replaceClosestWithFurthest(
  opts: OptionDef[],
  correctOi: number,
  correctVal: number | null,
  elim: number,
  oc: number,
  minRange: number,
  maxRange: number,
): void {
  const bestOi = findClosestOption(opts, correctOi, correctVal, elim, oc);
  if (bestOi == null) return;
  const oldVal = opts[bestOi].value;
  let bestNew = oldVal;
  let bestDist = 0;
  for (let v = minRange; v <= maxRange; v++) {
    if (v === correctVal || v === oldVal) continue;
    if (isInUse(opts, bestOi, v, oc)) continue;
    const d = absDiff(v, correctVal);
    if (d > bestDist) {
      bestDist = d;
      bestNew = v;
    }
  }
  opts[bestOi] = { value: bestNew };
}

function applyDeduceAction(action: DeduceAction, answers: (Answer | null)[], eliminated: number[]) {
  if (action.type === "force") {
    const oi = L2I[action.answer];
    eliminated[action.qi] = 0b11111 ^ (1 << oi);
    answers[action.qi] = action.answer;
  } else if (action.type === "eliminateMulti") {
    for (let i = 0; i < eliminated.length; i++) {
      if ((action.questionMask >> i) & 1) eliminated[i] |= action.optionMask;
    }
  } else if (action.type === "eliminate") {
    eliminated[action.qi] |= 1 << action.oi;
  }
}

// ── Rule factory ──

function makeRule(
  type: QuestionType["type"],
  qi: number,
  n: number,
  oc: number,
  solution: Answer[],
  assigned: Set<number>,
  rng: RNG,
): QuestionType | null {
  const validLetters = LETTERS.slice(0, oc);
  switch (type) {
    case "CountAnswer":
      return { type, answer: rng.pick(validLetters) };
    case "CountAnswerBefore":
      if (n < 6) return null;
      return {
        type,
        answer: rng.pick(validLetters),
        beforeIndex: rng.int(4, n - 1),
      };
    case "CountAnswerAfter":
      if (n < 6) return null;
      return {
        type,
        answer: rng.pick(validLetters),
        afterIndex: rng.int(0, Math.max(0, n - 5)),
      };
    case "CountVowel":
    case "CountConsonant":
    case "MostCommonCount":
      return { type };
    case "AnswerOf": {
      const targets = [...assigned].filter((j) => j !== qi).sort((a, b) => a - b);
      if (targets.length === 0) return null;
      return { type, questionIndex: rng.pick(targets) };
    }
    case "LetterDist": {
      const targets = [...assigned].filter((j) => j !== qi).sort((a, b) => a - b);
      if (targets.length === 0) {
        const pool = [];
        for (let j = 0; j < n; j++) if (j !== qi) pool.push(j);
        return { type, questionIndex: rng.pick(pool) };
      }
      return { type, questionIndex: rng.pick(targets) };
    }
    case "ClosestAfter":
      return {
        type,
        afterIndex: rng.int(0, Math.max(0, n - 5)),
        answer: rng.pick(validLetters),
      };
    case "ClosestBefore":
      if (n < 5) return null;
      return {
        type,
        beforeIndex: rng.int(4, n - 1),
        answer: rng.pick(validLetters),
      };
    case "FirstWith":
    case "LastWith":
      return { type, answer: rng.pick(validLetters) };
    case "PrevSame":
      if (qi < 4) return null;
      return { type };
    case "NextSame":
      if (qi + 5 > n) return null;
      return { type };
    case "OnlySame":
    case "SameAs":
    case "ConsecIdent":
    case "LeastCommon":
    case "MostCommon":
    case "NoOtherHasAnswer":
      return { type };
    case "EqualCount": {
      const refLetter = rng.pick(validLetters);
      const refCount = solution.filter((a) => a === refLetter).length;
      const hasMatch = validLetters.some(
        (l) => l !== refLetter && solution.filter((a) => a === l).length === refCount,
      );
      if (!hasMatch && rng.int(0, 4) > 1) return null;
      return { type, answer: refLetter };
    }
    case "OnlyOdd":
    case "OnlyEven":
      return { type, answer: rng.pick(validLetters) };
    case "AnswerIsSelf":
      return { type };
    case "TrueStmt":
      return { type };
    case "SameAsWhich": {
      const targets = [...assigned].filter((j) => j !== qi).sort((a, b) => a - b);
      if (targets.length === 0) return null;
      const ref = rng.pick(targets);
      if (solution[ref] === solution[qi]) return null;
      for (let j = 0; j < n; j++) {
        if (j !== qi && j !== ref && solution[j] === solution[ref])
          return { type, questionIndex: ref };
      }
      return null;
    }
  }
  return null;
}

// ── Structural checks ──

function checkStructural(rule: QuestionType, qi: number, sol: Answer[]): boolean {
  switch (rule.type) {
    case "OnlySame": {
      let m = 0;
      for (let i = 0; i < sol.length; i++) if (i !== qi && sol[i] === sol[qi]) m++;
      return m === 1;
    }
    case "ConsecIdent": {
      let p = 0;
      for (let i = 0; i < sol.length - 1; i++) if (sol[i] === sol[i + 1]) p++;
      return p === 1;
    }
    case "OnlyOdd":
    case "OnlyEven": {
      const parity = rule.type === "OnlyOdd" ? 1 : 0;
      let m = 0;
      for (let i = 0; i < sol.length; i++)
        if ((i + 1) % 2 === parity && sol[i] === rule.answer) m++;
      return m === 1;
    }
    case "NoOtherHasAnswer":
      return sol.filter((a) => a === sol[qi]).length === 1;
    default:
      return true;
  }
}

function solutionHasStructural(
  type: QuestionType["type"],
  qi: number,
  solution: Answer[],
  n: number,
): boolean {
  switch (type) {
    case "ConsecIdent": {
      let pairs = 0;
      for (let i = 0; i < n - 1; i++) if (solution[i] === solution[i + 1]) pairs++;
      return pairs === 1;
    }
    case "NoOtherHasAnswer":
      return solution.slice(0, n).filter((a) => a === solution[qi]).length === 1;
    case "OnlySame": {
      let m = 0;
      for (let i = 0; i < n; i++) if (i !== qi && solution[i] === solution[qi]) m++;
      return m === 1;
    }
    case "OnlyOdd":
    case "OnlyEven": {
      const parity = type === "OnlyOdd" ? 1 : 0;
      for (const letter of LETTERS) {
        let m = 0;
        for (let i = 0; i < n; i++) if ((i + 1) % 2 === parity && solution[i] === letter) m++;
        if (m === 1) return true;
      }
      return false;
    }
  }
  return false;
}

function solutionCompatible(
  type: QuestionType["type"],
  qi: number,
  solution: Answer[],
  n: number,
  oc: number,
): boolean {
  switch (type) {
    case "LeastCommon": {
      const c = letterCounts(solution.slice(0, n)).slice(0, oc);
      return c.filter((v) => v === Math.min(...c)).length === 1;
    }
    case "MostCommon": {
      const c = letterCounts(solution.slice(0, n)).slice(0, oc);
      return c.filter((v) => v === Math.max(...c)).length === 1;
    }
    case "SameAs": {
      if (n <= oc) return false;
      for (let i = 0; i < n; i++) if (i !== qi && solution[i] === solution[qi]) return true;
      return false;
    }
    case "EqualCount":
      return true;
  }
  if (STRUCTURAL_TYPES.has(type)) return solutionHasStructural(type, qi, solution, n);
  return true;
}

// ── Options, text, distractors ──

function letterCounts(sol: Answer[]): number[] {
  const c = [0, 0, 0, 0, 0];
  for (const a of sol) c[L2I[a]]++;
  return c;
}

function rangeWithNull(len: number, map: (i: number) => number = (i) => i): (number | null)[] {
  const v: (number | null)[] = [];
  for (let i = 0; i < len; i++) v.push(map(i));
  v.push(null);
  return v;
}

function validValues(rule: QuestionType, n: number): (number | null)[] {
  switch (rule.type) {
    case "CountAnswer":
    case "CountVowel":
    case "CountConsonant":
    case "MostCommonCount":
      return Array.from({ length: n + 1 }, (_, i) => i);
    case "CountAnswerBefore":
      return Array.from({ length: rule.beforeIndex + 1 }, (_, i) => i);
    case "CountAnswerAfter":
      return Array.from({ length: n - rule.afterIndex }, (_, i) => i);
    case "AnswerOf":
    case "LeastCommon":
    case "MostCommon":
    case "NoOtherHasAnswer":
    case "LetterDist":
    case "EqualCount":
      return [0, 1, 2, 3, 4];
    case "ClosestAfter":
      return rangeWithNull(n - rule.afterIndex - 1, (i) => i + rule.afterIndex + 1);
    case "ClosestBefore":
      return rangeWithNull(rule.beforeIndex);
    case "OnlyOdd":
      return rangeWithNull(Math.ceil(n / 2), (i) => i * 2);
    case "OnlyEven":
      return rangeWithNull(Math.floor(n / 2), (i) => i * 2 + 1);
    case "ConsecIdent":
      return rangeWithNull(n - 1);
    case "SameAs":
      return Array.from({ length: n }, (_, i) => i);
    default:
      return rangeWithNull(n);
  }
}

function pickDistractors(
  vals: (number | null)[],
  correct: number | null,
  qi: number,
  rule: QuestionType,
  rng: RNG,
): (number | null)[] {
  const excludeSelf = rule.type === "OnlySame" || rule.type === "SameAs";
  const pool: (number | null)[] = [];
  for (const v of vals) {
    if (v !== correct && !(excludeSelf && v === qi)) pool.push(v);
  }
  const shuffled = rng.shuffle(pool);
  return shuffled.slice(0, 4);
}

function engineerOptions(
  rule: QuestionType,
  qi: number,
  solution: Answer[],
  n: number,
  oc: number,
  rng: RNG,
): OptionDef[] | null {
  const correctOi = L2I[solution[qi]];

  if (hasIdentityOptions(rule.type)) {
    // Can't place NoOtherHasAnswer when another question already shares this answer.
    if (rule.type === "NoOtherHasAnswer") {
      const selfAns = solution[qi];
      if (solution.slice(0, n).some((a, j) => j !== qi && a === selfAns)) return null;
    }
    return Array.from({ length: oc }, (_, i) => ({ value: i }));
  }

  if (rule.type === "TrueStmt") {
    return buildClaims(qi, solution, n, oc, rng).slice(0, oc);
  }

  const letters = LETTERS.slice(0, oc);

  if (rule.type === "AnswerOf") {
    const correctAnswer = solution[rule.questionIndex];
    const pool = rng.shuffle(letters.filter((l) => l !== correctAnswer));
    const opts: OptionDef[] = new Array(oc);
    opts[correctOi] = { value: L2I[correctAnswer] };
    let di = 0;
    for (let i = 0; i < oc; i++) if (i !== correctOi) opts[i] = { value: L2I[pool[di++]] };
    return opts;
  }

  if (rule.type === "LeastCommon" || rule.type === "MostCommon") {
    const counts = letterCounts(solution.slice(0, n));
    const target =
      rule.type === "LeastCommon"
        ? Math.min(...counts.slice(0, oc))
        : Math.max(...counts.slice(0, oc));
    // Can't place MostCommon/LeastCommon when two letters tie for the extreme count.
    if (counts.slice(0, oc).filter((c) => c === target).length !== 1) return null;
    const correctLetter = LETTERS.findIndex((_, i) => i < oc && counts[i] === target);
    const pool = rng.shuffle(letters.filter((_, i) => i !== correctLetter));
    const opts: OptionDef[] = new Array(oc);
    opts[correctOi] = { value: correctLetter };
    let di = 0;
    for (let i = 0; i < oc; i++) if (i !== correctOi) opts[i] = { value: L2I[pool[di++]] };
    return opts;
  }

  if (rule.type === "EqualCount") {
    const correct = computeValue(rule, qi, solution);
    const pool: (number | null)[] = [];
    for (let i = 0; i < oc; i++) {
      if (LETTERS[i] !== rule.answer && i !== correct) pool.push(i);
    }
    if (correct != null) pool.push(null);
    const shuffled = rng.shuffle(pool);
    const opts: OptionDef[] = new Array(oc);
    opts[correctOi] = { value: correct };
    let di = 0;
    for (let i = 0; i < oc; i++) if (i !== correctOi) opts[i] = { value: shuffled[di++] };
    return opts;
  }

  if (rule.type === "SameAs") {
    // Can't place SameAs when no other question shares this answer.
    const correct = computeValue(rule, qi, solution);
    if (correct === null) return null;
    const pool: number[] = [];
    for (let j = 0; j < n; j++) {
      if (j !== qi && j !== correct && solution[j] !== solution[qi]) pool.push(j);
    }
    const shuffled = rng.shuffle(pool);
    const opts: OptionDef[] = new Array(oc);
    opts[correctOi] = { value: correct };
    let di = 0;
    for (let i = 0; i < oc; i++) if (i !== correctOi) opts[i] = { value: shuffled[di++] ?? null };
    return opts;
  }

  if (rule.type === "SameAsWhich") {
    // Can't place SameAsWhich when no other question shares the referenced answer.
    const correct = computeValue(rule, qi, solution);
    if (correct === null) return null;
    const refAns = solution[rule.questionIndex];
    const pool: number[] = [];
    for (let j = 0; j < n; j++) {
      if (j !== qi && j !== rule.questionIndex && solution[j] !== refAns) pool.push(j);
    }
    const shuffled = rng.shuffle(pool);
    const distractors = shuffled.slice(0, 4);
    const opts: OptionDef[] = new Array(oc);
    opts[correctOi] = { value: correct };
    let di = 0;
    for (let i = 0; i < oc; i++)
      if (i !== correctOi) opts[i] = { value: distractors[di++] ?? null };
    return opts;
  }

  if (rule.type === "OnlyOdd" || rule.type === "OnlyEven") {
    // Can't place OnlyOdd/OnlyEven when more than one same-parity question has this answer.
    const parity = rule.type === "OnlyOdd" ? 1 : 0;
    const matches = solution
      .slice(0, n)
      .filter((a, i) => (i + 1) % 2 === parity && a === rule.answer).length;
    if (matches > 1) return null;
  }

  if (rule.type === "OnlySame") {
    // Can't place OnlySame when more than one other question shares this answer.
    const others = solution.slice(0, n).filter((a, j) => j !== qi && a === solution[qi]).length;
    if (others > 1) return null;
  }

  if (rule.type === "ConsecIdent") {
    // Can't place ConsecIdent when more than one consecutive identical pair exists.
    let pairs = 0;
    for (let i = 0; i < n - 1; i++) if (solution[i] === solution[i + 1]) pairs++;
    if (pairs > 1) return null;
  }

  const correct = computeValue(rule, qi, solution);
  const vals = validValues(rule, n);
  const distractors = pickDistractors(vals, correct, qi, rule, rng);
  const opts: OptionDef[] = new Array(oc);
  opts[correctOi] = { value: correct };
  let di = 0;
  for (let i = 0; i < oc; i++) if (i !== correctOi) opts[i] = { value: distractors[di++] ?? null };
  return opts;
}

function computeValue(rule: QuestionType, qi: number, sol: Answer[]): number | null {
  switch (rule.type) {
    case "AnswerOf":
      return L2I[sol[rule.questionIndex]];
    case "CountAnswer":
      return sol.filter((a) => a === rule.answer).length;
    case "CountAnswerBefore":
      return sol.slice(0, rule.beforeIndex).filter((a) => a === rule.answer).length;
    case "CountAnswerAfter":
      return sol.slice(rule.afterIndex + 1).filter((a) => a === rule.answer).length;
    case "CountVowel":
      return sol.filter((a) => a === "A" || a === "E").length;
    case "CountConsonant":
      return sol.filter((a) => a !== "A" && a !== "E").length;
    case "MostCommonCount":
      return Math.max(...letterCounts(sol));
    case "ClosestAfter":
      for (let i = rule.afterIndex + 1; i < sol.length; i++) if (sol[i] === rule.answer) return i;
      return null;
    case "ClosestBefore":
      for (let i = rule.beforeIndex - 1; i >= 0; i--) if (sol[i] === rule.answer) return i;
      return null;
    case "FirstWith":
      for (let i = 0; i < sol.length; i++) if (sol[i] === rule.answer) return i;
      return null;
    case "LastWith":
      for (let i = sol.length - 1; i >= 0; i--) if (sol[i] === rule.answer) return i;
      return null;
    case "PrevSame":
      for (let i = qi - 1; i >= 0; i--) if (sol[i] === sol[qi]) return i;
      return null;
    case "NextSame":
      for (let i = qi + 1; i < sol.length; i++) if (sol[i] === sol[qi]) return i;
      return null;
    case "OnlySame":
      for (let i = 0; i < sol.length; i++) if (i !== qi && sol[i] === sol[qi]) return i;
      return null;
    case "SameAs":
      for (let i = 0; i < sol.length; i++) if (i !== qi && sol[i] === sol[qi]) return i;
      return null;
    case "OnlyOdd":
    case "OnlyEven": {
      const parity = rule.type === "OnlyOdd" ? 1 : 0;
      for (let i = 0; i < sol.length; i++)
        if ((i + 1) % 2 === parity && sol[i] === rule.answer) return i;
      return null;
    }
    case "ConsecIdent":
      for (let i = 0; i < sol.length - 1; i++) if (sol[i] === sol[i + 1]) return i;
      return null;
    case "EqualCount": {
      const refCount = sol.filter((a) => a === rule.answer).length;
      for (const l of LETTERS) {
        if (l !== rule.answer && sol.filter((a) => a === l).length === refCount) return L2I[l];
      }
      return null;
    }
    case "LetterDist":
      return Math.abs(L2I[sol[qi]] - L2I[sol[rule.questionIndex]]);
    case "SameAsWhich": {
      const refAns = sol[rule.questionIndex];
      for (let i = 0; i < sol.length; i++) {
        if (i !== qi && i !== rule.questionIndex && sol[i] === refAns) return i;
      }
      return null;
    }
  }
  throw new Error(`computeValue: ${rule.type}`);
}

function buildClaims(
  qi: number,
  solution: Answer[],
  n: number,
  optionCount: number,
  rng: RNG,
): StatementOption[] {
  const targetIdx = L2I[solution[qi]];
  const options: StatementOption[] = new Array(5);
  const trueClaim = makeTrueClaim(solution, qi, n, optionCount, rng);
  options[targetIdx] = { value: null, claim: trueClaim };
  const usedKeys = new Set([claimCategory(trueClaim)]);
  for (let i = 0; i < 5; i++) {
    if (i === targetIdx) continue;
    for (let att = 0; att < 30; att++) {
      const fc = makeFalseClaim(solution, qi, n, optionCount, rng);
      const key = claimCategory(fc);
      if (!usedKeys.has(key)) {
        usedKeys.add(key);
        options[i] = { value: null, claim: fc };
        break;
      }
    }
    if (!options[i]) {
      const fc = makeFalseClaim(solution, qi, n, optionCount, rng);
      options[i] = { value: null, claim: fc };
    }
  }
  return options;
}
type ClaimGen = (sol: Answer[], qi: number, n: number, rng: RNG) => Claim | null;

const CLAIM_GENS: ClaimGen[] = [
  (sol, _qi, _n, rng) => {
    const a = rng.pick(LETTERS);
    return {
      questionType: { type: "CountAnswer", answer: a },
      value: sol.filter((x) => x === a).length,
    };
  },
  (sol) => ({
    questionType: { type: "CountConsonant" },
    value: sol.filter((a) => a !== "A" && a !== "E").length,
  }),
  (sol) => ({
    questionType: { type: "CountVowel" },
    value: sol.filter((a) => a === "A" || a === "E").length,
  }),
  (sol, _qi, n, rng) => {
    const a = rng.pick(LETTERS);
    const ai = rng.int(0, Math.max(0, n - 5));
    return {
      questionType: { type: "CountAnswerAfter", answer: a, afterIndex: ai },
      value: sol.slice(ai + 1).filter((x) => x === a).length,
    };
  },
  (sol, _qi, n, rng) => {
    const a = rng.pick(LETTERS);
    const bi = rng.int(4, n - 1);
    return {
      questionType: { type: "CountAnswerBefore", answer: a, beforeIndex: bi },
      value: sol.slice(0, bi).filter((x) => x === a).length,
    };
  },
  (sol, _qi, n, rng) => {
    const target = rng.int(0, n - 1);
    return {
      questionType: { type: "AnswerOf", questionIndex: target },
      value: L2I[sol[target]],
    };
  },
  (sol, _qi, _n, rng) => {
    const a = rng.pick(LETTERS);
    const first = sol.indexOf(a);
    if (first < 0) return null;
    return { questionType: { type: "FirstWith", answer: a }, value: first };
  },
  (sol, _qi, _n, rng) => {
    const a = rng.pick(LETTERS);
    const last = sol.lastIndexOf(a);
    if (last < 0) return null;
    return { questionType: { type: "LastWith", answer: a }, value: last };
  },
  (sol) => {
    const counts = [0, 0, 0, 0, 0];
    for (const a of sol) counts[L2I[a]] += 1;
    const max = Math.max(...counts);
    if (counts.filter((c) => c === max).length !== 1) return null;
    const idx = counts.indexOf(max);
    return { questionType: { type: "MostCommon" }, value: idx };
  },
  // Newly enabled claim shapes
  (sol, _qi, n, rng) => {
    const a = rng.pick(LETTERS);
    const ai = rng.int(0, Math.max(0, n - 2));
    let target = -1;
    for (let i = ai + 1; i < sol.length; i++) {
      if (sol[i] === a) {
        target = i;
        break;
      }
    }
    if (target < 0) return null;
    return {
      questionType: { type: "ClosestAfter", answer: a, afterIndex: ai },
      value: target,
    };
  },
  (sol, _qi, n, rng) => {
    const a = rng.pick(LETTERS);
    const bi = rng.int(2, n - 1);
    let target = -1;
    for (let i = bi - 1; i >= 0; i--) {
      if (sol[i] === a) {
        target = i;
        break;
      }
    }
    if (target < 0) return null;
    return {
      questionType: { type: "ClosestBefore", answer: a, beforeIndex: bi },
      value: target,
    };
  },
  (sol) => {
    const counts = [0, 0, 0, 0, 0];
    for (const a of sol) counts[L2I[a]] += 1;
    const max = Math.max(...counts);
    return { questionType: { type: "MostCommonCount" }, value: max };
  },
  (sol) => {
    const counts = [0, 0, 0, 0, 0];
    for (const a of sol) counts[L2I[a]] += 1;
    const min = Math.min(...counts);
    if (counts.filter((c) => c === min).length !== 1) return null;
    return { questionType: { type: "LeastCommon" }, value: counts.indexOf(min) };
  },
  (sol, _qi, _n, rng) => {
    const ref = rng.pick(LETTERS);
    const refCount = sol.filter((x) => x === ref).length;
    const candidates = LETTERS.filter(
      (l) => l !== ref && sol.filter((x) => x === l).length === refCount,
    );
    if (candidates.length === 0) return null;
    const target = rng.pick(candidates);
    return {
      questionType: { type: "EqualCount", answer: ref },
      value: L2I[target],
    };
  },
  (sol) => {
    let pairIdx = -1;
    let pairCount = 0;
    for (let i = 0; i < sol.length - 1; i++) {
      if (sol[i] === sol[i + 1]) {
        if (pairCount === 0) pairIdx = i;
        pairCount++;
      }
    }
    if (pairCount > 1) return null;
    return { questionType: { type: "ConsecIdent" }, value: pairIdx };
  },
  (sol, _qi, _n, rng) => {
    const a = rng.pick(LETTERS);
    let found = -1;
    let count = 0;
    for (let i = 0; i < sol.length; i++) {
      if ((i + 1) % 2 === 1 && sol[i] === a) {
        found = i;
        count++;
      }
    }
    if (count !== 1) return null;
    return { questionType: { type: "OnlyOdd", answer: a }, value: found };
  },
  (sol, _qi, _n, rng) => {
    const a = rng.pick(LETTERS);
    let found = -1;
    let count = 0;
    for (let i = 0; i < sol.length; i++) {
      if ((i + 1) % 2 === 0 && sol[i] === a) {
        found = i;
        count++;
      }
    }
    if (count !== 1) return null;
    return { questionType: { type: "OnlyEven", answer: a }, value: found };
  },
  (sol, qi, n, rng) => {
    const ref = rng.int(0, n - 1);
    const refAns = sol[ref];
    const matches: number[] = [];
    for (let i = 0; i < n; i++) {
      if (i !== ref && i !== qi && sol[i] === refAns) matches.push(i);
    }
    if (matches.length === 0) return null;
    return {
      questionType: { type: "SameAsWhich", questionIndex: ref },
      value: rng.pick(matches),
    };
  },
];

function makeTrueClaim(sol: Answer[], qi: number, n: number, optionCount: number, rng: RNG): Claim {
  for (let attempt = 0; attempt < 20; attempt++) {
    const gen = rng.pick(CLAIM_GENS);
    const claim = gen(sol, qi, n, rng);
    if (claim != null && checkClaimFast(optionCount, sol, qi, claim)) return claim;
  }
  const a = rng.pick(LETTERS);
  return {
    questionType: { type: "CountAnswer", answer: a },
    value: sol.filter((x) => x === a).length,
  };
}

function perturbClaim(claim: Claim, n: number, rng: RNG): Claim | null {
  const qt = claim.questionType.type;
  switch (qt) {
    case "CountAnswer":
    case "CountConsonant":
    case "CountVowel":
    case "CountAnswerAfter":
    case "CountAnswerBefore":
    case "MostCommonCount": {
      const offset = rng.pick([-2, -1, 1, 2]);
      const newVal = claim.value + offset;
      if (newVal < 0 || newVal > n) return null;
      return { ...claim, value: newVal };
    }
    case "FirstWith":
    case "LastWith":
    case "ClosestAfter":
    case "ClosestBefore":
    case "ConsecIdent":
    case "OnlyOdd":
    case "OnlyEven":
    case "SameAsWhich":
      return { ...claim, value: rng.int(0, n - 1) };
    case "AnswerOf":
    case "MostCommon":
    case "LeastCommon":
    case "NoOtherHasAnswer":
      return { ...claim, value: L2I[rng.pick(LETTERS)] };
    case "EqualCount": {
      const v = L2I[rng.pick(LETTERS)];
      if (v === L2I[claim.questionType.answer]) return null;
      return { ...claim, value: v };
    }
    default:
      return null;
  }
}

function makeFalseClaim(
  sol: Answer[],
  qi: number,
  n: number,
  optionCount: number,
  rng: RNG,
): Claim {
  for (let i = 0; i < 30; i++) {
    const base = makeTrueClaim(sol, qi, n, optionCount, rng);
    const fc = perturbClaim(base, n, rng);
    if (fc && !checkClaimFast(optionCount, sol, qi, fc)) return fc;
  }
  return { questionType: { type: "CountAnswer", answer: "A" }, value: n + 1 };
}
function claimCategory(c: Claim): string {
  const qt = c.questionType;
  switch (qt.type) {
    case "CountAnswer":
      return "count:" + qt.answer;
    case "CountConsonant":
      return "consonant";
    case "CountVowel":
      return "vowel";
    case "CountAnswerAfter":
      return "after:" + qt.answer;
    case "CountAnswerBefore":
      return "before:" + qt.answer;
    case "AnswerOf":
      return "answerof:" + qt.questionIndex;
    case "FirstWith":
      return "first:" + qt.answer;
    case "LastWith":
      return "last:" + qt.answer;
    case "MostCommon":
      return "mostcommon";
    case "ClosestAfter":
      return "closestafter:" + qt.answer + ":" + qt.afterIndex;
    case "ClosestBefore":
      return "closestbefore:" + qt.answer + ":" + qt.beforeIndex;
    case "MostCommonCount":
      return "mostcommoncount";
    case "LeastCommon":
      return "leastcommon";
    case "NoOtherHasAnswer":
      return "unique";
    case "EqualCount":
      return "equalcount:" + qt.answer;
    case "ConsecIdent":
      return "consecident";
    case "OnlyOdd":
      return "onlyodd:" + qt.answer;
    case "OnlyEven":
      return "onlyeven:" + qt.answer;
    case "SameAsWhich":
      return "sameaswhich:" + qt.questionIndex;
    default:
      return "unknown:" + qt.type;
  }
}
