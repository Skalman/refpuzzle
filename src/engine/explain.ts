import type { AnswerLetter, FlatPuzzle, Puzzle } from "./types.ts";
import {
  LETTERS,
  VOWELS,
  letterIdx,
  L2I,
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
  RT_UNIQUE,
  RT_LETTER_DIST,
} from "./types.ts";
import { renderOptionLabel } from "./render.ts";
import type { DeduceResult } from "./deduce.ts";
import type { LookaheadResult } from "./lookahead.ts";

// ── Formatting helpers ──

function Q(i: number): string {
  return `Q${i + 1}`;
}

function optLabel(puzzle: Puzzle, qi: number, oi: number): string {
  const qt = puzzle.questions[qi].questionType;
  const v = puzzle.questions[qi].options[oi].value;
  return renderOptionLabel(qt, v, qi);
}

type Pred = (a: AnswerLetter) => boolean;

function countPred(r: { t: number; answer: string | null }): Pred | null {
  switch (r.t) {
    case RT_COUNT_ANSWER:
    case RT_COUNT_ANSWER_BEFORE:
    case RT_COUNT_ANSWER_AFTER:
      return (a) => a === r.answer;
    case RT_COUNT_VOWEL:
      return (a) => VOWELS.has(a);
    case RT_COUNT_CONSONANT:
      return (a) => !VOWELS.has(a);
    default:
      return null;
  }
}

function countRange(
  r: { t: number; afterIndex: number; beforeIndex: number },
  n: number,
): [number, number] {
  if (r.t === RT_COUNT_ANSWER_BEFORE) return [0, r.beforeIndex];
  if (r.t === RT_COUNT_ANSWER_AFTER) return [r.afterIndex + 1, n];
  return [0, n];
}

function countAnswers(
  answers: (AnswerLetter | null)[],
  pred: Pred,
  from: number,
  to: number,
): { count: number; remaining: number } {
  let count = 0;
  let remaining = 0;
  for (let i = from; i < to; i++) {
    const a = answers[i];
    if (a == null) remaining++;
    else if (pred(a)) count++;
  }
  return { count, remaining };
}

function countRuleLabel(
  r: {
    t: number;
    answer: string | null;
    afterIndex: number;
    beforeIndex: number;
  },
  count: number,
): string {
  const q = count === 1 ? "question" : "questions";
  switch (r.t) {
    case RT_COUNT_ANSWER:
      return `${q} with answer ${r.answer}`;
    case RT_COUNT_ANSWER_BEFORE:
      return `${q} before #${r.beforeIndex + 1} with answer ${r.answer}`;
    case RT_COUNT_ANSWER_AFTER:
      return `${q} after #${r.afterIndex + 1} with answer ${r.answer}`;
    case RT_COUNT_VOWEL:
      return `${q} with a vowel answer`;
    case RT_COUNT_CONSONANT:
      return `${q} with a consonant answer`;
    default:
      return `matching ${q}`;
  }
}

function isElim(eliminated: number[], qi: number, oi: number): boolean {
  return ((eliminated[qi] >> oi) & 1) === 1;
}

function remainingCount(eliminated: number): number {
  let c = 0;
  for (let i = 0; i < 5; i++) if (((eliminated >> i) & 1) === 0) c++;
  return c;
}

// ── Deduce explanation ──

export function explainDeduce(
  puzzle: Puzzle,
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  result: DeduceResult,
): string[] {
  const a = result.action;
  if (a.type === "force") {
    return explainForce(puzzle, fp, answers, eliminated, a.questionIndex, a.letter);
  }
  return explainElimination(
    puzzle,
    fp,
    answers,
    eliminated,
    a.questionIndex,
    a.optionIndex,
    result.rule,
  );
}

