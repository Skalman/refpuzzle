import type { AnswerLetter, FlatPuzzle, FlatQuestion, Claim } from "./types.ts";
import {
  LETTERS,
  L2I,
  VOWELS,
  letterIdx,
  RT_COUNT_ANSWER,
  RT_COUNT_ANSWER_BEFORE,
  RT_COUNT_ANSWER_AFTER,
  RT_COUNT_VOWEL,
  RT_COUNT_CONSONANT,
  RT_MOST_COMMON_COUNT,
  RT_CLOSEST_AFTER,
  RT_CLOSEST_BEFORE,
  RT_FIRST_WITH,
  RT_LAST_WITH,
  RT_PREV_SAME,
  RT_NEXT_SAME,
  RT_ONLY_SAME,
  RT_SAME_AS,
  RT_ONLY_ODD,
  RT_ONLY_EVEN,
  RT_CONSEC_IDENT,
  RT_ANSWER_OF,
  RT_LEAST_COMMON,
  RT_MOST_COMMON,
  RT_UNIQUE,
  RT_EQUAL_COUNT,
  RT_SELF,
  RT_LETTER_DIST,
  RT_TRUE_STMT,
} from "./types.ts";

// Reusable scratch for letter frequency counts (avoids allocation in hot path)
const _counts = [0, 0, 0, 0, 0];
function fillCounts(answers: (AnswerLetter | null)[]): number[] {
  _counts[0] = _counts[1] = _counts[2] = _counts[3] = _counts[4] = 0;
  for (const a of answers) {
    if (a != null) _counts[letterIdx(a)]++;
  }
  return _counts;
}

function countAnswer(answers: (AnswerLetter | null)[], target: string): number {
  let c = 0;
  for (const a of answers) if (a === target) c++;
  return c;
}

function countAnswerInRange(
  answers: (AnswerLetter | null)[],
  target: string,
  from: number,
  to: number,
): number {
  let c = 0;
  for (let i = from; i < to && i < answers.length; i++) {
    if (answers[i] === target) c++;
  }
  return c;
}

function countVowels(answers: (AnswerLetter | null)[]): number {
  let c = 0;
  for (const a of answers) if (a != null && VOWELS.has(a)) c++;
  return c;
}

function countConsonants(answers: (AnswerLetter | null)[]): number {
  let c = 0;
  for (const a of answers) if (a != null && !VOWELS.has(a)) c++;
  return c;
}

