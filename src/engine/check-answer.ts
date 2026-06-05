import type { Answer, FlatPuzzle, FlatQuestion, State, OptionPos, Claim } from "./types.ts";
import { claimAt } from "./types.ts";
import {
  LETTERS,
  L2I,
  VOWELS,
  letterIdx,
  flattenQuestion,
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
  QT_NO_OTHER_HAS_ANSWER,
  QT_EQUAL_COUNT,
  QT_ANSWER_IS_SELF,
  QT_LETTER_DIST,
  QT_TRUE_STMT,
  QT_SAME_AS_WHICH,
  isQuestionTypeWithIdentityOptions,
} from "./types.ts";
import { V_NEUTRAL, V_VALID, V_CONSISTENT, V_INVALID, V_PENDING, isValid } from "./state.ts";
import type { Validity } from "./state.ts";

// ── Helpers ──

type Pred = (a: Answer) => boolean;

function countMatching(
  answers: (Answer | null)[],
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
  if (count > value || count + remaining < value) return V_INVALID;
  if (count === value && remaining === 0) return V_VALID;
  return V_PENDING;
}

function countRange(
  r: { t: number; afterIndex: number; beforeIndex: number },
  n: number,
): [number, number] {
  if (r.t === QT_COUNT_ANSWER_BEFORE) return [0, r.beforeIndex];
  if (r.t === QT_COUNT_ANSWER_AFTER) return [r.afterIndex + 1, n];
  return [0, n];
}

function firstInRange(
  answers: (Answer | null)[],
  eliminated: number[],
  answer: string,
  start: number,
  end: number,
  pos: number | null,
): Validity {
  const amask = 1 << letterIdx(answer);
  if (pos != null) {
    if (pos < start || pos >= end) return V_INVALID;
    if (answers[pos] != null && answers[pos] !== answer) return V_INVALID;
    if (answers[pos] == null && (eliminated[pos] & amask) !== 0) return V_INVALID;
    let allCertain = true;
    for (let j = start; j < pos; j++) {
      if (answers[j] === answer) return V_INVALID;
      if (answers[j] == null && (eliminated[j] & amask) === 0) allCertain = false;
    }
    if (answers[pos] === answer && allCertain) return V_VALID;
    return V_PENDING;
  } else {
    let couldExist = false;
    for (let j = start; j < end; j++) {
      if (answers[j] === answer) return V_INVALID;
      if (answers[j] == null && (eliminated[j] & amask) === 0) couldExist = true;
    }
    return couldExist ? V_PENDING : V_VALID;
  }
}

function lastInRange(
  answers: (Answer | null)[],
  eliminated: number[],
  answer: string,
  start: number,
  end: number,
  pos: number | null,
): Validity {
  const amask = 1 << letterIdx(answer);
  if (pos != null) {
    if (pos < start || pos >= end) return V_INVALID;
    if (answers[pos] != null && answers[pos] !== answer) return V_INVALID;
    if (answers[pos] == null && (eliminated[pos] & amask) !== 0) return V_INVALID;
    let allCertain = true;
    for (let j = pos + 1; j < end; j++) {
      if (answers[j] === answer) return V_INVALID;
      if (answers[j] == null && (eliminated[j] & amask) === 0) allCertain = false;
    }
    if (answers[pos] === answer && allCertain) return V_VALID;
    return V_PENDING;
  } else {
    let couldExist = false;
    for (let j = start; j < end; j++) {
      if (answers[j] === answer) return V_INVALID;
      if (answers[j] == null && (eliminated[j] & amask) === 0) couldExist = true;
    }
    return couldExist ? V_PENDING : V_VALID;
  }
}

// ── Core validity check ──