function explainForce(
  _puzzle: Puzzle,
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  qi: number,
  letter: AnswerLetter,
): string[] {
  const steps = [`Try looking at ${Q(qi)}.`];
  const r = fp.questions[qi];
  const n = fp.n;

  if (remainingCount(eliminated[qi]) === 1) {
    steps.push(`${Q(qi)} has only one option left — it must be ${letter}.`);
    return steps;
  }

  if (r.t === RT_ANSWER_OF && answers[r.questionIndex] != null) {
    const target = answers[r.questionIndex]!;
    steps.push(
      `${Q(qi)} asks for ${Q(r.questionIndex)}'s answer — ${Q(r.questionIndex)} is ${target}, so ${Q(qi)} must be ${letter}.`,
    );
    return steps;
  }

  for (let other = 0; other < n; other++) {
    const otherAns = answers[other];
    if (otherAns == null) continue;
    const otherR = fp.questions[other];
    if (otherR.t === RT_ANSWER_OF && otherR.questionIndex === qi) {
      steps.push(`${Q(other)} is ${otherAns}, which says ${Q(qi)}'s answer is ${letter}.`);
      return steps;
    }
    if (otherR.t === RT_SAME_AS && fp.optionValues[other][letterIdx(otherAns)] === qi) {
      steps.push(
        `${Q(other)} is ${otherAns}, meaning it shares an answer with ${Q(qi)} — so ${Q(qi)} must be ${otherAns}.`,
      );
      return steps;
    }
  }

  if (r.t === RT_LETTER_DIST && answers[r.questionIndex] != null) {
    const other = answers[r.questionIndex]!;
    steps.push(
      `${Q(r.questionIndex)} is ${other}, and only option ${letter} gives the right distance.`,
    );
    return steps;
  }

  const pred = countPred(r);
  if (pred) {
    const [from, to] = countRange(r, n);
    const cr = countAnswers(answers, pred, from, to);
    if (cr.remaining === 0) {
      steps.push(
        `All relevant questions are answered — there are ${cr.count} ${countRuleLabel(r, cr.count)}, so ${Q(qi)} must be ${letter}.`,
      );
      return steps;
    }
  }

  steps.push(`${Q(qi)} must be ${letter}.`);
  return steps;
}

function explainElimination(
  puzzle: Puzzle,
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  qi: number,
  oi: number,
  rule: string,
): string[] {
  const letter = LETTERS[oi];
  const r = fp.questions[qi];
  const v = fp.optionValues[qi][oi];
  const ov = optLabel(puzzle, qi, oi);
  const n = fp.n;
  const steps = [`Try looking at ${Q(qi)}.`, `Consider ${Q(qi)}, option ${letter}.`];

  if (rule === "count_saturation") {
    steps.push(
      explainCountSaturation(fp, answers, eliminated, qi, oi) ?? `${Q(qi)} can't be ${letter}.`,
    );
    return steps;
  }

  if (rule === "vowel_consonant_cross") {
    steps.push(
      `${Q(qi)}, option ${letter} (${ov}): no compatible option exists on the other counting rule.`,
    );
    return steps;
  }

  const detail = explainElimDetail(r, qi, oi, ov, v, answers, eliminated, n);
  steps.push(detail ? `${Q(qi)}, option ${letter}: ${detail}` : `${Q(qi)} can't be ${letter}.`);
  return steps;
}

function explainCountSaturation(
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  _eliminated: number[],
  qi: number,
  oi: number,
): string | null {
  const n = fp.n;
  for (let src = 0; src < n; src++) {
    if (answers[src] == null) continue;
    const r = fp.questions[src];
    const pred = countPred(r);
    if (!pred) continue;
    const ai = letterIdx(answers[src]!);
    const value = fp.optionValues[src][ai];
    if (value == null) continue;
    const [from, to] = countRange(r, n);
    const cr = countAnswers(answers, pred, from, to);
    if (cr.count === value && pred(LETTERS[oi])) {
      return `${Q(src)} says ${value} ${countRuleLabel(r, value)}, and that count is already met — so ${Q(qi)} can't be ${LETTERS[oi]}.`;
    }
    if (cr.count + cr.remaining === value && !pred(LETTERS[oi])) {
      return `${Q(src)} says ${value} ${countRuleLabel(r, value)}, and all remaining unknowns must match — so ${Q(qi)} can't be ${LETTERS[oi]}.`;
    }
  }
  return null;
}

