import type { AnswerLetter, FlatPuzzle, Puzzle } from "./types.ts";

export type ExplainStep =
  | { type: "simple"; text: string }
  | { type: "complex"; header: string; lines: string[] };

function simple(text: string): ExplainStep {
  return { type: "simple", text };
}
function complex(header: string, lines: string[]): ExplainStep {
  return { type: "complex", header, lines };
}
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
  RT_EQUAL_COUNT,
  RT_LETTER_DIST,
} from "./types.ts";
import type { DeduceResult } from "./deduce.ts";
import type { LookaheadResult } from "./lookahead.ts";

// ── Formatting helpers ──

function Q(i: number): string {
  return `Q${i + 1}`;
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
  eliminated: number[],
  pred: Pred,
  from: number,
  to: number,
): { count: number; remaining: number } {
  let count = 0;
  let remaining = 0;
  for (let i = from; i < to; i++) {
    const a = answers[i];
    if (a == null) {
      for (let oi = 0; oi < 5; oi++) {
        if (((eliminated[i] >> oi) & 1) === 0 && pred(LETTERS[oi])) {
          remaining++;
          break;
        }
      }
    } else if (pred(a)) count++;
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
): ExplainStep[] {
  const a = result.action;
  if (a.type === "force") {
    return explainForce(puzzle, fp, answers, eliminated, a.questionIndex, a.letter, result.rule);
  }
  if (a.type === "eliminateMulti") {
    const qis: number[] = [];
    for (let i = 0; i < 16; i++) {
      if ((a.questionMask >> i) & 1) qis.push(i);
    }
    const optLetters: string[] = [];
    for (let b = 0; b < 5; b++) {
      if ((a.optionMask >> b) & 1) optLetters.push(LETTERS[b]);
    }
    const qList = qis.map(Q).join(", ");
    const optStr = optLetters.join(", ");

    if (result.rule === "PositionalRangeAnswered" || result.rule === "PositionalRangeUnanswered") {
      const oi = optLetters.length === 1 ? letterIdx(optLetters[0]) : 0;
      const src = findPositionalRangeSource(fp, answers, eliminated, qis[0], oi);
      const steps: ExplainStep[] = [];
      if (src) {
        steps.push(simple(`Try looking at ${Q(src.srcQi)}.`));
        const allQis = [src.srcQi, ...qis].sort((x, y) => x - y);
        steps.push(simple(`Try looking at ${allQis.map(Q).join(", ")}.`));
        steps.push(simple(`${qList} can't be ${optStr}: ${src.text}`));
      } else {
        console.error(`No positional_range explain for ${qList} option ${optStr}`);
        steps.push(simple(`Try looking at ${qList}.`));
        steps.push(simple(`${qList} can't be ${optStr}.`));
      }
      return steps;
    }

    const firstQi = qis[0];
    const reason = explainMultiElim(fp, answers, eliminated, firstQi, a.optionMask);
    const steps: ExplainStep[] = [];
    if (reason.otherQi != null) {
      steps.push(simple(`Try looking at ${Q(reason.otherQi)}.`));
      const allQis = [reason.otherQi, ...qis].sort((x, y) => x - y);
      steps.push(simple(`Try looking at ${allQis.map(Q).join(", ")}.`));
    } else {
      steps.push(simple(`Try looking at ${qis.map(Q).join(", ")}.`));
    }
    steps.push(simple(`${qList} can't be ${optStr}: ${reason.text}`));
    return steps;
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
  rule: string,
): ExplainStep[] {
  const steps: ExplainStep[] = [simple(`Try looking at ${Q(qi)}.`)];
  const r = fp.questions[qi];
  const n = fp.n;

  if (remainingCount(eliminated[qi]) === 1) {
    steps.push(simple(`${Q(qi)} has only one option left — it must be ${letter}.`));
    return steps;
  }

  if (r.t === RT_ANSWER_OF && answers[r.questionIndex] != null) {
    steps.push(simple(`Try looking at ${Q(qi)} and ${Q(r.questionIndex)}.`));
    steps.push(simple(
      `${Q(qi)} asks for ${Q(r.questionIndex)}'s answer. ${Q(r.questionIndex)} is ${answers[r.questionIndex]}, so ${Q(qi)} must be ${letter}.`,
    ));
    return steps;
  }

  for (let other = 0; other < n; other++) {
    const otherAns = answers[other];
    if (otherAns == null) continue;
    const otherR = fp.questions[other];
    if (otherR.t === RT_SAME_AS && fp.optionValues[other][letterIdx(otherAns)] === qi) {
      steps.push(simple(`Try looking at ${Q(qi)} and ${Q(other)}.`));
      steps.push(simple(
        `${Q(other)} says it has the same answer as ${Q(qi)}. ${Q(other)} is ${otherAns}, so ${Q(qi)} must be ${otherAns}.`,
      ));
      return steps;
    }
    if (
      (otherR.t === RT_PREV_SAME || otherR.t === RT_NEXT_SAME || otherR.t === RT_ONLY_SAME) &&
      fp.optionValues[other][letterIdx(otherAns)] === qi
    ) {
      steps.push(simple(`Try looking at ${Q(qi)} and ${Q(other)}.`));
      steps.push(simple(
        `${Q(other)} is ${otherAns}, pointing to ${Q(qi)} as having the same answer. So ${Q(qi)} must be ${otherAns}.`,
      ));
      return steps;
    }
  }

  // Reverse AnswerOf: another question says qi's answer
  for (let other = 0; other < n; other++) {
    const otherAns = answers[other];
    if (otherAns == null) continue;
    const otherR = fp.questions[other];
    if (otherR.t === RT_ANSWER_OF && otherR.questionIndex === qi) {
      steps.push(simple(`Try looking at ${Q(qi)} and ${Q(other)}.`));
      steps.push(simple(
        `${Q(other)} asks for ${Q(qi)}'s answer. ${Q(other)} is ${otherAns}, telling us ${Q(qi)} must be ${letter}.`,
      ));
      return steps;
    }
  }

  // LetterDist: target answered, only one valid distance
  if (r.t === RT_LETTER_DIST && answers[r.questionIndex] != null) {
    steps.push(simple(`Try looking at ${Q(qi)} and ${Q(r.questionIndex)}.`));
    steps.push(simple(
      `${Q(r.questionIndex)} is answered ${answers[r.questionIndex]}. Only option ${letter} gives the right letter distance.`,
    ));
    return steps;
  }

  // Reverse LetterDist: another question's LetterDist constrains qi
  for (let src = 0; src < n; src++) {
    if (src === qi) continue;
    const srcR = fp.questions[src];
    if (srcR.t !== RT_LETTER_DIST || srcR.questionIndex !== qi) continue;
    const srcAns = answers[src];
    if (srcAns != null) {
      const dist = fp.optionValues[src][letterIdx(srcAns)];
      steps.push(simple(`Try looking at ${Q(qi)} and ${Q(src)}.`));
      steps.push(simple(
        `${Q(src)} is answered ${srcAns} with letter distance ${dist}. Only ${letter} is at distance ${dist} from ${srcAns}, so ${Q(qi)} must be ${letter}.`,
      ));
      return steps;
    }
  }

  // Counting: all in range answered, count determines answer
  const pred = countPred(r);
  if (pred) {
    const [from, to] = countRange(r, n);
    const cr = countAnswers(answers, eliminated, pred, from, to);
    if (cr.remaining === 0) {
      steps.push(simple(
        `There are ${cr.count} ${countRuleLabel(r, cr.count)}, so ${Q(qi)} must be ${letter}.`,
      ));
      return steps;
    }
  }

  if (rule === "CountMustMatchForce") {
    const src = findCountSatSource(fp, answers, eliminated, qi, letter);
    if (src != null) {
      const srcR = fp.questions[src];
      const srcAi = letterIdx(answers[src]!);
      const srcVal = fp.optionValues[src][srcAi]!;
      const [from, to] = countRange(srcR, n);
      const cr = countAnswers(answers, eliminated, countPred(srcR)!, from, to);
      steps.push(simple(`Try looking at ${Q(qi)} and ${Q(src)}.`));
      steps.push(simple(
        `${Q(src)} says there are ${srcVal} ${countRuleLabel(srcR, srcVal)}. Only ${cr.count} found so far, and ${Q(qi)} is the only remaining question that could be ${letter} — so ${Q(qi)} must be ${letter}.`,
      ));
      return steps;
    }
  }

  throw new Error(`explainForce: no explanation found for ${Q(qi)} = ${letter} (rule: ${rule})`);
}

function findCountSatSource(
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  _targetQi: number,
  targetLetter: AnswerLetter,
): number | null {
  const n = fp.n;
  for (let src = 0; src < n; src++) {
    if (answers[src] == null) continue;
    const r = fp.questions[src];
    const pred = countPred(r);
    if (!pred || !pred(targetLetter)) continue;
    const ai = letterIdx(answers[src]!);
    const value = fp.optionValues[src][ai];
    if (value == null) continue;
    const [from, to] = countRange(r, n);
    const cr = countAnswers(answers, eliminated, pred, from, to);
    if (cr.count + cr.remaining === value && cr.remaining > 0) {
      return src;
    }
  }
  return null;
}

function explainElimination(
  _puzzle: Puzzle,
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  qi: number,
  oi: number,
  rule: string,
): ExplainStep[] {
  const letter = LETTERS[oi];
  const r = fp.questions[qi];
  const v = fp.optionValues[qi][oi];
  const n = fp.n;
  const steps: ExplainStep[] = [simple(`Try looking at ${Q(qi)}.`)];

  if (rule === "CountSaturated" || rule === "CountMustMatchElim") {
    const sat = explainCountSaturation(fp, answers, eliminated, qi, oi);
    if (sat) {
      steps.push(simple(`Try looking at ${Q(qi)} and ${Q(sat.srcQi)}.`));
      steps.push(simple(`What if ${Q(qi)} is ${letter}?`));
      steps.push(simple(sat.text));
    } else {
      steps.push(simple(`What if ${Q(qi)} is ${letter}?`));
      steps.push(simple(`${Q(qi)} can't be ${letter}.`));
    }
    return steps;
  }

  if (rule === "PositionalRangeAnswered" || rule === "PositionalRangeUnanswered") {
    const src = findPositionalRangeSource(fp, answers, eliminated, qi, oi);
    if (src != null) {
      steps.push(simple(`Try looking at ${Q(src.srcQi)} and ${Q(qi)}.`));
      steps.push(simple(src.text));
    } else {
      steps.push(simple(`${Q(qi)} can't be ${letter}.`));
    }
    return steps;
  }

  if (rule === "VowelCrossElim" || rule === "ConsonantCrossElim") {
    steps.push(simple(`What if ${Q(qi)} is ${letter}?`));
    steps.push(simple(
      `${Q(qi)} option ${letter}: no compatible option exists on the other counting rule.`,
    ));
    return steps;
  }

  const detail = explainElimDetail(r, qi, oi, v, answers, eliminated, n);
  if (detail && detail.otherQi != null) {
    steps.push(simple(`Try looking at ${Q(qi)} and ${Q(detail.otherQi)}.`));
  }
  steps.push(simple(`What if ${Q(qi)} is ${letter}?`));
  if (!detail) console.error(`No explainElimDetail for ${Q(qi)} option ${letter} (rule: ${rule})`);
  steps.push(simple(detail ? detail.text : `${Q(qi)} can't be ${letter}.`));
  return steps;
}

function findPositionalRangeSource(
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  qi: number,
  oi: number,
): { text: string; srcQi: number } | null {
  const n = fp.n;
  const letter = LETTERS[oi];
  for (let src = 0; src < n; src++) {
    if (src === qi) continue;
    const srcR = fp.questions[src];
    const srcAns = answers[src];

    if (srcR.t === RT_FIRST_WITH || srcR.t === RT_CLOSEST_AFTER) {
      if (srcR.answer !== letter) continue;
      const label = srcR.t === RT_FIRST_WITH ? "first" : "closest";
      if (srcAns != null) {
        const v = fp.optionValues[src][letterIdx(srcAns)];
        if (v != null && qi < v) {
          return {
            srcQi: src,
            text: `${Q(src)} says ${label} ${letter} is ${Q(v)}, so ${Q(qi)} can't be ${letter}.`,
          };
        }
      } else {
        let minPos = n;
        for (let si = 0; si < 5; si++) {
          if (isElim(eliminated, src, si)) continue;
          const optV = fp.optionValues[src][si];
          if (optV != null && optV < minPos) minPos = optV;
        }
        return {
          srcQi: src,
          text: `${Q(src)}'s remaining options for ${label} ${letter} are all at ${Q(minPos)} or later, so earlier questions can't be ${letter}.`,
        };
      }
    }

    if (srcR.t === RT_LAST_WITH || srcR.t === RT_CLOSEST_BEFORE) {
      if (srcR.answer !== letter) continue;
      const label = srcR.t === RT_LAST_WITH ? "last" : "closest";
      if (srcAns != null) {
        const v = fp.optionValues[src][letterIdx(srcAns)];
        if (v != null && qi > v) {
          return {
            srcQi: src,
            text: `${Q(src)} says ${label} ${letter} is ${Q(v)}, so ${Q(qi)} can't be ${letter}.`,
          };
        }
      } else {
        let maxPos = -1;
        for (let si = 0; si < 5; si++) {
          if (isElim(eliminated, src, si)) continue;
          const optV = fp.optionValues[src][si];
          if (optV != null && optV > maxPos) maxPos = optV;
        }
        return {
          srcQi: src,
          text: `${Q(src)}'s remaining options for ${label} ${letter} are all at ${Q(maxPos)} or earlier, so later questions can't be ${letter}.`,
        };
      }
    }

    if (srcR.t === RT_NEXT_SAME && srcAns != null && srcAns === letter) {
      const v = fp.optionValues[src][letterIdx(srcAns)];
      if (v != null && qi > src && qi < v) {
        return {
          srcQi: src,
          text: `${Q(src)} is ${srcAns} and says next same answer is ${Q(v)}, so ${Q(qi)} can't be ${letter}.`,
        };
      }
    }

    if (srcR.t === RT_PREV_SAME && srcAns != null && srcAns === letter) {
      const v = fp.optionValues[src][letterIdx(srcAns)];
      if (v != null && qi > v && qi < src) {
        return {
          srcQi: src,
          text: `${Q(src)} is ${srcAns} and says previous same answer is ${Q(v)}, so ${Q(qi)} can't be ${letter}.`,
        };
      }
    }
  }
  return null;
}

function explainCountSaturation(
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  qi: number,
  oi: number,
): { text: string; srcQi: number } | null {
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
    const cr = countAnswers(answers, eliminated, pred, from, to);
    if (cr.count === value && pred(LETTERS[oi])) {
      return {
        srcQi: src,
        text: `${Q(src)} says there are ${value} ${countRuleLabel(r, value)}. There are already ${value}, so ${Q(qi)} can't also be ${LETTERS[oi]}.`,
      };
    }
    if (cr.count + cr.remaining === value && cr.remaining > 0 && !pred(LETTERS[oi])) {
      return {
        srcQi: src,
        text: `${Q(src)} says there are ${value} ${countRuleLabel(r, value)}. Only ${cr.count} found so far, and all remaining unknowns must match — so ${Q(qi)} can't be ${LETTERS[oi]}.`,
      };
    }
  }
  return null;
}

function explainMultiElim(
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  _eliminated: number[],
  qi: number,
  _optionMask: number,
): { text: string; otherQi: number | null } {
  const n = fp.n;
  for (let src = 0; src < n; src++) {
    if (src === qi) continue;
    const srcR = fp.questions[src];
    if (srcR.t !== RT_LETTER_DIST || srcR.questionIndex !== qi) continue;
    const srcAns = answers[src];
    if (srcAns != null) {
      const dist = fp.optionValues[src][letterIdx(srcAns)];
      return {
        text: `${Q(src)} is answered ${srcAns} with letter distance ${dist}, so only answers at distance ${dist} from ${srcAns} are possible.`,
        otherQi: src,
      };
    }
    return {
      text: `${Q(src)}'s remaining options limit which answers are possible for ${Q(qi)}.`,
      otherQi: src,
    };
  }
  console.error(`No explainMultiElim source for ${Q(qi)}`);
  return { text: "incompatible with other constraints.", otherQi: null };
}

interface ElimDetail {
  text: string;
  otherQi: number | null;
}

function d(text: string, otherQi: number | null = null): ElimDetail {
  return { text, otherQi };
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
  v: number | null,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  n: number,
): ElimDetail | null {
  const letter = LETTERS[oi];
  const pred = countPred(r);
  if (pred && r.t !== RT_MOST_COMMON_COUNT) {
    const [from, to] = countRange(r, n);
    const cr = countAnswers(answers, eliminated, pred, from, to);
    if (v != null) {
      if (cr.count > v)
        return d(`${Q(qi)} option ${letter} claims ${v} ${countRuleLabel(r, v)}, but there are already ${cr.count}.`);
      if (cr.count + cr.remaining < v)
        return d(`${Q(qi)} option ${letter} claims ${v} ${countRuleLabel(r, v)}, but at most ${cr.count + cr.remaining} are possible.`);
    }
  }

  if (r.t === RT_ANSWER_OF) {
    if (v != null && v >= 0 && v < 5 && isElim(eliminated, r.questionIndex, v))
      return d(`${Q(qi)} option ${letter} claims ${Q(r.questionIndex)}'s answer is ${LETTERS[v]}, but ${LETTERS[v]} is ruled out for ${Q(r.questionIndex)}.`, r.questionIndex);
  }

  if (r.t === RT_LETTER_DIST) {
    const maxDist = Math.max(oi, 4 - oi);
    if (v != null && v > maxDist)
      return d(`${Q(qi)} option ${letter} claims letter distance ${v}, but ${letter} can be at most ${maxDist} letters from any answer.`);
    const other = answers[r.questionIndex];
    if (other != null && v != null) {
      const dist = Math.abs(oi - letterIdx(other));
      if (dist !== v)
        return d(`${Q(qi)} option ${letter} claims letter distance ${v}, but ${letter} is ${dist} letters from ${Q(r.questionIndex)}'s answer ${other}.`, r.questionIndex);
    }
    if (other == null && v != null) {
      let anyPossible = false;
      for (let ti = 0; ti < 5; ti++) {
        if (!isElim(eliminated, r.questionIndex, ti) && Math.abs(oi - ti) === v) anyPossible = true;
      }
      if (!anyPossible)
        return d(`${Q(qi)} option ${letter} claims letter distance ${v}, but no remaining answer for ${Q(r.questionIndex)} gives that distance from ${letter}.`, r.questionIndex);
    }
  }

  if (r.t === RT_CLOSEST_AFTER || r.t === RT_FIRST_WITH) {
    const label = r.t === RT_FIRST_WITH ? "first" : "closest";
    const scanStart = r.t === RT_CLOSEST_AFTER ? r.afterIndex + 1 : 0;
    if (v != null) {
      if (v < scanStart || v >= n)
        return d(`${Q(qi)} option ${letter} claims ${label} ${r.answer} is #${v + 1}, but that's out of range.`);
      if (answers[v] != null && answers[v] !== r.answer)
        return d(`${Q(qi)} option ${letter} claims ${label} ${r.answer} is ${Q(v)}, but ${Q(v)} is answered ${answers[v]}.`, v);
      if (answers[v] == null && isElim(eliminated, v, L2I[r.answer!]))
        return d(`${Q(qi)} option ${letter} claims ${label} ${r.answer} is ${Q(v)}, but ${r.answer} is ruled out for ${Q(v)}.`, v);
      for (let j = scanStart; j < v; j++) {
        if (answers[j] === r.answer)
          return d(`${Q(qi)} option ${letter} claims ${label} ${r.answer} is ${Q(v)}, but ${Q(j)} already has answer ${r.answer} and comes before ${Q(v)}.`, j);
      }
      if (LETTERS[oi] === r.answer && qi >= scanStart && qi < v)
        return d(`${Q(qi)} option ${letter} claims ${label} ${r.answer} is ${Q(v)}, but ${Q(qi)} itself is before ${Q(v)} and would have answer ${r.answer}. Contradiction.`);
    } else {
      for (let j = scanStart; j < n; j++) {
        if (answers[j] === r.answer)
          return d(`${Q(qi)} option ${letter} claims no question has answer ${r.answer}, but ${Q(j)} has answer ${r.answer}.`, j);
      }
    }
  }

  if (r.t === RT_CLOSEST_BEFORE || r.t === RT_LAST_WITH) {
    const label = r.t === RT_LAST_WITH ? "last" : "closest";
    const beforeIdx = r.t === RT_CLOSEST_BEFORE ? r.beforeIndex : n;
    if (v != null) {
      if (v < 0 || v >= beforeIdx)
        return d(`${Q(qi)} option ${letter} claims ${label} ${r.answer} is #${v + 1}, but that's out of range.`);
      if (answers[v] != null && answers[v] !== r.answer)
        return d(`${Q(qi)} option ${letter} claims ${label} ${r.answer} is ${Q(v)}, but ${Q(v)} is answered ${answers[v]}.`, v);
      if (answers[v] == null && isElim(eliminated, v, L2I[r.answer!]))
        return d(`${Q(qi)} option ${letter} claims ${label} ${r.answer} is ${Q(v)}, but ${r.answer} is ruled out for ${Q(v)}.`, v);
      for (let j = beforeIdx - 1; j > v; j--) {
        if (answers[j] === r.answer)
          return d(`${Q(qi)} option ${letter} claims ${label} ${r.answer} is ${Q(v)}, but ${Q(j)} has answer ${r.answer} and comes after ${Q(v)}.`, j);
      }
      if (LETTERS[oi] === r.answer && qi > v && qi < beforeIdx)
        return d(`${Q(qi)} option ${letter} claims ${label} ${r.answer} is ${Q(v)}, but ${Q(qi)} itself is after ${Q(v)} and would have answer ${r.answer}. Contradiction.`);
    } else {
      for (let j = 0; j < beforeIdx; j++) {
        if (answers[j] === r.answer)
          return d(`${Q(qi)} option ${letter} claims no question has answer ${r.answer}, but ${Q(j)} has answer ${r.answer}.`, j);
      }
    }
  }

  if (r.t === RT_ONLY_ODD || r.t === RT_ONLY_EVEN) {
    const parity = r.t === RT_ONLY_ODD ? 1 : 0;
    const parityName = r.t === RT_ONLY_ODD ? "even" : "odd";
    if (v != null) {
      if ((v + 1) % 2 !== parity)
        return d(`${Q(qi)} option ${letter} claims ${Q(v)}, but ${Q(v)} is ${parityName}-numbered.`);
      if (v >= 0 && v < n && answers[v] != null && answers[v] !== r.answer)
        return d(`${Q(qi)} option ${letter} claims ${Q(v)} has answer ${r.answer}, but ${Q(v)} is answered ${answers[v]}.`, v);
      if (v >= 0 && v < n && answers[v] == null && isElim(eliminated, v, L2I[r.answer!]))
        return d(`${Q(qi)} option ${letter} claims ${Q(v)} has answer ${r.answer}, but ${r.answer} is ruled out for ${Q(v)}.`, v);
    } else {
      for (let i = 0; i < n; i++) {
        if ((i + 1) % 2 === parity && answers[i] === r.answer)
          return d(`${Q(qi)} option ${letter} claims no ${parity === 1 ? "odd" : "even"}-numbered question has answer ${r.answer}, but ${Q(i)} does.`, i);
      }
    }
  }

  if (r.t === RT_CONSEC_IDENT) {
    if (v != null && v + 1 >= n)
      return d(`${Q(qi)} option ${letter} claims ${Q(v)}-${Q(v + 1)}, but that's out of range.`);
    if (v != null && v + 1 < n) {
      if (v === qi || v + 1 === qi) {
        const partner = v === qi ? v + 1 : v;
        if (isElim(eliminated, partner, oi))
          return d(`${Q(qi)} option ${letter} claims ${Q(v)}-${Q(v + 1)} are the consecutive pair, but ${letter} is ruled out for ${Q(partner)} so they can't match.`, partner);
      }
      const possA = ~eliminated[v] & 0b11111;
      const possB = ~eliminated[v + 1] & 0b11111;
      if ((possA & possB) === 0)
        return d(`${Q(qi)} option ${letter} claims ${Q(v)}-${Q(v + 1)} are the consecutive pair, but they share no possible answer.`, v);
    } else if (v == null) {
      for (let i = 0; i < n - 1; i++) {
        if (answers[i] != null && answers[i + 1] != null && answers[i] === answers[i + 1])
          return d(`${Q(qi)} option ${letter} claims no consecutive pair exists, but ${Q(i)} and ${Q(i + 1)} both have answer ${answers[i]}.`, i);
      }
    }
  }

  if (r.t === RT_PREV_SAME && v != null) {
    if (v >= qi)
      return d(`${Q(qi)} option ${letter} claims ${Q(v)}, but ${Q(v)} is not before ${Q(qi)}.`);
    if (isElim(eliminated, v, oi))
      return d(`${Q(qi)} option ${letter} claims ${Q(v)} has the same answer, but ${letter} is ruled out for ${Q(v)}.`, v);
    for (let j = qi - 1; j > v; j--) {
      if (answers[j] === LETTERS[oi])
        return d(`${Q(qi)} option ${letter} claims previous same answer is ${Q(v)}, but ${Q(j)} also has answer ${letter} and is closer.`, j);
    }
  }

  if (r.t === RT_NEXT_SAME && v != null) {
    if (v <= qi || v >= n)
      return d(`${Q(qi)} option ${letter} claims ${Q(v)}, but ${Q(v)} is not after ${Q(qi)}.`);
    if (isElim(eliminated, v, oi))
      return d(`${Q(qi)} option ${letter} claims ${Q(v)} has the same answer, but ${letter} is ruled out for ${Q(v)}.`, v);
    for (let j = qi + 1; j < v; j++) {
      if (answers[j] === LETTERS[oi])
        return d(`${Q(qi)} option ${letter} claims next same answer is ${Q(v)}, but ${Q(j)} also has answer ${letter} and is closer.`, j);
    }
  }

  if ((r.t === RT_ONLY_SAME || r.t === RT_SAME_AS) && v != null) {
    if (v === qi)
      return d(`${Q(qi)} option ${letter} points to ${Q(qi)} itself, but a question can't share an answer with itself.`);
    if (v >= 0 && v < n && isElim(eliminated, v, oi))
      return d(`${Q(qi)} option ${letter} claims ${Q(v)} has the same answer, but ${letter} is ruled out for ${Q(v)}.`, v);
  }

  if (r.t === RT_UNIQUE) {
    for (let i = 0; i < n; i++) {
      if (answers[i] === letter)
        return d(`${Q(qi)} option ${letter} claims ${letter} is unique, but ${Q(i)} already has answer ${letter}.`, i);
    }
  }

  if (r.t === RT_EQUAL_COUNT) {
    if (v != null && LETTERS[v] === r.answer)
      return d(`${Q(qi)} option ${letter} claims ${LETTERS[v]}, but the question asks for a different letter with the same count as ${r.answer}.`);
  }

  return null;
}

// ── Lookahead explanation ──

function briefForceReason(
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  qi: number,
  letter: AnswerLetter,
): string {
  const r = fp.questions[qi];
  const n = fp.n;

  if (r.t === RT_ANSWER_OF && answers[r.questionIndex] != null)
    return `${Q(r.questionIndex)} is ${answers[r.questionIndex]}`;

  for (let other = 0; other < n; other++) {
    const otherAns = answers[other];
    if (otherAns == null) continue;
    const otherR = fp.questions[other];
    if (otherR.t === RT_ANSWER_OF && otherR.questionIndex === qi)
      return `${Q(other)} is ${otherAns}, which implies ${letter}`;
    if (otherR.t === RT_SAME_AS && fp.optionValues[other][letterIdx(otherAns)] === qi)
      return `same answer as ${Q(other)}`;
    if (
      (otherR.t === RT_PREV_SAME || otherR.t === RT_NEXT_SAME || otherR.t === RT_ONLY_SAME) &&
      fp.optionValues[other][letterIdx(otherAns)] === qi
    )
      return `${Q(other)} is ${otherAns}, same answer as ${Q(qi)}`;
  }

  if (remainingCount(eliminated[qi]) === 1) return "only option left";

  return "";
}

export function explainLookahead(
  puzzle: Puzzle,
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  _eliminated: number[],
  result: LookaheadResult,
): ExplainStep[] {
  const qi = result.assumptionQi;
  const letter = result.assumptionAnswer;
  const n = fp.n;
  const steps: ExplainStep[] = [];

  const hypAnswers = answers.slice(0, n);
  const hypEliminated = _eliminated.slice(0, n);
  hypAnswers[qi] = letter;
  hypEliminated[qi] = 0b11111 ^ (1 << letterIdx(letter));

  const involvedQis = new Set<number>([qi]);
  const lines: string[] = [];

  for (const dr of result.chain) {
    const a = dr.action;
    if (a.type === "force") {
      involvedQis.add(a.questionIndex);
      const reason = briefForceReason(fp, hypAnswers, hypEliminated, a.questionIndex, a.letter);
      lines.push(
        reason
          ? `${Q(a.questionIndex)} must be ${a.letter} (${reason}).`
          : `${Q(a.questionIndex)} must be ${a.letter}.`,
      );
      hypEliminated[a.questionIndex] = 0b11111 ^ (1 << letterIdx(a.letter));
      hypAnswers[a.questionIndex] = a.letter;
    } else if (a.type === "eliminateMulti") {
      const qis: number[] = [];
      for (let i = 0; i < n; i++) {
        if ((a.questionMask >> i) & 1) { involvedQis.add(i); qis.push(i); }
      }
      const optLetters: string[] = [];
      for (let b = 0; b < 5; b++) {
        if ((a.optionMask >> b) & 1) optLetters.push(LETTERS[b]);
      }
      lines.push(`Eliminate ${optLetters.join(", ")} from ${qis.map(Q).join(", ")}.`);
      for (const q of qis) hypEliminated[q] |= a.optionMask;
    } else {
      involvedQis.add(a.questionIndex);
      const elimDetail = explainElimDetail(
        fp.questions[a.questionIndex],
        a.questionIndex,
        a.optionIndex,
        fp.optionValues[a.questionIndex][a.optionIndex],
        hypAnswers,
        hypEliminated,
        n,
      );
      if (elimDetail) {
        lines.push(`Eliminate ${Q(a.questionIndex)} option ${LETTERS[a.optionIndex]}: ${elimDetail.text}`);
      } else {
        console.error(`No explain for: ELIM ${Q(a.questionIndex)} option ${LETTERS[a.optionIndex]} (rule: ${dr.rule})`);
        lines.push(`Eliminate ${Q(a.questionIndex)} option ${LETTERS[a.optionIndex]}.`);
      }
      hypEliminated[a.questionIndex] |= 1 << a.optionIndex;
    }
  }

  const contradictionQi = result.contradictionQi;
  involvedQis.add(contradictionQi);
  let detail = `${Q(contradictionQi)} would be invalid`;
  const reason = explainInvalidDetail(fp, puzzle, hypAnswers, hypEliminated, contradictionQi);
  if (reason) {
    detail = reason.replace(" claims ", " would say ");
  }
  lines.push(`But ${detail}. Contradiction.`);
  lines.push(`So ${Q(qi)} can't be ${letter}.`);

  steps.push(simple(`Try looking at ${Q(qi)}.`));
  if (involvedQis.size > 1) {
    const qList = [...involvedQis].sort((a, b) => a - b).map(Q).join(", ");
    steps.push(simple(`Try looking at ${qList}.`));
  }
  steps.push(simple(`What if ${Q(qi)} is ${letter}?`));
  steps.push(complex(`What if ${Q(qi)} is ${letter}?`, lines));

  return steps;
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
  _puzzle: Puzzle,
  answers: (AnswerLetter | null)[],
  eliminated: number[],
  qi: number,
): string | null {
  const a = answers[qi];
  if (a == null) return null;
  const ai = letterIdx(a);
  const r = fp.questions[qi];
  const v = fp.optionValues[qi][ai];
  const n = fp.n;

  const pred = countPred(r);
  if (pred && r.t !== RT_MOST_COMMON_COUNT) {
    const [from, to] = countRange(r, n);
    const cr = countAnswers(answers, eliminated, pred, from, to);
    if (v != null) {
      if (cr.count > v)
        return `${Q(qi)} claims ${v} ${countRuleLabel(r, v)}, but there are already ${cr.count}`;
      if (cr.count + cr.remaining < v)
        return `${Q(qi)} claims ${v} ${countRuleLabel(r, v)}, but at most ${cr.count + cr.remaining} are possible`;
    }
  }

  if (r.t === RT_ANSWER_OF) {
    const target = answers[r.questionIndex];
    if (target != null && v != null && letterIdx(target) !== v)
      return `${Q(qi)} claims ${Q(r.questionIndex)}'s answer is ${LETTERS[v]}, but ${Q(r.questionIndex)} is answered ${target}`;
  }

  if (r.t === RT_LETTER_DIST) {
    const other = answers[r.questionIndex];
    if (other != null && v != null) {
      const dist = Math.abs(ai - letterIdx(other));
      if (dist !== v)
        return `${Q(qi)} claims letter distance ${v}, but ${a} is ${dist} letters from ${Q(r.questionIndex)}'s answer ${other}`;
    }
  }

  if (r.t === RT_UNIQUE) {
    for (let i = 0; i < n; i++) {
      if (i !== qi && answers[i] === a)
        return `${Q(qi)} claims ${a} is unique, but ${Q(i)} already has answer ${a}`;
    }
  }

  if (r.t === RT_CLOSEST_AFTER || r.t === RT_FIRST_WITH) {
    const label = r.t === RT_FIRST_WITH ? "first" : "closest";
    const scanStart = r.t === RT_CLOSEST_AFTER ? r.afterIndex + 1 : 0;
    if (v != null && v >= 0 && v < n && answers[v] != null && answers[v] !== r.answer)
      return `${Q(qi)} claims ${label} ${r.answer} is ${Q(v)}, but ${Q(v)} is answered ${answers[v]}`;
    if (v != null) {
      for (let j = scanStart; j < v; j++) {
        if (answers[j] === r.answer)
          return `${Q(qi)} claims ${label} ${r.answer} is ${Q(v)}, but ${Q(j)} has answer ${r.answer} and comes before ${Q(v)}`;
      }
    }
    if (v == null) {
      for (let j = scanStart; j < n; j++) {
        if (answers[j] === r.answer)
          return `${Q(qi)} claims no question has answer ${r.answer}, but ${Q(j)} does`;
      }
    }
  }

  if (r.t === RT_CLOSEST_BEFORE || r.t === RT_LAST_WITH) {
    const label = r.t === RT_LAST_WITH ? "last" : "closest";
    const beforeIdx = r.t === RT_CLOSEST_BEFORE ? r.beforeIndex : n;
    if (v != null && v >= 0 && v < n && answers[v] != null && answers[v] !== r.answer)
      return `${Q(qi)} claims ${label} ${r.answer} is ${Q(v)}, but ${Q(v)} is answered ${answers[v]}`;
    if (v != null) {
      for (let j = beforeIdx - 1; j > v; j--) {
        if (answers[j] === r.answer)
          return `${Q(qi)} claims ${label} ${r.answer} is ${Q(v)}, but ${Q(j)} has answer ${r.answer} and comes after ${Q(v)}`;
      }
    }
    if (v == null) {
      for (let j = 0; j < beforeIdx; j++) {
        if (answers[j] === r.answer)
          return `${Q(qi)} claims no question has answer ${r.answer}, but ${Q(j)} does`;
      }
    }
  }

  if (r.t === RT_SAME_AS) {
    if (v != null && v >= 0 && v < n && answers[v] != null && answers[v] !== a)
      return `${Q(qi)} claims same answer as ${Q(v)}, but ${Q(v)} is ${answers[v]} and ${Q(qi)} is ${a}`;
  }

  return null;
}
