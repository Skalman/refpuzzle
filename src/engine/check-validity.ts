import type { AnswerLetter, FlatPuzzle } from "./types.ts";
import {
  LETTERS,
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
import { evaluateClaim } from "./evaluators.ts";

export type Validity = "valid" | "invalid" | "pending";

// ── Helpers ──

type Pred = (a: AnswerLetter) => boolean;

function countMatching(
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  pred: Pred,
  matchMask: number,
  from: number,
  to: number,
): { count: number; remaining: number } {
  let count = 0;
  let remaining = 0;
  for (let i = from; i < to; i++) {
    const a = answers[i];
    if (a != null) {
      if (pred(a)) count++;
    } else if ((eliminated[i] & matchMask) !== matchMask) {
      remaining++;
    }
  }
  return { count, remaining };
}

function countValidity(count: number, remaining: number, value: number): Validity {
  if (count > value || count + remaining < value) return "invalid";
  if (count === value && remaining === 0) return "valid";
  return "pending";
}

function countRange(
  r: { t: number; afterIndex: number; beforeIndex: number },
  n: number,
): [number, number] {
  if (r.t === RT_COUNT_ANSWER_BEFORE) return [0, r.beforeIndex];
  if (r.t === RT_COUNT_ANSWER_AFTER) return [r.afterIndex + 1, n];
  return [0, n];
}

function firstInRange(
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  answer: string,
  start: number,
  end: number,
  pos: number | null,
): Validity {
  const amask = 1 << letterIdx(answer);
  if (pos != null) {
    if (pos < start || pos >= end) return "invalid";
    if (answers[pos] != null && answers[pos] !== answer) return "invalid";
    let allCertain = true;
    for (let j = start; j < pos; j++) {
      if (answers[j] === answer) return "invalid";
      if (answers[j] == null && (eliminated[j] & amask) === 0) allCertain = false;
    }
    if (answers[pos] === answer && allCertain) return "valid";
    return "pending";
  } else {
    let couldExist = false;
    for (let j = start; j < end; j++) {
      if (answers[j] === answer) return "invalid";
      if (answers[j] == null && (eliminated[j] & amask) === 0) couldExist = true;
    }
    return couldExist ? "pending" : "valid";
  }
}

function lastInRange(
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  answer: string,
  start: number,
  end: number,
  pos: number | null,
): Validity {
  const amask = 1 << letterIdx(answer);
  if (pos != null) {
    if (pos < start || pos >= end) return "invalid";
    if (answers[pos] != null && answers[pos] !== answer) return "invalid";
    let allCertain = true;
    for (let j = pos + 1; j < end; j++) {
      if (answers[j] === answer) return "invalid";
      if (answers[j] == null && (eliminated[j] & amask) === 0) allCertain = false;
    }
    if (answers[pos] === answer && allCertain) return "valid";
    return "pending";
  } else {
    let couldExist = false;
    for (let j = start; j < end; j++) {
      if (answers[j] === answer) return "invalid";
      if (answers[j] == null && (eliminated[j] & amask) === 0) couldExist = true;
    }
    return couldExist ? "pending" : "valid";
  }
}

// ── Main function ──

export function checkAnswerValidity(
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  qi: number,
): Validity {
  const a = answers[qi];
  if (a == null) return "pending";
  const ai = letterIdx(a);
  const q = fp.questions[qi];
  const v = fp.optionValues[qi][ai];
  const n = fp.n;

  // ── Counting ──
  if (q.t === RT_COUNT_ANSWER || q.t === RT_COUNT_ANSWER_BEFORE || q.t === RT_COUNT_ANSWER_AFTER) {
    if (v == null) return "pending";
    const answer = q.answer!;
    const [from, to] = countRange(q, n);
    const matchMask = 1 << letterIdx(answer);
    const cr = countMatching(answers, eliminated, (x) => x === answer, matchMask, from, to);
    return countValidity(cr.count, cr.remaining, v);
  }

  if (q.t === RT_COUNT_VOWEL) {
    if (v == null) return "pending";
    const cr = countMatching(answers, eliminated, (x) => VOWELS.has(x), 0b10001, 0, n);
    return countValidity(cr.count, cr.remaining, v);
  }

  if (q.t === RT_COUNT_CONSONANT) {
    if (v == null) return "pending";
    const cr = countMatching(answers, eliminated, (x) => !VOWELS.has(x), 0b01110, 0, n);
    return countValidity(cr.count, cr.remaining, v);
  }

  if (q.t === RT_MOST_COMMON_COUNT) {
    if (v == null) return "pending";
    const counts = [0, 0, 0, 0, 0];
    let allKnown = true;
    for (let i = 0; i < n; i++) {
      const x = answers[i];
      if (x != null) counts[letterIdx(x)]++;
      else allKnown = false;
    }
    for (let i = 0; i < 5; i++) {
      if (counts[i] > v) return "invalid";
    }
    if (!allKnown) return "pending";
    const max = Math.max(...counts);
    return max === v ? "valid" : "invalid";
  }

  // ── Positional ──
  if (q.t === RT_FIRST_WITH) return firstInRange(answers, eliminated, q.answer!, 0, n, v);
  if (q.t === RT_CLOSEST_AFTER)
    return firstInRange(answers, eliminated, q.answer!, q.afterIndex + 1, n, v);
  if (q.t === RT_LAST_WITH) return lastInRange(answers, eliminated, q.answer!, 0, n, v);
  if (q.t === RT_CLOSEST_BEFORE)
    return lastInRange(answers, eliminated, q.answer!, 0, q.beforeIndex, v);

  // ── Reference ──
  if (q.t === RT_ANSWER_OF) {
    if (v == null) return "pending";
    const target = answers[q.questionIndex];
    if (target == null) return "pending";
    return letterIdx(target) === v ? "valid" : "invalid";
  }

  if (q.t === RT_LETTER_DIST) {
    const other = answers[q.questionIndex];
    if (other == null) return "pending";
    const dist = Math.abs(ai - letterIdx(other));
    return dist === v ? "valid" : "invalid";
  }

  if (q.t === RT_SAME_AS) {
    if (v == null || v < 0 || v >= n || v === qi) return "invalid";
    const ta = answers[v];
    if (ta == null) return "pending";
    return ta === a ? "valid" : "invalid";
  }

  // ── Unique ──
  if (q.t === RT_UNIQUE) {
    const amask = 1 << ai;
    let others = 0;
    let couldMatch = 0;
    for (let j = 0; j < n; j++) {
      if (j === qi) continue;
      if (answers[j] === a) others++;
      else if (answers[j] == null && (eliminated[j] & amask) === 0) couldMatch++;
    }
    if (others > 0) return "invalid";
    if (couldMatch === 0) return "valid";
    return "pending";
  }

  // ── Previous/Next same ──
  if (q.t === RT_PREV_SAME) {
    if (v != null && (v < 0 || v >= qi)) return "invalid";
    return lastInRange(answers, eliminated, a, 0, qi, v);
  }

  if (q.t === RT_NEXT_SAME) {
    if (v != null && (v <= qi || v >= n)) return "invalid";
    return firstInRange(answers, eliminated, a, qi + 1, n, v);
  }

  // ── Only same ──
  if (q.t === RT_ONLY_SAME) {
    const amask = 1 << ai;

    if (v == null) {
      let matches = 0;
      let couldMatch = 0;
      for (let j = 0; j < n; j++) {
        if (j === qi) continue;
        if (answers[j] === a) matches++;
        else if (answers[j] == null && (eliminated[j] & amask) === 0) couldMatch++;
      }
      if (matches > 0) return "invalid";
      if (couldMatch === 0) return "valid";
      return "pending";
    }

    if (v < 0 || v >= n) return "invalid";
    if (v === qi) return "invalid";

    if (answers[v] != null && answers[v] !== a) return "invalid";

    let otherMatches = 0;
    let otherRemaining = 0;
    for (let j = 0; j < n; j++) {
      if (j === qi || j === v) continue;
      if (answers[j] === a) otherMatches++;
      else if (answers[j] == null && (eliminated[j] & amask) === 0) otherRemaining++;
    }

    if (otherMatches > 0) return "invalid";
    if (answers[v] === a && otherRemaining === 0) return "valid";
    return "pending";
  }

  // ── Consecutive identical ──
  if (q.t === RT_CONSEC_IDENT) {
    if (v != null) {
      if (v < 0 || v + 1 >= n) return "invalid";

      if (answers[v] != null && answers[v + 1] != null && answers[v] !== answers[v + 1])
        return "invalid";

      let otherPairs = 0;
      let uncertainPairs = 0;
      for (let j = 0; j < n - 1; j++) {
        if (j === v) continue;
        if (answers[j] != null && answers[j + 1] != null) {
          if (answers[j] === answers[j + 1]) otherPairs++;
        } else {
          uncertainPairs++;
        }
      }

      if (otherPairs > 0) return "invalid";
      if (
        answers[v] != null &&
        answers[v + 1] != null &&
        answers[v] === answers[v + 1] &&
        uncertainPairs === 0
      )
        return "valid";
      return "pending";
    } else {
      let anyPair = false;
      let anyUncertain = false;
      for (let j = 0; j < n - 1; j++) {
        if (answers[j] != null && answers[j + 1] != null) {
          if (answers[j] === answers[j + 1]) anyPair = true;
        } else {
          anyUncertain = true;
        }
      }
      if (anyPair) return "invalid";
      if (anyUncertain) return "pending";
      return "valid";
    }
  }

  // ── Only odd / only even ──
  if (q.t === RT_ONLY_ODD || q.t === RT_ONLY_EVEN) {
    const parity = q.t === RT_ONLY_ODD ? 1 : 0;
    const answer = q.answer!;
    const amask = 1 << letterIdx(answer);

    if (v != null) {
      if ((v + 1) % 2 !== parity) return "invalid";
      if (answers[v] != null && answers[v] !== answer) return "invalid";

      let otherMatches = 0;
      let otherRemaining = 0;
      for (let j = 0; j < n; j++) {
        if (j === v || (j + 1) % 2 !== parity) continue;
        if (answers[j] === answer) otherMatches++;
        else if (answers[j] == null && (eliminated[j] & amask) === 0) otherRemaining++;
      }

      if (otherMatches > 0) return "invalid";
      if (answers[v] === answer && otherRemaining === 0) return "valid";
      return "pending";
    } else {
      let anyMatch = false;
      let anyCould = false;
      for (let j = 0; j < n; j++) {
        if ((j + 1) % 2 !== parity) continue;
        if (answers[j] === answer) anyMatch = true;
        if (answers[j] == null && (eliminated[j] & amask) === 0) anyCould = true;
      }
      if (anyMatch) return "invalid";
      if (anyCould) return "pending";
      return "valid";
    }
  }

  // ── True statement ──
  if (q.t === RT_TRUE_STMT) {
    const allKnown = answers.slice(0, n).every((x) => x != null);
    if (!allKnown) return "pending";
    const claims = fp.optionClaims[qi];
    let trueCount = 0;
    let selectedTrue = false;
    for (let i = 0; i < 5; i++) {
      const claim = claims[i];
      if (claim && evaluateClaim(claim, answers)) {
        trueCount++;
        if (i === ai) selectedTrue = true;
      }
    }
    return selectedTrue && trueCount === 1 ? "valid" : "invalid";
  }

  // ── Always valid ──
  if (q.t === RT_SELF) return "valid";

  // ── Equal count ──
  if (q.t === RT_EQUAL_COUNT) {
    if (v != null) {
      const claimed = LETTERS[v];
      if (claimed === q.answer) return "invalid";
      const allKnown = answers.slice(0, n).every((x) => x != null);
      if (!allKnown) return "pending";
      let refCount = 0;
      let selCount = 0;
      for (let i = 0; i < n; i++) {
        if (answers[i] === q.answer) refCount++;
        if (answers[i] === claimed) selCount++;
      }
      return refCount === selCount ? "valid" : "invalid";
    } else {
      const allKnown = answers.slice(0, n).every((x) => x != null);
      if (!allKnown) return "pending";
      const counts = [0, 0, 0, 0, 0];
      for (let i = 0; i < n; i++) counts[letterIdx(answers[i]!)]++;
      const refCount = counts[letterIdx(q.answer!)];
      let anyMatch = false;
      for (let i = 0; i < 5; i++) {
        if (LETTERS[i] !== q.answer && counts[i] === refCount) anyMatch = true;
      }
      return anyMatch ? "invalid" : "valid";
    }
  }

  // ── Global: need all answers ──
  if (q.t === RT_LEAST_COMMON || q.t === RT_MOST_COMMON) {
    const allKnown = answers.slice(0, n).every((x) => x != null);
    if (!allKnown) return "pending";
    if (v == null || v < 0 || v >= 5) return "invalid";
    const counts = [0, 0, 0, 0, 0];
    for (let i = 0; i < n; i++) counts[letterIdx(answers[i]!)]++;
    if (q.t === RT_LEAST_COMMON) {
      const min = Math.min(...counts);
      return counts[v] === min && counts.filter((c) => c === min).length === 1
        ? "valid"
        : "invalid";
    } else {
      const max = Math.max(...counts);
      return counts[v] === max && counts.filter((c) => c === max).length === 1
        ? "valid"
        : "invalid";
    }
  }

  return "pending";
}

export function checkQuestionAgainstSolution(
  fp: FlatPuzzle,
  qi: number,
  _selected: AnswerLetter,
  answers: (AnswerLetter | null)[],
): boolean {
  const empty = new Array(fp.n).fill(0);
  return checkAnswerValidity(fp, answers, empty, qi) === "valid";
}
