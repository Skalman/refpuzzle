import type { Answer, FlatPuzzle, FlatQuestion, Claim } from "./types.ts";
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
const reusableCounts = [0, 0, 0, 0, 0];
function fillCounts(answers: (Answer | null)[]): number[] {
  reusableCounts[0] =
    reusableCounts[1] =
    reusableCounts[2] =
    reusableCounts[3] =
    reusableCounts[4] =
      0;
  for (const a of answers) {
    if (a != null) reusableCounts[letterIdx(a)]++;
  }
  return reusableCounts;
}

function countAnswer(answers: (Answer | null)[], target: string): number {
  let c = 0;
  for (const a of answers) if (a === target) c++;
  return c;
}

function countAnswerInRange(
  answers: (Answer | null)[],
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

function countVowels(answers: (Answer | null)[]): number {
  let c = 0;
  for (const a of answers) if (a != null && VOWELS.has(a)) c++;
  return c;
}

function countConsonants(answers: (Answer | null)[]): number {
  let c = 0;
  for (const a of answers) if (a != null && !VOWELS.has(a)) c++;
  return c;
}

export function checkQuestionAgainstSolution(
  question: FlatQuestion,
  questionIdx: number,
  selectedAnswer: Answer,
  answers: (Answer | null)[],
  fp: FlatPuzzle,
): boolean {
  const si = letterIdx(selectedAnswer);
  const v = fp.optionValues[questionIdx][si];
  const q = question;
  const n = fp.n;

  switch (q.t) {
    case RT_COUNT_ANSWER:
      return countAnswer(answers, q.answer!) === v;

    case RT_COUNT_ANSWER_BEFORE:
      return countAnswerInRange(answers, q.answer!, 0, q.beforeIndex) === v;

    case RT_COUNT_ANSWER_AFTER:
      return countAnswerInRange(answers, q.answer!, q.afterIndex + 1, n) === v;

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
      for (let i = q.afterIndex + 1; i < n; i++) {
        if (answers[i] === q.answer) return i === v;
      }
      return v == null;
    }

    case RT_CLOSEST_BEFORE: {
      for (let i = q.beforeIndex - 1; i >= 0; i--) {
        if (answers[i] === q.answer) return i === v;
      }
      return v == null;
    }

    case RT_FIRST_WITH: {
      for (let i = 0; i < n; i++) {
        if (answers[i] === q.answer) return i === v;
      }
      return v == null;
    }

    case RT_LAST_WITH: {
      for (let i = n - 1; i >= 0; i--) {
        if (answers[i] === q.answer) return i === v;
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
      const parity = q.t === RT_ONLY_ODD ? 1 : 0;
      const matches: number[] = [];
      for (let i = 0; i < n; i++) {
        if ((i + 1) % 2 === parity && answers[i] === q.answer) matches.push(i);
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
      const other = answers[q.questionIndex];
      return other != null && letterIdx(other) === v;
    }

    case RT_LEAST_COMMON: {
      if (v == null) return false;
      const c = fillCounts(answers);
      let min = c[0];
      for (let i = 1; i < 5; i++) if (c[i] < min) min = c[i];
      return c[v] === min && c.filter((x) => x === min).length === 1;
    }

    case RT_MOST_COMMON: {
      if (v == null) return false;
      const c = fillCounts(answers);
      let max = c[0];
      for (let i = 1; i < 5; i++) if (c[i] > max) max = c[i];
      return c[v] === max && c.filter((x) => x === max).length === 1;
    }

    case RT_UNIQUE:
      return countAnswer(answers, selectedAnswer) === 1;

    case RT_EQUAL_COUNT: {
      const refCount = countAnswer(answers, q.answer!);
      if (v == null)
        return !LETTERS.some((l) => l !== q.answer && countAnswer(answers, l) === refCount);
      const claimed = LETTERS[v];
      return claimed !== q.answer && countAnswer(answers, claimed) === refCount;
    }

    case RT_SELF:
      return true;

    case RT_LETTER_DIST: {
      const other = answers[q.questionIndex];
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
        const isTrue = evaluateClaim(claim, questionIdx, answers);
        if (isTrue) trueCount++;
        if (LETTERS[i] === selectedAnswer && isTrue) selectedIsTrue = true;
      }

      return selectedIsTrue && trueCount === 1;
    }
  }
  return false;
}

export function evaluateClaim(claim: Claim, qi: number, answers: (Answer | null)[]): boolean {
  const qt = claim.questionType;
  const value = claim.value;
  const n = answers.length;

  switch (qt.type) {
    case "CountAnswer":
      return countAnswer(answers, qt.answer) === value;
    case "CountConsonant":
      return countConsonants(answers) === value;
    case "CountVowel":
      return countVowels(answers) === value;
    case "CountAnswerAfter":
      return countAnswerInRange(answers, qt.answer, qt.afterIndex + 1, n) === value;
    case "CountAnswerBefore":
      return countAnswerInRange(answers, qt.answer, 0, qt.beforeIndex) === value;
    case "AnswerOf":
      return value >= 0 && value <= 4 && answers[qt.questionIndex] === LETTERS[value];
    case "FirstWith": {
      for (let i = 0; i < n; i++) {
        if (answers[i] === qt.answer) return i === value;
      }
      return false;
    }
    case "LastWith": {
      let last = -1;
      for (let i = 0; i < n; i++) {
        if (answers[i] === qt.answer) last = i;
      }
      return last === value;
    }
    case "MostCommon": {
      if (value < 0 || value > 4) return false;
      const counts = [0, 0, 0, 0, 0];
      for (const a of answers) {
        if (a !== null) counts[L2I[a]] += 1;
      }
      const max = Math.max(...counts);
      return counts[value] === max && counts.filter((c) => c === max).length === 1;
    }
    case "ClosestAfter": {
      for (let i = qt.afterIndex + 1; i < n; i++) {
        if (answers[i] === qt.answer) return i === value;
      }
      return value === -1;
    }
    case "ClosestBefore": {
      for (let i = qt.beforeIndex - 1; i >= 0; i--) {
        if (answers[i] === qt.answer) return i === value;
      }
      return value === -1;
    }
    case "MostCommonCount": {
      const counts = [0, 0, 0, 0, 0];
      for (const a of answers) {
        if (a !== null) counts[L2I[a]] += 1;
      }
      return Math.max(...counts) === value;
    }
    case "LeastCommon": {
      if (value < 0 || value > 4) return false;
      const counts = [0, 0, 0, 0, 0];
      for (const a of answers) {
        if (a !== null) counts[L2I[a]] += 1;
      }
      const min = Math.min(...counts);
      return counts[value] === min && counts.filter((c) => c === min).length === 1;
    }
    case "Unique": {
      if (value < 0 || value > 4) return false;
      const counts = [0, 0, 0, 0, 0];
      for (const a of answers) {
        if (a !== null) counts[L2I[a]] += 1;
      }
      return counts[value] === 1 && counts.filter((c) => c === 1).length === 1;
    }
    case "EqualCount": {
      if (value < 0 || value > 4) return false;
      const refCount = countAnswer(answers, qt.answer);
      const counts = [0, 0, 0, 0, 0];
      for (const a of answers) {
        if (a !== null) counts[L2I[a]] += 1;
      }
      return counts[value] === refCount && value !== L2I[qt.answer];
    }
    case "ConsecIdent": {
      for (let i = 0; i < n - 1; i++) {
        if (answers[i] !== null && answers[i] === answers[i + 1]) return i === value;
      }
      return value === -1;
    }
    case "OnlyOdd":
    case "OnlyEven": {
      const parity = qt.type === "OnlyEven" ? 1 : 0;
      let found = -1;
      let count = 0;
      for (let i = 0; i < n; i++) {
        if (i % 2 === parity && answers[i] === qt.answer) {
          found = i;
          count++;
        }
      }
      return count === 1 && found === value;
    }
    case "PrevSame": {
      const selfAns = answers[qi];
      if (selfAns == null) return false;
      for (let i = qi - 1; i >= 0; i--) {
        if (answers[i] === selfAns) return i === value;
      }
      return value === -1;
    }
    case "NextSame": {
      const selfAns = answers[qi];
      if (selfAns == null) return false;
      for (let i = qi + 1; i < n; i++) {
        if (answers[i] === selfAns) return i === value;
      }
      return value === -1;
    }
    case "OnlySame": {
      const selfAns = answers[qi];
      if (selfAns == null) return false;
      let found = -1;
      let count = 0;
      for (let i = 0; i < n; i++) {
        if (i !== qi && answers[i] === selfAns) {
          found = i;
          count++;
        }
      }
      if (count === 0) return value === -1;
      return count === 1 && found === value;
    }
    case "SameAs": {
      const selfAns = answers[qi];
      if (selfAns == null) return false;
      return value >= 0 && value < n && value !== qi && answers[value] === selfAns;
    }
    case "SameAsWhich": {
      const refAns = answers[qt.questionIndex];
      if (refAns == null) return false;
      return (
        value >= 0 &&
        value < n &&
        value !== qi &&
        value !== qt.questionIndex &&
        answers[value] === refAns
      );
    }
    case "LetterDist": {
      const selfAns = answers[qi];
      const other = answers[qt.questionIndex];
      if (selfAns == null || other == null) return false;
      return Math.abs(L2I[selfAns] - L2I[other]) === value;
    }
    case "AnswerIsSelf":
    case "TrueStmt":
      return false;
  }
  qt satisfies never;
  return false;
}
