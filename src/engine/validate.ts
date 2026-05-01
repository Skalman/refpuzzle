import type { AnswerLetter, Puzzle, FlatPuzzle, FlatQuestion, Marks } from "./types.ts";
import {
  getFlatPuzzle,
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
  RT_ONLY_ODD,
  RT_CONSEC_IDENT,
  RT_PREV_SAME,
  RT_NEXT_SAME,
  RT_ONLY_SAME,
  RT_UNIQUE,
  RT_TRUE_STMT,
  RT_SELF,
} from "./types.ts";
import type { Claim } from "./types.ts";
import { checkQuestionAgainstSolution } from "./check-validity.ts";

export type Validity = "neutral" | "valid" | "invalid" | "pending";

export function validate(
  puzzle: Puzzle,
  answers: (AnswerLetter | null)[],
  markSets?: Marks[],
): Validity[] {
  const fp = getFlatPuzzle(puzzle);
  const allAnswered = answers.every((a) => a != null);
  return fp.questions.map((r, i) => {
    const answer = answers[i];
    if (answer == null) return "neutral";
    if (allAnswered) {
      return checkQuestionAgainstSolution(fp, i, answer, answers) ? "valid" : "invalid";
    }
    // only_true_statement: if selected claim is definitively true, that's sufficient
    if (r.t === RT_TRUE_STMT) {
      const selectedClaim = fp.optionClaims[i][letterIdx(answer)];
      if (selectedClaim && isClaimDefinitive(selectedClaim, fp.n, answers, markSets))
        return "valid";
      return "pending";
    }
    const isValid = checkQuestionAgainstSolution(fp, i, answer, answers);
    if (isValid && isDefinitive(r, i, answers, fp, markSets)) return "valid";
    if (!isValid && isProvablyWrong(r, i, answer, answers, fp)) return "invalid";
    return "pending";
  });
}

const L2I: Record<string, number> = { A: 0, B: 1, C: 2, D: 3, E: 4 };

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

