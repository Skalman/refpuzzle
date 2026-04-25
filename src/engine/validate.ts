import type { AnswerLetter, Puzzle, FlatPuzzle, FlatRule, Marks } from "./types.ts";
import {
  getFlatPuzzle,
  NONE,
  letterIdx,
  RT_COUNT_ANSWER,
  RT_COUNT_ANSWER_BEFORE,
  RT_COUNT_ANSWER_AFTER,
  RT_COUNT_VOWEL,
  RT_COUNT_CONSONANT,
  RT_ANSWER_OF,
  RT_LETTER_DIST,
  RT_CLOSEST_AFTER,
  RT_CLOSEST_BEFORE,
  RT_FIRST_WITH,
  RT_LAST_WITH,
  RT_SAME_AS,
  RT_UNIQUE,
  RT_TRUE_STMT,
  RT_SELF,
} from "./types.ts";
import type { Claim } from "./types.ts";
import { evaluate } from "./evaluators.ts";

export type Validity = "neutral" | "valid" | "invalid" | "pending";

export function validate(
  puzzle: Puzzle,
  answers: (AnswerLetter | null)[],
  markSets?: Marks[],
): Validity[] {
  const fp = getFlatPuzzle(puzzle);
  const allAnswered = answers.every((a) => a != null);
  return fp.rules.map((r, i) => {
    const answer = answers[i];
    if (answer == null) return "neutral";
    if (allAnswered) {
      return evaluate(r, i, answer, answers, fp) ? "valid" : "invalid";
    }
    // only_true_statement: if selected claim is definitively true, that's sufficient
    if (r.t === RT_TRUE_STMT) {
      const selectedClaim = fp.optionClaims[i][letterIdx(answer)];
      if (selectedClaim && isClaimDefinitive(selectedClaim, fp.n, answers, markSets))
        return "valid";
      return "pending";
    }
    const isValid = evaluate(r, i, answer, answers, fp);
    if (isValid && isDefinitive(r, i, answers, fp, markSets)) return "valid";
    if (!isValid && isProvablyWrong(r, i, answer, answers, fp)) return "invalid";
    return "pending";
  });
}

const L2I: Record<string, number> = { A: 0, B: 1, C: 2, D: 3, E: 4 };

// Check if question j definitely cannot have answer `letter`
// (either answered as something else, or that option is marked incorrect)
function cantBe(
  j: number,
  letter: string,
  answers: (AnswerLetter | null)[],
  markSets?: Marks[],
): boolean {
  if (answers[j] != null) return answers[j] !== letter;
  if (markSets?.[j]) return markSets[j][L2I[letter]] === "incorrect";
  return false;
}

// Check if question j's answer is fully determined (answered, or all but one eliminated)
function isResolved(j: number, answers: (AnswerLetter | null)[], markSets?: Marks[]): boolean {
  if (answers[j] != null) return true;
  if (!markSets?.[j]) return false;
  let remaining = 0;
  for (let k = 0; k < 5; k++) if (markSets[j][k] !== "incorrect") remaining++;
  return remaining <= 1;
}

// Check if question j's answer is definitely a vowel, definitely a consonant, or unknown
function vowelConsonantResolved(
  j: number,
  answers: (AnswerLetter | null)[],
  markSets?: Marks[],
): boolean {
  if (answers[j] != null) return true;
  if (!markSets?.[j]) return false;
  const VOWELS = [0, 4]; // A=0, E=4
  let hasVowel = false;
  let hasConsonant = false;
  for (let k = 0; k < 5; k++) {
    if (markSets[j][k] !== "incorrect") {
      if (VOWELS.includes(k)) hasVowel = true;
      else hasConsonant = true;
    }
  }
  return !(hasVowel && hasConsonant);
}