function checkValueValidityInner(
  q: FlatQuestion,
  v: number | null,
  a: Answer,
  qi: number,
  answers: (Answer | null)[],
  eliminated: number[],
  n: number,
  optionCount: number,
): Validity {
  const ai = letterIdx(a);

  // ── Counting ──
  if (q.t === QT_COUNT_ANSWER || q.t === QT_COUNT_ANSWER_BEFORE || q.t === QT_COUNT_ANSWER_AFTER) {
    if (v == null) return V_INVALID;
    const answer = q.answer!;
    const [from, to] = countRange(q, n);
    const matchMask = 1 << letterIdx(answer);
    const cr = countMatching(answers, eliminated, (x) => x === answer, matchMask, from, to);
    return countValidity(cr.count, cr.remaining, v);
  }

  if (q.t === QT_COUNT_VOWEL) {
    if (v == null) return V_INVALID;
    const cr = countMatching(answers, eliminated, (x) => VOWELS.has(x), 0b10001, 0, n);
    return countValidity(cr.count, cr.remaining, v);
  }

  if (q.t === QT_COUNT_CONSONANT) {
    if (v == null) return V_INVALID;
    const cr = countMatching(answers, eliminated, (x) => !VOWELS.has(x), 0b01110, 0, n);
    return countValidity(cr.count, cr.remaining, v);
  }

  if (q.t === QT_MOST_COMMON_COUNT) {
    if (v == null) return V_INVALID;
    const counts = [0, 0, 0, 0, 0];
    let allKnown = true;
    for (let i = 0; i < n; i++) {
      const x = answers[i];
      if (x != null) counts[letterIdx(x)]++;
      else allKnown = false;
    }
    for (let i = 0; i < 5; i++) {
      if (counts[i] > v) return V_INVALID;
    }
    if (!allKnown) return V_PENDING;
    const max = Math.max(...counts);
    return max === v ? V_VALID : V_INVALID;
  }

  // ── Positional ──
  if (q.t === QT_FIRST_WITH) return firstInRange(answers, eliminated, q.answer!, 0, n, v);
  if (q.t === QT_CLOSEST_AFTER)
    return firstInRange(answers, eliminated, q.answer!, q.afterIndex + 1, n, v);
  if (q.t === QT_LAST_WITH) return lastInRange(answers, eliminated, q.answer!, 0, n, v);
  if (q.t === QT_CLOSEST_BEFORE)
    return lastInRange(answers, eliminated, q.answer!, 0, q.beforeIndex, v);

  // ── Reference ──
  if (q.t === QT_ANSWER_OF) {
    if (v == null || v < 0 || v > 4) return V_INVALID;
    const target = answers[q.questionIndex];
    if (target == null) return V_PENDING;
    return letterIdx(target) === v ? V_VALID : V_INVALID;
  }

  if (q.t === QT_LETTER_DIST) {
    if (v == null) return V_PENDING;
    const other = answers[q.questionIndex];
    if (other == null) return V_PENDING;
    const dist = Math.abs(ai - letterIdx(other));
    return dist === v ? V_VALID : V_INVALID;
  }

  if (q.t === QT_SAME_AS) {
    if (v == null) {
      // "none": valid iff no other question shares qi's (candidate) answer a.
      const amask = 1 << ai;
      let couldExist = false;
      for (let j = 0; j < n; j++) {
        if (j === qi) continue;
        if (answers[j] === a) return V_INVALID;
        if (answers[j] == null && (eliminated[j] & amask) === 0) couldExist = true;
      }
      return couldExist ? V_PENDING : V_VALID;
    }
    if (v < 0 || v >= n || v === qi) return V_INVALID;
    const ta = answers[v];
    if (ta == null) return V_PENDING;
    return ta === a ? V_VALID : V_INVALID;
  }

  if (q.t === QT_SAME_AS_WHICH) {
    if (v == null || v < 0 || v >= n || v === qi || v === q.questionIndex) return V_INVALID;
    const refAns = answers[q.questionIndex];
    if (refAns == null) return V_PENDING;
    const targetAns = answers[v];
    if (targetAns == null) return V_PENDING;
    return targetAns === refAns ? V_VALID : V_INVALID;
  }

  // ── NoOtherHasAnswer ──
  if (q.t === QT_NO_OTHER_HAS_ANSWER) {
    if (v == null || v < 0 || v > 4) return V_INVALID;
    const letter = LETTERS[v];
    const amask = 1 << v;
    let others = 0;
    let couldMatch = 0;
    for (let j = 0; j < n; j++) {
      if (j === qi) continue;
      if (answers[j] === letter) others++;
      else if (answers[j] == null && (eliminated[j] & amask) === 0) couldMatch++;
    }
    if (others > 0) return V_INVALID;
    if (couldMatch === 0) return V_VALID;
    return V_PENDING;
  }

  // ── Previous/Next same ──
  if (q.t === QT_PREV_SAME) {
    if (v != null && (v < 0 || v >= qi)) return V_INVALID;
    return lastInRange(answers, eliminated, a, 0, qi, v);
  }

  if (q.t === QT_NEXT_SAME) {
    if (v != null && (v <= qi || v >= n)) return V_INVALID;
    return firstInRange(answers, eliminated, a, qi + 1, n, v);
  }

  // ── Only same ──
  if (q.t === QT_ONLY_SAME) {
    const amask = 1 << ai;

    if (v == null) {
      let matches = 0;
      let couldMatch = 0;
      for (let j = 0; j < n; j++) {
        if (j === qi) continue;
        if (answers[j] === a) matches++;
        else if (answers[j] == null && (eliminated[j] & amask) === 0) couldMatch++;
      }
      if (matches > 0) return V_INVALID;
      if (couldMatch === 0) return V_VALID;
      return V_PENDING;
    }

    if (v < 0 || v >= n) return V_INVALID;
    if (v === qi) return V_INVALID;

    if (answers[v] != null && answers[v] !== a) return V_INVALID;

    let otherMatches = 0;
    let otherRemaining = 0;
    for (let j = 0; j < n; j++) {
      if (j === qi || j === v) continue;
      if (answers[j] === a) otherMatches++;
      else if (answers[j] == null && (eliminated[j] & amask) === 0) otherRemaining++;
    }

    if (otherMatches > 0) return V_INVALID;
    if (answers[v] === a && otherRemaining === 0) return V_VALID;
    return V_PENDING;
  }

  // ── Consecutive identical ──
  if (q.t === QT_CONSEC_IDENT) {
    if (v != null) {
      if (v < 0 || v + 1 >= n) return V_INVALID;

      if (answers[v] != null && answers[v + 1] != null && answers[v] !== answers[v + 1])
        return V_INVALID;

      const possA = ~eliminated[v] & 0b11111;
      const possB = ~eliminated[v + 1] & 0b11111;
      if ((possA & possB) === 0) return V_INVALID;
      if (answers[v] != null && (eliminated[v + 1] & (1 << letterIdx(answers[v]))) !== 0)
        return V_INVALID;
      if (answers[v + 1] != null && (eliminated[v] & (1 << letterIdx(answers[v + 1]!))) !== 0)
        return V_INVALID;

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

      if (otherPairs > 0) return V_INVALID;
      if (
        answers[v] != null &&
        answers[v + 1] != null &&
        answers[v] === answers[v + 1] &&
        uncertainPairs === 0
      )
        return V_VALID;
      return V_PENDING;
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
      if (anyPair) return V_INVALID;
      if (anyUncertain) return V_PENDING;
      return V_VALID;
    }
  }

  // ── Only odd / only even ──
  if (q.t === QT_ONLY_ODD || q.t === QT_ONLY_EVEN) {
    const parity = q.t === QT_ONLY_ODD ? 1 : 0;
    const answer = q.answer!;
    const amask = 1 << letterIdx(answer);

    if (v != null) {
      if ((v + 1) % 2 !== parity) return V_INVALID;
      if (answers[v] != null && answers[v] !== answer) return V_INVALID;

      let otherMatches = 0;
      let otherRemaining = 0;
      for (let j = 0; j < n; j++) {
        if (j === v || (j + 1) % 2 !== parity) continue;
        if (answers[j] === answer) otherMatches++;
        else if (answers[j] == null && (eliminated[j] & amask) === 0) otherRemaining++;
      }

      if (otherMatches > 0) return V_INVALID;
      if (answers[v] === answer && otherRemaining === 0) return V_VALID;
      return V_PENDING;
    } else {
      let anyMatch = false;
      let anyCould = false;
      for (let j = 0; j < n; j++) {
        if ((j + 1) % 2 !== parity) continue;
        if (answers[j] === answer) anyMatch = true;
        if (answers[j] == null && (eliminated[j] & amask) === 0) anyCould = true;
      }
      if (anyMatch) return V_INVALID;
      if (anyCould) return V_PENDING;
      return V_VALID;
    }
  }

  // ── Always valid ──
  if (q.t === QT_ANSWER_IS_SELF) return V_VALID;

  // ── Equal count ──
  if (q.t === QT_EQUAL_COUNT) {
    if (v != null) {
      const claimed = LETTERS[v];
      if (claimed === q.answer) return V_INVALID;
      const refMask = 1 << letterIdx(q.answer!);
      const claimedMask = 1 << v;
      const rc = countMatching(answers, eliminated, (x) => x === q.answer, refMask, 0, n);
      const sc = countMatching(answers, eliminated, (x) => x === claimed, claimedMask, 0, n);
      if (rc.count + rc.remaining < sc.count || sc.count + sc.remaining < rc.count)
        return V_INVALID;
      if (rc.remaining === 0 && sc.remaining === 0)
        return rc.count === sc.count ? V_VALID : V_INVALID;
      return V_PENDING;
    } else {
      const allKnown = answers.slice(0, n).every((x) => x != null);
      if (!allKnown) return V_PENDING;
      const counts = [0, 0, 0, 0, 0];
      for (let i = 0; i < n; i++) counts[letterIdx(answers[i]!)]++;
      const refCount = counts[letterIdx(q.answer!)];
      let anyMatch = false;
      for (let i = 0; i < 5; i++) {
        if (LETTERS[i] !== q.answer && counts[i] === refCount) anyMatch = true;
      }
      return anyMatch ? V_INVALID : V_VALID;
    }
  }

  // ── Global: need all answers ──
  if (q.t === QT_LEAST_COMMON || q.t === QT_MOST_COMMON) {
    const allKnown = answers.slice(0, n).every((x) => x != null);
    if (!allKnown) return V_PENDING;
    if (v == null || v < 0 || v >= optionCount) return V_INVALID;
    const counts = [0, 0, 0, 0, 0];
    for (let i = 0; i < n; i++) counts[letterIdx(answers[i]!)]++;
    const active = counts.slice(0, optionCount);
    if (q.t === QT_LEAST_COMMON) {
      const min = Math.min(...active);
      return counts[v] === min && active.filter((c) => c === min).length === 1
        ? V_VALID
        : V_INVALID;
    } else {
      const max = Math.max(...active);
      return counts[v] === max && active.filter((c) => c === max).length === 1
        ? V_VALID
        : V_INVALID;
    }
  }

  // TrueStmt can't be checked via checkClaim
  if (q.t === QT_TRUE_STMT) return V_PENDING;

  return V_PENDING;
}