function isResolved(j: number, answers: (AnswerLetter | null)[], markSets?: Marks[]): boolean {
  if (answers[j] != null) return true;
  if (!markSets?.[j]) return false;
  let remaining = 0;
  for (let k = 0; k < 5; k++) if (markSets[j][k] !== "incorrect") remaining++;
  return remaining <= 1;
}

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
  rule: FlatQuestion,
  qi: number,
  answers: (AnswerLetter | null)[],
  fp: FlatPuzzle,
  markSets?: Marks[],
): boolean {
  const n = fp.n;
  const ai = letterIdx(answers[qi]!);
  const v = fp.optionValues[qi][ai];

  switch (rule.t) {
    case RT_SELF:
      return true;
    case RT_ANSWER_OF:
      return answers[rule.questionIndex] != null;
    case RT_LETTER_DIST:
      return answers[rule.questionIndex] != null;

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

    case RT_CLOSEST_AFTER:
    case RT_FIRST_WITH: {
      const target = rule.answer!;
      const start = rule.t === RT_CLOSEST_AFTER ? rule.afterIndex + 1 : 0;
      if (v == null) {
        for (let j = start; j < n; j++) if (!cantBe(j, target, answers, markSets)) return false;
        return true;
      }
      if (answers[v] == null) return false;
      for (let j = start; j < v; j++) if (!cantBe(j, target, answers, markSets)) return false;
      return true;
    }
    case RT_CLOSEST_BEFORE:
    case RT_LAST_WITH: {
      const target = rule.answer!;
      const end = rule.t === RT_CLOSEST_BEFORE ? rule.beforeIndex : n;
      if (v == null) {
        for (let j = 0; j < end; j++) if (!cantBe(j, target, answers, markSets)) return false;
        return true;
      }
      if (answers[v] == null) return false;
      for (let j = v + 1; j < end; j++) if (!cantBe(j, target, answers, markSets)) return false;
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

    case RT_ONLY_ODD: {
      const target = rule.answer!;
      if (v == null) {
        for (let j = 0; j < n; j += 2) if (!cantBe(j, target, answers, markSets)) return false;
        return true;
      }
      if (answers[v] == null) return false;
      for (let j = 0; j < n; j += 2) {
        if (j === v) continue;
        if (!cantBe(j, target, answers, markSets)) return false;
      }
      return true;
    }

    case RT_CONSEC_IDENT: {
      for (let j = 0; j < n - 1; j++) {
        if (!isResolved(j, answers, markSets) || !isResolved(j + 1, answers, markSets))
          return false;
      }
      return true;
    }

    case RT_PREV_SAME: {
      for (let j = 0; j < qi; j++) if (!isResolved(j, answers, markSets)) return false;
      return true;
    }

    case RT_NEXT_SAME: {
      for (let j = qi + 1; j < n; j++) if (!isResolved(j, answers, markSets)) return false;
      return true;
    }

    case RT_ONLY_SAME: {
      const letter = answers[qi]!;
      for (let j = 0; j < n; j++) {
        if (j === qi) continue;
        if (!isResolved(j, answers, markSets) && !cantBe(j, letter, answers, markSets))
          return false;
      }
      return true;
    }

    case RT_UNIQUE: {
      const letter = answers[qi]!;
      for (let j = 0; j < n; j++) {
        if (j === qi) continue;
        if (!cantBe(j, letter, answers, markSets)) return false;
      }
      return true;
    }

    case RT_TRUE_STMT: {
      const selectedClaim = fp.optionClaims[qi][ai];
      if (!selectedClaim) return false;
      return isClaimDefinitive(selectedClaim, n, answers, markSets);
    }
  }

  return answers.slice(0, n).every((a) => a != null);
}

function isClaimDefinitive(
  claim: Claim,
  n: number,
  answers: (AnswerLetter | null)[],
  markSets?: Marks[],
): boolean {
  switch (claim.type) {
    case "count_answer": {
      for (let j = 0; j < n; j++)
        if (!isResolved(j, answers, markSets) && !cantBe(j, claim.answer, answers, markSets))
          return false;
      return true;
    }
    case "count_consonant_answers":
    case "count_vowel_answers":
      for (let j = 0; j < n; j++) if (!isResolved(j, answers, markSets)) return false;
      return true;
    case "count_answer_after": {
      for (let j = claim.afterIndex + 1; j < n; j++)
        if (!isResolved(j, answers, markSets) && !cantBe(j, claim.answer, answers, markSets))
          return false;
      return true;
    }
    case "count_answer_before": {
      for (let j = 0; j < claim.beforeIndex; j++)
        if (!isResolved(j, answers, markSets) && !cantBe(j, claim.answer, answers, markSets))
          return false;
      return true;
    }
  }
  return false;
}

function isProvablyWrong(
  rule: FlatQuestion,
  qi: number,
  answer: AnswerLetter,
  answers: (AnswerLetter | null)[],
  fp: FlatPuzzle,
): boolean {
  const ai = letterIdx(answer);
  const v = fp.optionValues[qi][ai];
  const n = fp.n;

  switch (rule.t) {
    case RT_COUNT_ANSWER:
    case RT_COUNT_ANSWER_BEFORE:
    case RT_COUNT_ANSWER_AFTER:
    case RT_COUNT_VOWEL:
    case RT_COUNT_CONSONANT: {
      if (v == null) return false;
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
      return count > v || count + remaining < v;
    }

    case RT_ANSWER_OF: {
      const target = answers[rule.questionIndex];
      if (target == null) return false;
      return v != null && letterIdx(target) !== v;
    }

    case RT_LETTER_DIST: {
      const other = answers[rule.questionIndex];
      if (other == null) return false;
      const dist = Math.abs(ai - letterIdx(other));
      return dist !== v;
    }

    case RT_CLOSEST_AFTER:
    case RT_FIRST_WITH: {
      if (v == null) {
        const start = rule.t === RT_CLOSEST_AFTER ? rule.afterIndex + 1 : 0;
        for (let j = start; j < n; j++) {
          if (answers[j] === rule.answer) return true;
        }
        return false;
      }
      if (v >= 0 && v < n && answers[v] != null && answers[v] !== rule.answer) return true;
      const start = rule.t === RT_CLOSEST_AFTER ? rule.afterIndex + 1 : 0;
      for (let j = start; j < v; j++) {
        if (answers[j] === rule.answer) return true;
      }
      return false;
    }

    case RT_CLOSEST_BEFORE:
    case RT_LAST_WITH: {
      if (v == null) {
        const end = rule.t === RT_CLOSEST_BEFORE ? rule.beforeIndex : n;
        for (let j = 0; j < end; j++) {
          if (answers[j] === rule.answer) return true;
        }
        return false;
      }
      if (v >= 0 && v < n && answers[v] != null && answers[v] !== rule.answer) return true;
      const end = rule.t === RT_CLOSEST_BEFORE ? rule.beforeIndex : n;
      for (let j = end - 1; j > v; j--) {
        if (answers[j] === rule.answer) return true;
      }
      return false;
    }

    case RT_SAME_AS: {
      if (v == null || v < 0 || v >= n) return false;
      if (answers[v] != null && answers[v] !== answer) return true;
      return false;
    }

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