function explainElimDetail(
  r: {
    t: number;
    answer: string | null;
    questionIndex: number;
    afterIndex: number;
    beforeIndex: number;
  },
  qi: number,
  oi: number,
  ov: string,
  v: number | null,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  n: number,
): string | null {
  const pred = countPred(r);
  if (pred && r.t !== RT_MOST_COMMON_COUNT) {
    const [from, to] = countRange(r, n);
    const cr = countAnswers(answers, pred, from, to);
    if (v != null) {
      if (cr.count > v)
        return `says ${ov}, but already ${cr.count} ${countRuleLabel(r, cr.count)}.`;
      if (cr.count + cr.remaining < v)
        return `says ${ov}, but at most ${cr.count + cr.remaining} ${countRuleLabel(r, cr.count + cr.remaining)} are possible.`;
    }
  }

  if (r.t === RT_ANSWER_OF) {
    const target = answers[r.questionIndex];
    if (target != null && v != null && letterIdx(target) !== v)
      return `says ${Q(r.questionIndex)} is ${LETTERS[v]}, but it's marked ${target}.`;
    if (target == null && v != null && v >= 0 && v < 5 && isElim(eliminated, r.questionIndex, v))
      return `says ${Q(r.questionIndex)} is ${LETTERS[v]}, but ${LETTERS[v]} is already ruled out there.`;
  }

  if (r.t === RT_LETTER_DIST) {
    const other = answers[r.questionIndex];
    if (other != null && v != null) {
      const dist = Math.abs(oi - letterIdx(other));
      if (dist !== v)
        return `says distance is ${ov}, but ${LETTERS[oi]} and ${other} are ${dist} apart.`;
    }
    if (other == null && v != null) {
      let anyPossible = false;
      for (let ti = 0; ti < 5; ti++) {
        if (!isElim(eliminated, r.questionIndex, ti) && Math.abs(oi - ti) === v) anyPossible = true;
      }
      if (!anyPossible)
        return `says distance is ${ov}, but no remaining option for ${Q(r.questionIndex)} gives that distance.`;
    }
  }

  if (r.t === RT_CLOSEST_AFTER || r.t === RT_FIRST_WITH) {
    const scanStart = r.t === RT_CLOSEST_AFTER ? r.afterIndex + 1 : 0;
    if (v != null) {
      if (v < scanStart || v >= n) return `says ${Q(v)}, but that's out of range.`;
      if (answers[v] != null && answers[v] !== r.answer)
        return `says ${Q(v)}, but ${Q(v)} is marked ${answers[v]}.`;
      if (answers[v] == null && isElim(eliminated, v, L2I[r.answer!]))
        return `says ${Q(v)}, but ${r.answer} is already ruled out there.`;
      for (let j = scanStart; j < v; j++) {
        if (answers[j] === r.answer)
          return `says ${Q(v)}, but ${Q(j)} also has ${r.answer} and comes earlier.`;
      }
    } else {
      for (let j = scanStart; j < n; j++) {
        if (answers[j] === r.answer) return `says "None", but ${Q(j)} has answer ${r.answer}.`;
      }
    }
  }

  if (r.t === RT_CLOSEST_BEFORE || r.t === RT_LAST_WITH) {
    const beforeIdx = r.t === RT_CLOSEST_BEFORE ? r.beforeIndex : n;
    if (v != null) {
      if (v < 0 || v >= beforeIdx) return `says ${Q(v)}, but that's out of range.`;
      if (answers[v] != null && answers[v] !== r.answer)
        return `says ${Q(v)}, but ${Q(v)} is marked ${answers[v]}.`;
      if (answers[v] == null && isElim(eliminated, v, L2I[r.answer!]))
        return `says ${Q(v)}, but ${r.answer} is already ruled out there.`;
      for (let j = beforeIdx - 1; j > v; j--) {
        if (answers[j] === r.answer)
          return `says ${Q(v)}, but ${Q(j)} also has ${r.answer} and comes later.`;
      }
    } else {
      for (let j = 0; j < beforeIdx; j++) {
        if (answers[j] === r.answer) return `says "None", but ${Q(j)} has answer ${r.answer}.`;
      }
    }
  }

  if (r.t === RT_ONLY_ODD || r.t === RT_ONLY_EVEN) {
    const parity = r.t === RT_ONLY_ODD ? 1 : 0;
    const parityName = r.t === RT_ONLY_ODD ? "even" : "odd";
    if (v != null) {
      if ((v + 1) % 2 !== parity) return `says ${Q(v)}, but that's an ${parityName}-numbered question.`;
      if (v >= 0 && v < n && answers[v] != null && answers[v] !== r.answer)
        return `says ${Q(v)}, but ${Q(v)} is marked ${answers[v]}.`;
      if (v >= 0 && v < n && answers[v] == null && isElim(eliminated, v, L2I[r.answer!]))
        return `says ${Q(v)}, but ${r.answer} is already ruled out there.`;
    } else {
      for (let i = 0; i < n; i++) {
        if ((i + 1) % 2 === parity && answers[i] === r.answer)
          return `says "None", but ${Q(i)} has answer ${r.answer}.`;
      }
    }
  }

  if (r.t === RT_CONSEC_IDENT) {
    if (v != null) {
      if (v + 1 >= n) return `says ${Q(v)} & ${Q(v + 1)}, but that's out of range.`;
      const possA = ~eliminated[v] & 0b11111;
      const possB = ~eliminated[v + 1] & 0b11111;
      if ((possA & possB) === 0)
        return `says ${Q(v)} & ${Q(v + 1)}, but they have no possible answer in common.`;
    } else {
      for (let i = 0; i < n - 1; i++) {
        if (answers[i] != null && answers[i + 1] != null && answers[i] === answers[i + 1])
          return `says none, but ${Q(i)} and ${Q(i + 1)} both have answer ${answers[i]}.`;
      }
    }
  }

  if (r.t === RT_PREV_SAME && v != null) {
    if (v >= qi) return `says ${Q(v)}, but that's not before ${Q(qi)}.`;
    if (isElim(eliminated, v, oi)) return `says ${Q(v)}, but ${LETTERS[oi]} is ruled out there.`;
    for (let j = qi - 1; j > v; j--) {
      if (answers[j] === LETTERS[oi])
        return `says ${Q(v)}, but ${Q(j)} also has ${LETTERS[oi]} and is closer.`;
    }
  }

  if (r.t === RT_NEXT_SAME && v != null) {
    if (v <= qi || v >= n) return `says ${Q(v)}, but that's not after ${Q(qi)}.`;
    if (isElim(eliminated, v, oi)) return `says ${Q(v)}, but ${LETTERS[oi]} is ruled out there.`;
    for (let j = qi + 1; j < v; j++) {
      if (answers[j] === LETTERS[oi])
        return `says ${Q(v)}, but ${Q(j)} also has ${LETTERS[oi]} and is closer.`;
    }
  }

  if ((r.t === RT_ONLY_SAME || r.t === RT_SAME_AS) && v != null) {
    if (v === qi) return `can't refer to itself.`;
    if (v >= 0 && v < n && isElim(eliminated, v, oi))
      return `says ${Q(v)}, but ${LETTERS[oi]} is ruled out there.`;
  }

  if (r.t === RT_UNIQUE) {
    const a = LETTERS[oi];
    let count = 0;
    for (let i = 0; i < n; i++) if (answers[i] === a) count++;
    if (count > 0) return `${a} is already used by another question.`;
  }

  return null;
}