function isDefinitive(
  rule: FlatRule,
  qi: number,
  answers: (AnswerLetter | null)[],
  fp: FlatPuzzle,
  markSets?: Marks[],
): boolean {
  const n = fp.n;
  const ai = letterIdx(answers[qi]!);
  const on = fp.optionNums[qi][ai];

  switch (rule.t) {
    case RT_SELF:
      return true;
    case RT_ANSWER_OF:
      return answers[rule.questionIndex] != null;
    case RT_LETTER_DIST:
      return answers[rule.otherQuestionIndex] != null;

    // Counting: definitive if every question in range is resolved
    case RT_COUNT_ANSWER: {
      const target = rule.answer!;
      for (let j = 0; j < n; j++)
        if (!isResolved(j, answers, markSets) && !cantBe(j, target, answers, markSets))
          return false;
      return true;
    }
    case RT_COUNT_ANSWER_BEFORE: {
      const target = rule.answer!;
      for (let j = 0; j < rule.beforeIndex; j++)
        if (!isResolved(j, answers, markSets) && !cantBe(j, target, answers, markSets))
          return false;
      return true;
    }
    case RT_COUNT_ANSWER_AFTER: {
      const target = rule.answer!;
      for (let j = rule.afterIndex + 1; j < n; j++)
        if (!isResolved(j, answers, markSets) && !cantBe(j, target, answers, markSets))
          return false;
      return true;
    }
    case RT_COUNT_VOWEL:
    case RT_COUNT_CONSONANT:
      for (let j = 0; j < n; j++) if (!vowelConsonantResolved(j, answers, markSets)) return false;
      return true;

    // Positional: for "first_with E = Q5", definitive if Q1-Q4 can't be E and Q5 is E
    case RT_CLOSEST_AFTER:
    case RT_FIRST_WITH: {
      const target = rule.answer!;
      const start = rule.t === RT_CLOSEST_AFTER ? rule.afterIndex + 1 : 0;
      if (on === NONE) {
        // Claimed no match — every question in range must be resolved or can't be target
        for (let j = start; j < n; j++) if (!cantBe(j, target, answers, markSets)) return false;
        return true;
      }
      const pos = on - 1;
      // Claimed position must have the answer
      if (answers[pos] == null) return false;
      // All positions before must definitively not have the answer
      for (let j = start; j < pos; j++) if (!cantBe(j, target, answers, markSets)) return false;
      return true;
    }
    case RT_CLOSEST_BEFORE:
    case RT_LAST_WITH: {
      const target = rule.answer!;
      const end = rule.t === RT_CLOSEST_BEFORE ? rule.beforeIndex : n;
      if (on === NONE) {
        for (let j = 0; j < end; j++) if (!cantBe(j, target, answers, markSets)) return false;
        return true;
      }
      const pos = on - 1;
      if (answers[pos] == null) return false;
      for (let j = pos + 1; j < end; j++) if (!cantBe(j, target, answers, markSets)) return false;
      return true;
    }

    case RT_SAME_AS: {
      const letter = answers[qi]!;
      for (let j = 0; j < n; j++) {
        if (j === qi) continue;
        if (!isResolved(j, answers, markSets) && !cantBe(j, letter, answers, markSets))
          return false;
      }
      return true;
    }

    case RT_UNIQUE:
      for (let j = 0; j < n; j++) if (!isResolved(j, answers, markSets)) return false;
      return true;

    case RT_TRUE_STMT: {
      const selectedClaim = fp.optionClaims[qi][ai];
      if (!selectedClaim) return false;
      return isClaimDefinitive(selectedClaim, n, answers, markSets);
    }
  }

  // Global rules (most_common, etc.): need all answers
  return answers.slice(0, n).every((a) => a != null);
}

function isClaimDefinitive(
  claim: Claim,
  n: number,
  answers: (AnswerLetter | null)[],
  markSets?: Marks[],
): boolean {
  switch (claim.type) {
    case "count_answer_equals": {
      for (let j = 0; j < n; j++)
        if (!isResolved(j, answers, markSets) && !cantBe(j, claim.answer, answers, markSets))
          return false;
      return true;
    }
    case "count_consonant_answers_equals":
    case "count_vowel_answers_equals":
      for (let j = 0; j < n; j++) if (!isResolved(j, answers, markSets)) return false;
      return true;
    case "count_answer_after_equals": {
      for (let j = claim.afterIndex + 1; j < n; j++)
        if (!isResolved(j, answers, markSets) && !cantBe(j, claim.answer, answers, markSets))
          return false;
      return true;
    }
    case "count_answer_before_equals": {
      for (let j = 0; j < claim.beforeIndex; j++)
        if (!isResolved(j, answers, markSets) && !cantBe(j, claim.answer, answers, markSets))
          return false;
      return true;
    }
  }
  return false;
}

