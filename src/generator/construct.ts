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
  QuestionTypeDef,
  Claim,
  StatementOption,
} from "../engine/types.ts";
import { LETTERS, L2I, flattenPuzzle } from "../engine/types.ts";
import { checkQuestionAgainstSolution as evaluate } from "../engine/check-validity.ts";
import { evaluateClaim } from "../engine/evaluators.ts";
import { deduce } from "../engine/deduce.ts";
import { lookahead } from "../engine/lookahead.ts";
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

const CONSTRAINED_TYPES = new Set<string>(["Unique", "AnswerIsSelf"]);

// Rule types by category
const ENTRY_TYPES: QuestionTypeDef["type"][] = [
  "CountAnswer",
  "CountAnswerBefore",
  "CountAnswerAfter",
  "CountVowel",
  "CountConsonant",
];

const POSITIONAL_TYPES: QuestionTypeDef["type"][] = [
  "FirstWith",
  "LastWith",
  "ClosestAfter",
  "ClosestBefore",
];

const VARIETY_TYPES: QuestionTypeDef["type"][] = [
  "LetterDist",
  "ConsecIdent",
  "MostCommonCount",
  "PrevSame",
  "NextSame",
  "OnlySame",
  "SameAs",
  "OnlyOdd",
  "OnlyEven",
  "LeastCommon",
  "MostCommon",
  "Unique",
  "EqualCount",
  "AnswerIsSelf",
  "TrueStmt",
];

const STRUCTURAL_TYPES = new Set<QuestionTypeDef["type"]>([
  "ConsecIdent",
  "Unique",
  "OnlySame",
  "OnlyOdd",
  "OnlyEven",
]);

function typeCap(type: QuestionTypeDef["type"]): number {
  if (type === "LetterDist") return 1;
  if (type === "AnswerOf") return 2;
  return 3;
}

function symmetricGroup(type: QuestionTypeDef["type"]): string | null {
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
  types: QuestionTypeDef["type"][],
  profile: DifficultyProfile,
): QuestionTypeDef["type"][] {
  return types.filter((t) => profile.allowedTypes.includes(t));
}