export function checkClaim(fp: FlatPuzzle, state: State, opt: OptionPos, claim: Claim): Validity {
  const { qi, oi } = opt;
  const a = LETTERS[oi];
  const { answers, eliminated } = state;
  const fq = flattenQuestion(claim.questionType);
  const v = claim.value === -1 ? null : claim.value;
  return checkValueValidityInner(fq, v, a, qi, answers, eliminated, fp.n, fp.optionCount);
}

// ── Answer-level validity (delegates to checkValueValidityInner) ──

export function checkAnswer(fp: FlatPuzzle, state: State, qi: number): Validity {
  const { answers, eliminated } = state;
  const a = answers[qi];
  if (a == null) {
    const oc = fp.optionCount;
    const allElim = (~eliminated[qi] & ((1 << oc) - 1)) === 0;
    if (allElim) return V_INVALID;
    return V_NEUTRAL;
  }
  const ai = letterIdx(a);
  const q = fp.questions[qi];
  const n = fp.n;

  if (q.t === QT_TRUE_STMT) {
    const selectedClaim = claimAt(fp, qi, ai);
    if (!selectedClaim) return V_INVALID;

    const selectedV = checkClaim(fp, state, { qi, oi: ai }, selectedClaim);

    if (selectedV !== V_VALID) return selectedV;

    // Check if claim holds for ALL possible answers (PROVEN) or only this one (CONSISTENT).
    // Temporarily substitute each alternative answer to test — restored before returning.
    const savedAnswer = answers[qi];
    for (let oi = 0; oi < 5; oi++) {
      if (oi === ai) continue;
      answers[qi] = LETTERS[oi];
      const v = checkClaim(fp, state, { qi, oi }, selectedClaim);
      answers[qi] = savedAnswer;
      if (v !== V_VALID) return V_CONSISTENT;
    }
    return V_VALID;
  }

  if (isQuestionTypeWithIdentityOptions(q.t)) {
    const result = checkValueValidityInner(q, ai, a, qi, answers, eliminated, n, fp.optionCount);
    return result === V_VALID && affectedByOwnAnswer(q, qi) ? V_CONSISTENT : result;
  }

  const v = fp.optionValues[qi][ai];
  const result = checkValueValidityInner(q, v, a, qi, answers, eliminated, n, fp.optionCount);
  return result === V_VALID && affectedByOwnAnswer(q, qi) ? V_CONSISTENT : result;
}