// ── Lookahead explanation ──

export function explainLookahead(
  puzzle: Puzzle,
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  _eliminated: number[],
  result: LookaheadResult,
): string[] {
  const qi = result.assumptionQi;
  const letter = result.assumptionAnswer;
  const steps: string[] = [];

  steps.push(`Try looking at ${Q(qi)}.`);
  steps.push(`What if ${Q(qi)} is ${letter}?`);

  const chainParts: string[] = [];
  for (const dr of result.chain) {
    const a = dr.action;
    if (a.type === "force") {
      chainParts.push(`${Q(a.questionIndex)} must be ${a.letter}`);
    }
  }

  const contradictionQi = result.contradictionQi;

  let detail = `${Q(contradictionQi)} would be invalid`;
  const hypAnswers = rebuildHypState(answers, fp.n, result);
  const hypEliminated = rebuildHypEliminated(_eliminated, fp.n, result);
  const reason = explainInvalidDetail(fp, puzzle, hypAnswers, hypEliminated, contradictionQi);
  if (reason) detail = reason;

  if (chainParts.length > 0) {
    steps.push(`Then ${chainParts.join(", and ")} — but ${detail}.`);
  } else {
    steps.push(`But ${detail}.`);
  }

  steps.push(`So ${Q(qi)} can't be ${letter}.`);
  return steps;
}

function rebuildHypState(
  answers: (AnswerLetter | null)[],
  n: number,
  result: LookaheadResult,
): (AnswerLetter | null)[] {
  const hyp = answers.slice(0, n);
  hyp[result.assumptionQi] = result.assumptionAnswer;
  for (const dr of result.chain) {
    if (dr.action.type === "force") {
      hyp[dr.action.questionIndex] = dr.action.letter;
    }
  }
  return hyp;
}

function rebuildHypEliminated(eliminated: number[], n: number, result: LookaheadResult): number[] {
  const hyp = eliminated.slice(0, n);
  const assumeOi = letterIdx(result.assumptionAnswer);
  hyp[result.assumptionQi] = 0b11111 ^ (1 << assumeOi);
  for (const dr of result.chain) {
    const a = dr.action;
    if (a.type === "force") {
      hyp[a.questionIndex] = 0b11111 ^ (1 << letterIdx(a.letter));
    } else {
      hyp[a.questionIndex] |= 1 << a.optionIndex;
    }
  }
  return hyp;
}