export function checkQuestionAgainstSolution(
  rule: FlatQuestion,
  questionIdx: number,
  selectedAnswer: AnswerLetter,
  answers: (AnswerLetter | null)[],
  fp: FlatPuzzle,
): boolean {
  const si = letterIdx(selectedAnswer);
  const v = fp.optionValues[questionIdx][si];
  const r = rule;
  const n = fp.n;

  switch (r.t) {
    case RT_COUNT_ANSWER:
      return countAnswer(answers, r.answer!) === v;

    case RT_COUNT_ANSWER_BEFORE:
      return countAnswerInRange(answers, r.answer!, 0, r.beforeIndex) === v;

    case RT_COUNT_ANSWER_AFTER:
      return countAnswerInRange(answers, r.answer!, r.afterIndex + 1, n) === v;

    case RT_COUNT_VOWEL:
      return countVowels(answers) === v;

    case RT_COUNT_CONSONANT:
      return countConsonants(answers) === v;

    case RT_MOST_COMMON_COUNT: {
      const c = fillCounts(answers);
      let max = c[0];
      for (let i = 1; i < 5; i++) if (c[i] > max) max = c[i];
      return max === v;
    }

    case RT_CLOSEST_AFTER: {
      for (let i = r.afterIndex + 1; i < n; i++) {
        if (answers[i] === r.answer) return i === v;
      }
      return v == null;
    }

    case RT_CLOSEST_BEFORE: {
      for (let i = r.beforeIndex - 1; i >= 0; i--) {
        if (answers[i] === r.answer) return i === v;
      }
      return v == null;
    }

    case RT_FIRST_WITH: {
      for (let i = 0; i < n; i++) {
        if (answers[i] === r.answer) return i === v;
      }
      return v == null;
    }

    case RT_LAST_WITH: {
      for (let i = n - 1; i >= 0; i--) {
        if (answers[i] === r.answer) return i === v;
      }
      return v == null;
    }

    case RT_PREV_SAME: {
      for (let i = questionIdx - 1; i >= 0; i--) {
        if (answers[i] === selectedAnswer) return i === v;
      }
      return v == null;
    }

    case RT_NEXT_SAME: {
      for (let i = questionIdx + 1; i < n; i++) {
        if (answers[i] === selectedAnswer) return i === v;
      }
      return v == null;
    }

    case RT_ONLY_SAME: {
      const matches: number[] = [];
      for (let i = 0; i < n; i++) {
        if (i !== questionIdx && answers[i] === selectedAnswer) matches.push(i);
      }
      if (v == null) return matches.length === 0;
      return matches.length === 1 && matches[0] === v;
    }

    case RT_SAME_AS: {
      if (v == null || v < 0 || v >= n || answers[v] == null) return false;
      return answers[v] === selectedAnswer;
    }

    case RT_ONLY_ODD:
    case RT_ONLY_EVEN: {
      const parity = r.t === RT_ONLY_ODD ? 1 : 0;
      const matches: number[] = [];
      for (let i = 0; i < n; i++) {
        if ((i + 1) % 2 === parity && answers[i] === r.answer) matches.push(i);
      }
      if (v == null) return matches.length === 0;
      return matches.length === 1 && matches[0] === v;
    }

    case RT_CONSEC_IDENT: {
      const pairs: number[] = [];
      for (let i = 0; i < n - 1; i++) {
        if (answers[i] != null && answers[i] === answers[i + 1]) pairs.push(i);
      }
      if (v == null) return pairs.length === 0;
      return pairs.length === 1 && pairs[0] === v;
    }

    case RT_ANSWER_OF: {
      const other = answers[r.questionIndex];
      return other != null && letterIdx(other) === v;
    }

    case RT_LEAST_COMMON: {
      if (v == null) return false;
      const c = fillCounts(answers);
      let min = c[0];
      for (let i = 1; i < 5; i++) if (c[i] < min) min = c[i];
      return c[v] === min;
    }

    case RT_MOST_COMMON: {
      if (v == null) return false;
      const c = fillCounts(answers);
      let max = c[0];
      for (let i = 1; i < 5; i++) if (c[i] > max) max = c[i];
      return c[v] === max;
    }

    case RT_UNIQUE:
      return countAnswer(answers, selectedAnswer) === 1;

    case RT_EQUAL_COUNT: {
      const refCount = countAnswer(answers, r.answer!);
      if (v == null) return !LETTERS.some((l) => l !== r.answer && countAnswer(answers, l) === refCount);
      const claimed = LETTERS[v];
      return claimed !== r.answer && countAnswer(answers, claimed) === refCount;
    }

    case RT_SELF:
      return true;

    case RT_LETTER_DIST: {
      const other = answers[r.questionIndex];
      if (other == null) return false;
      const dist = Math.abs(si - letterIdx(other));
      return dist === v;
    }

    case RT_TRUE_STMT: {
      const claims = fp.optionClaims[questionIdx];
      let trueCount = 0;
      let selectedIsTrue = false;

      for (let i = 0; i < 5; i++) {
        const claim = claims[i];
        if (!claim) continue;
        const isTrue = evaluateClaim(claim, answers);
        if (isTrue) trueCount++;
        if (LETTERS[i] === selectedAnswer && isTrue) selectedIsTrue = true;
      }

      return selectedIsTrue && trueCount === 1;
    }
  }
  return false;
}

export function evaluateClaim(claim: Claim, answers: (AnswerLetter | null)[]): boolean {
  switch (claim.type) {
    case "count_answer":
      return countAnswer(answers, claim.answer) === claim.value;

    case "count_consonant_answers":
      return countConsonants(answers) === claim.value;

    case "count_vowel_answers":
      return countVowels(answers) === claim.value;

    case "count_answer_after":
      return (
        countAnswerInRange(answers, claim.answer, claim.afterIndex + 1, answers.length) ===
        claim.value
      );

    case "count_answer_before":
      return countAnswerInRange(answers, claim.answer, 0, claim.beforeIndex) === claim.value;

    case "answer_of_question":
      return answers[claim.questionIndex] === LETTERS[claim.value];

    case "first_with_answer": {
      for (let i = 0; i < answers.length; i++) {
        if (answers[i] === claim.answer) return i === claim.value;
      }
      return false;
    }

    case "last_with_answer": {
      let last = -1;
      for (let i = 0; i < answers.length; i++) {
        if (answers[i] === claim.answer) last = i;
      }
      return last === claim.value;
    }

    case "most_common_answer": {
      const counts = [0, 0, 0, 0, 0];
      for (const a of answers) {
        if (a !== null) counts[L2I[a]] += 1;
      }
      const max = Math.max(...counts);
      return counts[claim.value] === max && counts.filter((c) => c === max).length === 1;
    }
  }
  claim satisfies never;
  return false;
}