function isProvablyWrong(
  rule: FlatRule,
  qi: number,
  answer: AnswerLetter,
  answers: (AnswerLetter | null)[],
  fp: FlatPuzzle,
): boolean {
  const ai = letterIdx(answer);
  const on = fp.optionNums[qi][ai];
  const n = fp.n;

  switch (rule.t) {
    // Counting: provably wrong if count already exceeds or can't reach claimed value
    case RT_COUNT_ANSWER:
    case RT_COUNT_ANSWER_BEFORE:
    case RT_COUNT_ANSWER_AFTER:
    case RT_COUNT_VOWEL:
    case RT_COUNT_CONSONANT: {
      if (Number.isNaN(on)) return false;
      const rangeStart = rule.t === RT_COUNT_ANSWER_AFTER ? rule.afterIndex + 1 : 0;
      const rangeEnd = rule.t === RT_COUNT_ANSWER_BEFORE ? rule.beforeIndex : n;
      let count = 0;
      let remaining = 0;
      for (let j = rangeStart; j < rangeEnd; j++) {
        const a = answers[j];
        if (a == null) {
          remaining++;
        } else if (
          rule.t === RT_COUNT_VOWEL
            ? a === "A" || a === "E"
            : rule.t === RT_COUNT_CONSONANT
              ? a !== "A" && a !== "E"
              : a === rule.answer
        ) {
          count++;
        }
      }
      return count > on || count + remaining < on;
    }

    // answer_of_question: if referenced question is answered with different letter
    case RT_ANSWER_OF: {
      const target = answers[rule.questionIndex];
      if (target == null) return false;
      return target !== fp.optionLabels[qi][ai];
    }

    // letter_distance: if other question answered, distance doesn't match
    case RT_LETTER_DIST: {
      const other = answers[rule.otherQuestionIndex];
      if (other == null) return false;
      const dist = Math.abs(ai - letterIdx(other));
      return dist !== on;
    }

    // Positional: if claimed position is answered with wrong letter
    case RT_CLOSEST_AFTER:
    case RT_FIRST_WITH: {
      if (on === NONE) {
        // Claimed "None" but a match exists in the range
        const start = rule.t === RT_CLOSEST_AFTER ? rule.afterIndex + 1 : 0;
        for (let j = start; j < n; j++) {
          if (answers[j] === rule.answer) return true;
        }
        return false;
      }
      const pos = on - 1;
      if (pos >= 0 && pos < n && answers[pos] != null && answers[pos] !== rule.answer) return true;
      // Closer match exists
      const start = rule.t === RT_CLOSEST_AFTER ? rule.afterIndex + 1 : 0;
      for (let j = start; j < pos; j++) {
        if (answers[j] === rule.answer) return true;
      }
      return false;
    }

    case RT_CLOSEST_BEFORE:
    case RT_LAST_WITH: {
      if (on === NONE) {
        const end = rule.t === RT_CLOSEST_BEFORE ? rule.beforeIndex : n;
        for (let j = 0; j < end; j++) {
          if (answers[j] === rule.answer) return true;
        }
        return false;
      }
      const pos = on - 1;
      if (pos >= 0 && pos < n && answers[pos] != null && answers[pos] !== rule.answer) return true;
      const end = rule.t === RT_CLOSEST_BEFORE ? rule.beforeIndex : n;
      for (let j = end - 1; j > pos; j--) {
        if (answers[j] === rule.answer) return true;
      }
      return false;
    }

    // same_answer_as: if target question answered with different letter
    case RT_SAME_AS: {
      const tq = on - 1;
      if (tq < 0 || tq >= n) return false;
      if (answers[tq] != null && answers[tq] !== answer) return true;
      return false;
    }

    // unique: if same answer appears elsewhere
    case RT_UNIQUE: {
      let count = 0;
      for (let j = 0; j < n; j++) {
        if (answers[j] === answer) count++;
      }
      return count > 1;
    }
  }

  return false;
}