// ── Validity explanation (why is this red?) ──

export function explainInvalid(
  fp: FlatPuzzle,
  puzzle: Puzzle,
  answers: (AnswerLetter | null)[],
  _eliminated: number[],
  qi: number,
): string | null {
  if (answers[qi] == null) return null;
  return explainInvalidDetail(fp, puzzle, answers, _eliminated, qi);
}

function explainInvalidDetail(
  fp: FlatPuzzle,
  puzzle: Puzzle,
  answers: (AnswerLetter | null)[],
  _eliminated: number[],
  qi: number,
): string | null {
  const a = answers[qi];
  if (a == null) return null;
  const ai = letterIdx(a);
  const r = fp.questions[qi];
  const v = fp.optionValues[qi][ai];
  const ov = optLabel(puzzle, qi, ai);
  const n = fp.n;

  const pred = countPred(r);
  if (pred && r.t !== RT_MOST_COMMON_COUNT) {
    const [from, to] = countRange(r, n);
    const cr = countAnswers(answers, pred, from, to);
    if (v != null) {
      if (cr.count > v)
        return `${Q(qi)} says ${ov} ${countRuleLabel(r, v)}, but there are already ${cr.count}`;
      if (cr.count + cr.remaining < v)
        return `${Q(qi)} says ${ov} ${countRuleLabel(r, v)}, but at most ${cr.count + cr.remaining} are possible`;
    }
  }

  if (r.t === RT_ANSWER_OF) {
    const target = answers[r.questionIndex];
    if (target != null && v != null && letterIdx(target) !== v)
      return `${Q(qi)} says ${Q(r.questionIndex)} is ${LETTERS[v]}, but it's ${target}`;
  }

  if (r.t === RT_LETTER_DIST) {
    const other = answers[r.questionIndex];
    if (other != null && v != null) {
      const dist = Math.abs(ai - letterIdx(other));
      if (dist !== v)
        return `${Q(qi)} says distance ${ov}, but ${a} and ${other} are ${dist} apart`;
    }
  }

  if (r.t === RT_UNIQUE) {
    let count = 0;
    for (let i = 0; i < n; i++) if (answers[i] === a) count++;
    if (count > 1) return `${a} should be unique, but ${count} questions have answer ${a}`;
  }

  if (r.t === RT_CLOSEST_AFTER || r.t === RT_FIRST_WITH) {
    const scanStart = r.t === RT_CLOSEST_AFTER ? r.afterIndex + 1 : 0;
    if (v != null && v >= 0 && v < n && answers[v] != null && answers[v] !== r.answer)
      return `${Q(qi)} says ${Q(v)} has ${r.answer}, but ${Q(v)} is ${answers[v]}`;
    if (v != null) {
      for (let j = scanStart; j < v; j++) {
        if (answers[j] === r.answer)
          return `${Q(qi)} says first ${r.answer} is ${Q(v)}, but ${Q(j)} has ${r.answer} earlier`;
      }
    }
    if (v == null) {
      for (let j = scanStart; j < n; j++) {
        if (answers[j] === r.answer)
          return `${Q(qi)} says no ${r.answer} exists, but ${Q(j)} has it`;
      }
    }
  }

  if (r.t === RT_CLOSEST_BEFORE || r.t === RT_LAST_WITH) {
    const beforeIdx = r.t === RT_CLOSEST_BEFORE ? r.beforeIndex : n;
    if (v != null && v >= 0 && v < n && answers[v] != null && answers[v] !== r.answer)
      return `${Q(qi)} says ${Q(v)} has ${r.answer}, but ${Q(v)} is ${answers[v]}`;
    if (v != null) {
      for (let j = beforeIdx - 1; j > v; j--) {
        if (answers[j] === r.answer)
          return `${Q(qi)} says last ${r.answer} is ${Q(v)}, but ${Q(j)} has ${r.answer} later`;
      }
    }
    if (v == null) {
      for (let j = 0; j < beforeIdx; j++) {
        if (answers[j] === r.answer)
          return `${Q(qi)} says no ${r.answer} exists, but ${Q(j)} has it`;
      }
    }
  }

  if (r.t === RT_SAME_AS) {
    if (v != null && v >= 0 && v < n && answers[v] != null && answers[v] !== a)
      return `${Q(qi)} says same as ${Q(v)}, but ${Q(v)} is ${answers[v]} and this is ${a}`;
  }

  return null;
}