function tryConstructive(profile: DifficultyProfile, rng: RNG): GenerateResult | null {
  const n = profile.questionCount;

  // 1. Random solution
  const solution: AnswerLetter[] = Array.from({ length: n }, () => rng.pick(LETTERS));

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

  const rules: (QuestionTypeDef | null)[] = new Array(n).fill(null);
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

  function placeRule(type: QuestionTypeDef["type"], slotIdx: number): boolean {
    const cap = capsOverride[type] ?? typeCap(type);
    if ((kindCounts[type] ?? 0) >= cap) return false;
    const group = symmetricGroup(type);
    if (group !== null && (groupCounts[group] ?? 0) >= (groupCaps[group] ?? 3)) return false;
    const qi = slots[slotIdx];
    if (!solutionCompatible(type, qi, solution, n)) return false;
    for (let attempt = 0; attempt < 10; attempt++) {
      const rule = makeRule(type, qi, n, solution, assigned, rng);
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

  function placeFrom(types: QuestionTypeDef["type"][]): boolean {
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
    if (!placeRule("AnswerOf", assigned.size)) return null;
  }

  // Phase 2b: occasionally place prev/next_same (need specific slot positions)
  if (rng.int(0, 1) === 0 && assigned.size < n) {
    const candidates: [QuestionTypeDef["type"], number][] = [
      ["PrevSame", n - 1],
      ["NextSame", 0],
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
  const exoticSlots: QuestionTypeDef["type"][] = [];
  if (allowed(["LetterDist"], profile).length > 0) exoticSlots.push("LetterDist");
  if (allowed(["TrueStmt"], profile).length > 0) exoticSlots.push("TrueStmt");
  if (allowed(["ConsecIdent"], profile).length > 0) exoticSlots.push("ConsecIdent");
  for (const type of exoticSlots) {
    if (assigned.size >= n) break;
    placeRule(type, assigned.size);
  }

  // Phase 5: Fill remaining, reserving slots for structural rules
  const avStructural = avVariety.filter((t) => STRUCTURAL_TYPES.has(t));
  const structuralReserve = Math.min(avStructural.length > 0 ? 1 : 0, n - assigned.size);
  const fillTarget = n - structuralReserve;

  const fillPool: QuestionTypeDef["type"][] = [...avEntry, ...avPositional, ...avVariety].filter(
    (t) => profile.allowedTypes.includes(t) && t !== "AnswerOf",
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
        !placeRule("AnswerOf", assigned.size) &&
        !placeRule("AnswerIsSelf", assigned.size)
      )
        return null;
    }
  }

  // 3. Build and validate puzzle
  const finalRules: QuestionTypeDef[] = rules.filter((t): t is QuestionTypeDef => t !== null);
  const questions = finalRules.map<QuestionDef>((questionType, i) => ({
    options: engineerOptions(questionType, i, solution, n, rng),
    questionType,
  }));

  const puzzle: Puzzle = {
    id: `level-${profile.level}`,
    title: profile.name,
    difficulty: String(profile.level),
    questions,
  };

  const fp = flattenPuzzle(puzzle);
  for (let i = 0; i < n; i++) {
    if (!evaluate(fp, i, solution[i], solution)) return null;
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
      if (!evaluate(fp2, i, solution[i], solution)) {
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
  const fp = flattenPuzzle(puzzle);
  const answers: (AnswerLetter | null)[] = new Array(n).fill(null);
  const eliminated: number[] = new Array(n).fill(0);
  for (let step = 0; step < n * 15; step++) {
    if (answers.every((a) => a != null)) return { solved: true, answers };
    const drs = deduce(fp, answers, eliminated);
    if (drs.length > 0) {
      for (const dr of drs) applyDeduceAction(dr.action, answers, eliminated);
      continue;
    }
    const lr = lookahead(fp, answers, eliminated);
    if (lr) {
      eliminated[lr.eliminateQi] |= 1 << lr.eliminateOi;
      continue;
    }
    return { solved: false, answers };
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
    const rule = puzzle.questions[qi].questionType;
    const correctOi = L2I[solution[qi]];

    if (CONSTRAINED_TYPES.has(rule.type)) continue;
    if (rule.type === "TrueStmt") continue;

    const opts = puzzle.questions[qi].options;

    if (rule.type === "AnswerOf") {
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

    if (rule.type === "LetterDist" && rule.questionIndex != null) {
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

    if (rule.type === "ConsecIdent") {
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
    "CountAnswer",
    "CountAnswerBefore",
    "CountAnswerAfter",
    "CountVowel",
    "CountConsonant",
    "MostCommonCount",
  ].includes(type);
}

function repairCountingDistractors(
  rule: QuestionTypeDef,
  correctVal: number | null,
  answers: (AnswerLetter | null)[],
  n: number,
  rng: RNG,
): (number | null)[] {
  const from = rule.type === "CountAnswerAfter" ? rule.afterIndex + 1 : 0;
  const to = rule.type === "CountAnswerBefore" ? rule.beforeIndex : n;

  let confirmed = 0;
  let unknown = 0;
  for (let i = from; i < to; i++) {
    if (answers[i] == null) {
      unknown++;
    } else if (
      rule.type === "CountVowel"
        ? answers[i] === "A" || answers[i] === "E"
        : rule.type === "CountConsonant"
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
    rule.type === "CountAnswerBefore"
      ? rule.beforeIndex
      : rule.type === "CountAnswerAfter"
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
  rule: QuestionTypeDef,
  correctVal: number | null,
  qi: number,
  answers: (AnswerLetter | null)[],
  n: number,
  rng: RNG,
): (number | null)[] {
  const answer = "answer" in rule ? rule.answer : undefined;
  let minPos = 0;
  let maxPos = n - 1;
  if (rule.type === "ClosestAfter") minPos = rule.afterIndex + 1;
  if (rule.type === "ClosestBefore") maxPos = rule.beforeIndex - 1;
  if (rule.type === "PrevSame") maxPos = qi - 1;
  if (rule.type === "NextSame") minPos = qi + 1;

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

function applyDeduceAction(
  action: {
    type: string;
    questionIndex?: number;
    questionMask?: number;
    letter?: AnswerLetter;
    optionIndex?: number;
    optionMask?: number;
  },
  answers: (AnswerLetter | null)[],
  eliminated: number[],
) {
  if (action.type === "force" && action.letter && action.questionIndex != null) {
    const oi = L2I[action.letter];
    eliminated[action.questionIndex] = 0b11111 ^ (1 << oi);
    answers[action.questionIndex] = action.letter;
  } else if (
    action.type === "eliminateMulti" &&
    action.questionMask != null &&
    action.optionMask != null
  ) {
    for (let i = 0; i < eliminated.length; i++) {
      if ((action.questionMask >> i) & 1) eliminated[i] |= action.optionMask;
    }
  } else if (
    action.type === "eliminate" &&
    action.questionIndex != null &&
    action.optionIndex != null
  ) {
    eliminated[action.questionIndex] |= 1 << action.optionIndex;
  }
}

// ── Rule factory ──

function makeRule(
  type: QuestionTypeDef["type"],
  qi: number,
  n: number,
  solution: AnswerLetter[],
  assigned: Set<number>,
  rng: RNG,
): QuestionTypeDef | null {
  switch (type) {
    case "CountAnswer":
      return { type, answer: rng.pick(LETTERS) };
    case "CountAnswerBefore":
      if (n < 6) return null;
      return {
        type,
        answer: rng.pick(LETTERS),
        beforeIndex: rng.int(4, n - 1),
      };
    case "CountAnswerAfter":
      if (n < 6) return null;
      return {
        type,
        answer: rng.pick(LETTERS),
        afterIndex: rng.int(0, Math.max(0, n - 5)),
      };
    case "CountVowel":
    case "CountConsonant":
    case "MostCommonCount":
      return { type };
    case "AnswerOf": {
      const targets = [...assigned].filter((j) => j !== qi);
      if (targets.length === 0) return null;
      return { type, questionIndex: rng.pick(targets) };
    }
    case "LetterDist": {
      const targets = [...assigned].filter((j) => j !== qi);
      if (targets.length === 0) {
        // Can point at any other question
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
        answer: rng.pick(LETTERS),
      };
    case "ClosestBefore":
      if (n < 5) return null;
      return {
        type,
        beforeIndex: rng.int(4, n - 1),
        answer: rng.pick(LETTERS),
      };
    case "FirstWith":
    case "LastWith":
      return { type, answer: rng.pick(LETTERS) };
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
    case "Unique":
      return { type };
    case "EqualCount": {
      const refLetter = rng.pick(LETTERS);
      const refCount = solution.filter((a) => a === refLetter).length;
      const hasMatch = LETTERS.some(
        (l) => l !== refLetter && solution.filter((a) => a === l).length === refCount,
      );
      if (!hasMatch && rng.int(0, 4) > 1) return null;
      return { type, answer: refLetter };
    }
    case "OnlyOdd":
    case "OnlyEven":
      return { type, answer: rng.pick(LETTERS) };
    case "AnswerIsSelf":
      return { type };
    case "TrueStmt":
      return { type };
  }
  return null;
}

// ── Structural checks ──

function checkStructural(rule: QuestionTypeDef, qi: number, sol: AnswerLetter[]): boolean {
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
    case "Unique":
      return sol.filter((a) => a === sol[qi]).length === 1;
    default:
      return true;
  }
}

function solutionHasStructural(
  type: QuestionTypeDef["type"],
  qi: number,
  solution: AnswerLetter[],
  n: number,
): boolean {
  switch (type) {
    case "ConsecIdent": {
      let pairs = 0;
      for (let i = 0; i < n - 1; i++) if (solution[i] === solution[i + 1]) pairs++;
      return pairs === 1;
    }
    case "Unique":
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
  type: QuestionTypeDef["type"],
  qi: number,
  solution: AnswerLetter[],
  n: number,
): boolean {
  switch (type) {
    case "LeastCommon": {
      const c = letterCounts(solution.slice(0, n));
      return c.filter((v) => v === Math.min(...c)).length === 1;
    }
    case "MostCommon": {
      const c = letterCounts(solution.slice(0, n));
      return c.filter((v) => v === Math.max(...c)).length === 1;
    }
    case "SameAs": {
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

function letterCounts(sol: AnswerLetter[]): number[] {
  const c = [0, 0, 0, 0, 0];
  for (const a of sol) c[L2I[a]]++;
  return c;
}

function engineerOptions(
  rule: QuestionTypeDef,
  qi: number,
  solution: AnswerLetter[],
  n: number,
  rng: RNG,
): OptionDef[] {
  // Constrained types: value is the letter index (0=A, 1=B, etc.)
  if (CONSTRAINED_TYPES.has(rule.type)) return LETTERS.map((_l, i) => ({ value: i }));
  if (rule.type === "TrueStmt") return buildClaims(qi, solution, n, rng);
  if (rule.type === "LeastCommon" || rule.type === "MostCommon") {
    const counts = letterCounts(solution.slice(0, n));
    const target = rule.type === "LeastCommon" ? Math.min(...counts) : Math.max(...counts);
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

function computeValue(rule: QuestionTypeDef, qi: number, sol: AnswerLetter[]): number | null {
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
  }
  throw new Error(`computeValue: ${rule.type}`);
}

function makeDistractors(
  rule: QuestionTypeDef,
  correct: number | null,
  qi: number,
  n: number,
  rng: RNG,
): (number | null)[] {
  if (rule.type === "AnswerOf") return rng.shuffle([0, 1, 2, 3, 4].filter((v) => v !== correct));
  if (rule.type === "LetterDist")
    return rng.shuffle([0, 1, 2, 3, 4].filter((v) => v !== correct)).slice(0, 4);
  if (rule.type === "ConsecIdent") {
    const pool: (number | null)[] = [];
    for (let i = 0; i < n - 1; i++) {
      if (i !== correct) pool.push(i);
    }
    if (correct != null) pool.push(null);
    return rng.shuffle(pool).slice(0, 4);
  }
  if (
    [
      "CountAnswer",
      "CountAnswerBefore",
      "CountAnswerAfter",
      "CountVowel",
      "CountConsonant",
      "MostCommonCount",
    ].includes(rule.type)
  ) {
    let max = n;
    if (rule.type === "CountAnswerBefore") max = rule.beforeIndex;
    if (rule.type === "CountAnswerAfter") max = n - rule.afterIndex - 1;
    const pool: number[] = [];
    for (let i = 0; i <= Math.max(max, 4); i++) if (i !== correct) pool.push(i);
    return rng.shuffle(pool).slice(0, 4);
  }
  if (rule.type === "OnlyOdd" || rule.type === "OnlyEven") {
    const parity = rule.type === "OnlyOdd" ? 1 : 0;
    const pool: (number | null)[] = [];
    for (let i = 0; i < n; i++) {
      if ((i + 1) % 2 === parity && i !== correct) pool.push(i);
    }
    if (correct != null) pool.push(null);
    return rng.shuffle(pool).slice(0, 4);
  }
  if (rule.type === "EqualCount") {
    const pool: (number | null)[] = [];
    for (const l of LETTERS) {
      const li = L2I[l];
      if (l !== rule.answer && li !== correct) pool.push(li);
    }
    if (correct != null) pool.push(null);
    return rng.shuffle(pool).slice(0, 4);
  }
  // Positional (0-based indices)
  let minPos = 0,
    maxPos = n - 1;
  if (rule.type === "ClosestAfter") minPos = rule.afterIndex + 1;
  if (rule.type === "ClosestBefore") maxPos = rule.beforeIndex - 1;
  if (rule.type === "PrevSame") maxPos = qi - 1;
  if (rule.type === "NextSame") minPos = qi + 1;
  const excludeSelf = rule.type === "OnlySame" || rule.type === "SameAs";
  const pool: (number | null)[] = [];
  for (let i = minPos; i <= maxPos; i++) {
    if (i !== correct && !(excludeSelf && i === qi)) pool.push(i);
  }
  if (correct != null) pool.push(null);
  return rng.shuffle(pool).slice(0, 4);
}

function buildClaims(qi: number, solution: AnswerLetter[], n: number, rng: RNG): StatementOption[] {
  const targetIdx = L2I[solution[qi]];
  const options: StatementOption[] = new Array(5);
  const trueClaim = makeTrueClaim(solution, n, rng);
  options[targetIdx] = { value: null, claim: trueClaim };
  const usedKeys = new Set([claimCategory(trueClaim)]);
  for (let i = 0; i < 5; i++) {
    if (i === targetIdx) continue;
    for (let att = 0; att < 30; att++) {
      const fc = makeFalseClaim(solution, qi, n, rng);
      const key = claimCategory(fc);
      if (!usedKeys.has(key)) {
        usedKeys.add(key);
        options[i] = { value: null, claim: fc };
        break;
      }
    }
    if (!options[i]) {
      const fc = makeFalseClaim(solution, qi, n, rng);
      options[i] = { value: null, claim: fc };
    }
  }
  return options;
}
function makeTrueClaim(sol: AnswerLetter[], n: number, rng: RNG): Claim {
  const t = rng.int(0, 8);
  if (t === 0) {
    const a = rng.pick(LETTERS);
    return {
      questionType: { type: "CountAnswer", answer: a },
      value: sol.filter((x) => x === a).length,
    };
  }
  if (t === 1)
    return {
      questionType: { type: "CountConsonant" },
      value: sol.filter((a) => a !== "A" && a !== "E").length,
    };
  if (t === 2)
    return {
      questionType: { type: "CountVowel" },
      value: sol.filter((a) => a === "A" || a === "E").length,
    };
  if (t === 3) {
    const a = rng.pick(LETTERS);
    const ai = rng.int(0, Math.max(0, n - 5));
    return {
      questionType: { type: "CountAnswerAfter", answer: a, afterIndex: ai },
      value: sol.slice(ai + 1).filter((x) => x === a).length,
    };
  }
  if (t === 4) {
    const a = rng.pick(LETTERS);
    const bi = rng.int(4, n - 1);
    return {
      questionType: { type: "CountAnswerBefore", answer: a, beforeIndex: bi },
      value: sol.slice(0, bi).filter((x) => x === a).length,
    };
  }
  if (t === 5) {
    const targetQi = rng.int(0, n - 1);
    return {
      questionType: { type: "AnswerOf", questionIndex: targetQi },
      value: L2I[sol[targetQi]],
    };
  }
  if (t === 6) {
    const a = rng.pick(LETTERS);
    const first = sol.indexOf(a);
    if (first >= 0) return { questionType: { type: "FirstWith", answer: a }, value: first };
    const a2 = rng.pick(LETTERS);
    return {
      questionType: { type: "CountAnswer", answer: a2 },
      value: sol.filter((x) => x === a2).length,
    };
  }
  if (t === 7) {
    const a = rng.pick(LETTERS);
    const last = sol.lastIndexOf(a);
    if (last >= 0) return { questionType: { type: "LastWith", answer: a }, value: last };
    const a2 = rng.pick(LETTERS);
    return {
      questionType: { type: "CountAnswer", answer: a2 },
      value: sol.filter((x) => x === a2).length,
    };
  }
  const counts = [0, 0, 0, 0, 0];
  for (const a of sol) counts[L2I[a]] += 1;
  const max = Math.max(...counts);
  const most = LETTERS.filter((_, i) => counts[i] === max);
  if (most.length === 1) {
    return { questionType: { type: "MostCommon" }, value: L2I[most[0]] };
  }
  const a = rng.pick(LETTERS);
  return {
    questionType: { type: "CountAnswer", answer: a },
    value: sol.filter((x) => x === a).length,
  };
}
function perturbClaim(claim: Claim, n: number, rng: RNG): Claim | null {
  switch (claim.questionType.type) {
    case "CountAnswer":
    case "CountConsonant":
    case "CountVowel":
    case "CountAnswerAfter":
    case "CountAnswerBefore": {
      const offset = rng.pick([-2, -1, 1, 2]);
      const newVal = claim.value + offset;
      if (newVal < 0 || newVal > n) return null;
      return { ...claim, value: newVal };
    }
    case "FirstWith":
    case "LastWith":
      return { ...claim, value: rng.int(0, n - 1) };
    case "AnswerOf":
    case "MostCommon":
      return { ...claim, value: L2I[rng.pick(LETTERS)] };
    default:
      return null;
  }
}
function makeFalseClaim(sol: AnswerLetter[], qi: number, n: number, rng: RNG): Claim {
  for (let i = 0; i < 30; i++) {
    const base = makeTrueClaim(sol, n, rng);
    const fc = perturbClaim(base, n, rng);
    if (fc && !evaluateClaim(fc, qi, sol)) return fc;
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
    default:
      return "unknown:" + qt.type;
  }
}