function affectedByOwnAnswer(q: FlatQuestion, qi: number): boolean {
  if (q.t === QT_ANSWER_OF || q.t === QT_SAME_AS_WHICH) return q.questionIndex === qi;
  return true;
}

export function checkAnswers(fp: FlatPuzzle, answers: (Answer | null)[]): boolean {
  const state: State = { answers, eliminated: new Array(fp.n).fill(0) };
  for (let i = 0; i < fp.n; i++) {
    if (!isValid(checkAnswer(fp, state, i))) return false;
  }
  return true;
}

/** Like checkClaim, but assumes answers is fully populated; returns bool. */
export function checkClaimFast(
  optionCount: number,
  answers: Answer[],
  qi: number,
  claim: Claim,
): boolean {
  const qt = claim.questionType;
  const value = claim.value;
  const n = answers.length;

  switch (qt.type) {
    case "CountAnswer":
      return answers.filter((a) => a === qt.answer).length === value;
    case "CountConsonant":
      return answers.filter((a) => !VOWELS.has(a)).length === value;
    case "CountVowel":
      return answers.filter((a) => VOWELS.has(a)).length === value;
    case "CountAnswerAfter":
      return answers.slice(qt.afterIndex + 1).filter((a) => a === qt.answer).length === value;
    case "CountAnswerBefore":
      return answers.slice(0, qt.beforeIndex).filter((a) => a === qt.answer).length === value;
    case "AnswerOf":
      return answers[qt.questionIndex] === LETTERS[value];
    case "FirstWith": {
      const i = answers.indexOf(qt.answer);
      return i >= 0 ? i === value : value === -1;
    }
    case "LastWith": {
      const i = answers.lastIndexOf(qt.answer);
      return i >= 0 ? i === value : value === -1;
    }
    case "MostCommon": {
      if (value < 0 || value >= optionCount) return false;
      const counts = [0, 0, 0, 0, 0];
      for (const a of answers) counts[L2I[a]]++;
      const active = counts.slice(0, optionCount);
      const max = Math.max(...active);
      return counts[value] === max && active.filter((c) => c === max).length === 1;
    }
    case "LeastCommon": {
      if (value < 0 || value >= optionCount) return false;
      const counts = [0, 0, 0, 0, 0];
      for (const a of answers) counts[L2I[a]]++;
      const active = counts.slice(0, optionCount);
      const min = Math.min(...active);
      return counts[value] === min && active.filter((c) => c === min).length === 1;
    }
    case "MostCommonCount": {
      const counts = [0, 0, 0, 0, 0];
      for (const a of answers) counts[L2I[a]]++;
      return Math.max(...counts) === value;
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
    case "NoOtherHasAnswer": {
      const letter = LETTERS[value];
      for (let i = 0; i < n; i++) {
        if (i !== qi && answers[i] === letter) return false;
      }
      return true;
    }
    case "EqualCount": {
      if (value < 0 || value >= optionCount) return false;
      const refCount = answers.filter((a) => a === qt.answer).length;
      const counts = [0, 0, 0, 0, 0];
      for (const a of answers) counts[L2I[a]]++;
      return counts[value] === refCount && value !== L2I[qt.answer];
    }
    case "ConsecIdent": {
      for (let i = 0; i < n - 1; i++) {
        if (answers[i] === answers[i + 1]) return i === value;
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
      for (let i = qi - 1; i >= 0; i--) {
        if (answers[i] === selfAns) return i === value;
      }
      return value === -1;
    }
    case "NextSame": {
      const selfAns = answers[qi];
      for (let i = qi + 1; i < n; i++) {
        if (answers[i] === selfAns) return i === value;
      }
      return value === -1;
    }
    case "OnlySame": {
      const selfAns = answers[qi];
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
      const anyMatch = answers.some((x, i) => i !== qi && x === selfAns);
      if (!anyMatch) return value === -1;
      return value >= 0 && value < n && value !== qi && answers[value] === selfAns;
    }
    case "SameAsWhich": {
      const refAns = answers[qt.questionIndex];
      return (
        value >= 0 &&
        value < n &&
        value !== qi &&
        value !== qt.questionIndex &&
        answers[value] === refAns
      );
    }
    case "LetterDist":
      return Math.abs(L2I[answers[qi]] - L2I[answers[qt.questionIndex]]) === value;
    case "AnswerIsSelf":
    case "TrueStmt":
      return false;
  }
  qt satisfies never;
  return false;
}
