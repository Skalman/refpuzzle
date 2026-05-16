import type { Answer, FlatPuzzle, FlatQuestion, Claim } from "./types.ts";
import {
  LETTERS,
  L2I,
  VOWELS,
  letterIdx,
  QT_COUNT_ANSWER,
  QT_COUNT_ANSWER_BEFORE,
  QT_COUNT_ANSWER_AFTER,
  QT_COUNT_VOWEL,
  QT_COUNT_CONSONANT,
  QT_MOST_COMMON_COUNT,
  QT_CLOSEST_AFTER,
  QT_CLOSEST_BEFORE,
  QT_FIRST_WITH,
  QT_LAST_WITH,
  QT_PREV_SAME,
  QT_NEXT_SAME,
  QT_ONLY_SAME,
  QT_SAME_AS,
  QT_ONLY_ODD,
  QT_ONLY_EVEN,
  QT_CONSEC_IDENT,
  QT_ANSWER_OF,
  QT_LEAST_COMMON,
  QT_MOST_COMMON,
  QT_UNIQUE,
  QT_EQUAL_COUNT,
  QT_ANSWER_IS_SELF,
  QT_LETTER_DIST,
  QT_TRUE_STMT,
  QT_SAME_AS_WHICH,
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
    case QT_COUNT_ANSWER:
      return countAnswer(answers, q.answer!) === v;

    case QT_COUNT_ANSWER_BEFORE:
      return countAnswerInRange(answers, q.answer!, 0, q.beforeIndex) === v;

    case QT_COUNT_ANSWER_AFTER:
      return countAnswerInRange(answers, q.answer!, q.afterIndex + 1, n) === v;

    case QT_COUNT_VOWEL:
      return countVowels(answers) === v;

    case QT_COUNT_CONSONANT:
      return countConsonants(answers) === v;

    case QT_MOST_COMMON_COUNT: {
      const c = fillCounts(answers);
      let max = c[0];
      for (let i = 1; i < 5; i++) if (c[i] > max) max = c[i];
      return max === v;
    }

    case QT_CLOSEST_AFTER: {
      for (let i = q.afterIndex + 1; i < n; i++) {
        if (answers[i] === q.answer) return i === v;
      }
      return v == null;
    }

    case QT_CLOSEST_BEFORE: {
      for (let i = q.beforeIndex - 1; i >= 0; i--) {
        if (answers[i] === q.answer) return i === v;
      }
      return v == null;
    }

    case QT_FIRST_WITH: {
      for (let i = 0; i < n; i++) {
        if (answers[i] === q.answer) return i === v;
      }
      return v == null;
    }

    case QT_LAST_WITH: {
      for (let i = n - 1; i >= 0; i--) {
        if (answers[i] === q.answer) return i === v;
      }
      return v == null;
    }

    case QT_PREV_SAME: {
      for (let i = questionIdx - 1; i >= 0; i--) {
        if (answers[i] === selectedAnswer) return i === v;
      }
      return v == null;
    }

    case QT_NEXT_SAME: {
      for (let i = questionIdx + 1; i < n; i++) {
        if (answers[i] === selectedAnswer) return i === v;
      }
      return v == null;
    }

    case QT_ONLY_SAME: {
      const matches: number[] = [];
      for (let i = 0; i < n; i++) {
        if (i !== questionIdx && answers[i] === selectedAnswer) matches.push(i);
      }
      if (v == null) return matches.length === 0;
      return matches.length === 1 && matches[0] === v;
    }

    case QT_SAME_AS: {
      if (v == null || v < 0 || v >= n || answers[v] == null) return false;
      return answers[v] === selectedAnswer;
    }

    case QT_SAME_AS_WHICH: {
      if (v == null || v < 0 || v >= n || v === questionIdx || v === q.questionIndex) return false;
      const refAns = answers[q.questionIndex];
      if (refAns == null) return false;
      return answers[v] === refAns;
    }

    case QT_ONLY_ODD:
    case QT_ONLY_EVEN: {
      const parity = q.t === QT_ONLY_ODD ? 1 : 0;
      const matches: number[] = [];
      for (let i = 0; i < n; i++) {
        if ((i + 1) % 2 === parity && answers[i] === q.answer) matches.push(i);
      }
      if (v == null) return matches.length === 0;
      return matches.length === 1 && matches[0] === v;
    }

    case QT_CONSEC_IDENT: {
      const pairs: number[] = [];
      for (let i = 0; i < n - 1; i++) {
        if (answers[i] != null && answers[i] === answers[i + 1]) pairs.push(i);
      }
      if (v == null) return pairs.length === 0;
      return pairs.length === 1 && pairs[0] === v;
    }

    case QT_ANSWER_OF: {
      const other = answers[q.questionIndex];
      return other != null && letterIdx(other) === v;
    }

    case QT_LEAST_COMMON: {
      if (v == null || v < 0 || v >= fp.optionCount) return false;
      const c = fillCounts(answers);
      const active = c.slice(0, fp.optionCount);
      const min = Math.min(...active);
      return c[v] === min && active.filter((x) => x === min).length === 1;
    }

    case QT_MOST_COMMON: {
      if (v == null || v < 0 || v >= fp.optionCount) return false;
      const c = fillCounts(answers);
      const active = c.slice(0, fp.optionCount);
      const max = Math.max(...active);
      return c[v] === max && active.filter((x) => x === max).length === 1;
    }

    case QT_UNIQUE:
      return countAnswer(answers, selectedAnswer) === 1;

    case QT_EQUAL_COUNT: {
      const refCount = countAnswer(answers, q.answer!);
      if (v == null)
        return !LETTERS.some((l) => l !== q.answer && countAnswer(answers, l) === refCount);
      const claimed = LETTERS[v];
      return claimed !== q.answer && countAnswer(answers, claimed) === refCount;
    }

    case QT_ANSWER_IS_SELF:
      return true;

    case QT_LETTER_DIST: {
      const other = answers[q.questionIndex];
      if (other == null) return false;
      const dist = Math.abs(si - letterIdx(other));
      return dist === v;
    }

    case QT_TRUE_STMT: {
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
  throw new Error(`unhandled question type: ${q.t}`);
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
