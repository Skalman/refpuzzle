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
  RT_LEAST_COMMON,
  RT_MOST_COMMON,
  RT_LETTER_DIST,
  RT_TRUE_STMT,
  RT_SAME_AS_WHICH,
} from "./types.ts";
import type { DeduceResult } from "./deduce.ts";
import type { LookaheadResult } from "./lookahead.ts";

// ── Formatting helpers ──

function Q(i: number): string {
  return `#${i + 1}`;
}

type Pred = (a: AnswerLetter) => boolean;

function countPred(q: { t: number; answer: string | null }): Pred | null {
  switch (q.t) {
    case RT_COUNT_ANSWER:
    case RT_COUNT_ANSWER_BEFORE:
    case RT_COUNT_ANSWER_AFTER:
      return (a) => a === q.answer;
    case RT_COUNT_VOWEL:
      return (a) => VOWELS.has(a);
    case RT_COUNT_CONSONANT:
      return (a) => !VOWELS.has(a);
    default:
      return null;
  }
}

function countRange(
  q: { t: number; afterIndex: number; beforeIndex: number },
  n: number,
): [number, number] {
  if (q.t === RT_COUNT_ANSWER_BEFORE) return [0, q.beforeIndex];
  if (q.t === RT_COUNT_ANSWER_AFTER) return [q.afterIndex + 1, n];
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
  q: {
    t: number;
    answer: string | null;
    afterIndex: number;
    beforeIndex: number;
  },
  count: number,
): string {
  const qs = count === 1 ? "question" : "questions";
  switch (q.t) {
    case RT_COUNT_ANSWER:
      return `${qs} with answer ${q.answer}`;
    case RT_COUNT_ANSWER_BEFORE:
      return `${qs} before #${q.beforeIndex + 1} with answer ${q.answer}`;
    case RT_COUNT_ANSWER_AFTER:
      return `${qs} after #${q.afterIndex + 1} with answer ${q.answer}`;
    case RT_COUNT_VOWEL:
      return `${qs} with a vowel answer`;
    case RT_COUNT_CONSONANT:
      return `${qs} with a consonant answer`;
    default:
      return `matching ${qs}`;
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
        throw new Error(`No positional_range explain for ${qList} option ${optStr}`);
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
  const q = fp.questions[qi];
  const n = fp.n;

  if (remainingCount(eliminated[qi]) === 1) {
    steps.push(simple(`${Q(qi)} has only one option left — it must be ${letter}.`));
    return steps;
  }

  if (q.t === RT_ANSWER_OF && answers[q.questionIndex] != null) {
    steps.push(simple(`Try looking at ${Q(qi)} and ${Q(q.questionIndex)}.`));
    steps.push(
      simple(
        `${Q(qi)} asks for ${Q(q.questionIndex)}'s answer. ${Q(q.questionIndex)} is ${answers[q.questionIndex]}, so ${Q(qi)} must be ${letter}.`,
      ),
    );
    return steps;
  }

  for (let other = 0; other < n; other++) {
    const otherAns = answers[other];
    if (otherAns == null) continue;
    const otherR = fp.questions[other];
    if (otherR.t === RT_SAME_AS && fp.optionValues[other][letterIdx(otherAns)] === qi) {
      steps.push(simple(`Try looking at ${Q(qi)} and ${Q(other)}.`));
      steps.push(
        simple(
          `${Q(other)} says it has the same answer as ${Q(qi)}. ${Q(other)} is ${otherAns}, so ${Q(qi)} must be ${otherAns}.`,
        ),
      );
      return steps;
    }
    if (
      (otherR.t === RT_PREV_SAME || otherR.t === RT_NEXT_SAME || otherR.t === RT_ONLY_SAME) &&
      fp.optionValues[other][letterIdx(otherAns)] === qi
    ) {
      steps.push(simple(`Try looking at ${Q(qi)} and ${Q(other)}.`));
      steps.push(
        simple(
          `${Q(other)} is ${otherAns}, pointing to ${Q(qi)} as having the same answer. So ${Q(qi)} must be ${otherAns}.`,
        ),
      );
      return steps;
    }
  }

  // SameAsWhich reverse: answered SameAsWhich question propagates equality
  for (let other = 0; other < n; other++) {
    const otherAns = answers[other];
    if (otherAns == null) continue;
    const otherR = fp.questions[other];
    if (otherR.t === RT_SAME_AS_WHICH) {
      const targetQ = fp.optionValues[other][letterIdx(otherAns)];
      const refQ = otherR.questionIndex;
      if (targetQ != null && targetQ >= 0 && targetQ < n) {
        if (targetQ === qi && answers[refQ] != null) {
          steps.push(simple(`Try looking at ${Q(qi)} and ${Q(other)}.`));
          steps.push(
            simple(
              `${Q(other)} is ${otherAns}, pointing to ${Q(qi)} as having the same answer as ${Q(refQ)} (${answers[refQ]}). So ${Q(qi)} must be ${letter}.`,
            ),
          );
          return steps;
        }
        if (refQ === qi && answers[targetQ] != null) {
          steps.push(simple(`Try looking at ${Q(qi)} and ${Q(other)}.`));
          steps.push(
            simple(
              `${Q(other)} is ${otherAns}, pointing to ${Q(targetQ)} as having the same answer as ${Q(qi)}. ${Q(targetQ)} is ${answers[targetQ]}, so ${Q(qi)} must be ${letter}.`,
            ),
          );
          return steps;
        }
      }
    }
  }

  // Reverse AnswerOf: another question says qi's answer
  for (let other = 0; other < n; other++) {
    const otherAns = answers[other];
    if (otherAns == null) continue;
    const otherR = fp.questions[other];
    if (otherR.t === RT_ANSWER_OF && otherR.questionIndex === qi) {
      steps.push(simple(`Try looking at ${Q(qi)} and ${Q(other)}.`));
      steps.push(
        simple(
          `${Q(other)} asks for ${Q(qi)}'s answer. ${Q(other)} is ${otherAns}, telling us ${Q(qi)} must be ${letter}.`,
        ),
      );
      return steps;
    }
  }

  // LetterDist: target answered, only one valid distance
  if (q.t === RT_LETTER_DIST && answers[q.questionIndex] != null) {
    steps.push(simple(`Try looking at ${Q(qi)} and ${Q(q.questionIndex)}.`));
    steps.push(
      simple(
        `${Q(q.questionIndex)} is answered ${answers[q.questionIndex]}. Only option ${letter} gives the right letter distance.`,
      ),
    );
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
      steps.push(
        simple(
          `${Q(src)} is answered ${srcAns} with letter distance ${dist}. Only ${letter} is at distance ${dist} from ${srcAns}, so ${Q(qi)} must be ${letter}.`,
        ),
      );
      return steps;
    }
  }

  // Counting: all in range answered, count determines answer
  const pred = countPred(q);
  if (pred) {
    const [from, to] = countRange(q, n);
    const cr = countAnswers(answers, eliminated, pred, from, to);
    if (cr.remaining === 0) {
      steps.push(
        simple(
          `There are ${cr.count} ${countRuleLabel(q, cr.count)}, so ${Q(qi)} must be ${letter}.`,
        ),
      );
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
      steps.push(
        simple(
          `${Q(src)} says there are ${srcVal} ${countRuleLabel(srcR, srcVal)}. Only ${cr.count} found so far, and ${Q(qi)} is the only remaining question that could be ${letter} — so ${Q(qi)} must be ${letter}.`,
        ),
      );
      return steps;
    }
  }

  if (rule === "LeastCommonForce" && q.t === RT_LEAST_COMMON) {
    steps.push(
      simple(
        `Only one answer can make its claimed letter the least common — ${Q(qi)} must be ${letter}.`,
      ),
    );
    return steps;
  }

  if (rule === "MostCommonForce" && q.t === RT_MOST_COMMON) {
    steps.push(
      simple(
        `Only one answer can make its claimed letter the most common — ${Q(qi)} must be ${letter}.`,
      ),
    );
    return steps;
  }

  if (rule === "ConsecIdentForwardForce" || rule === "ConsecIdentForwardBothForce") {
    for (let src = 0; src < n; src++) {
      if (fp.questions[src].t !== RT_CONSEC_IDENT || answers[src] == null) continue;
      const srcV = fp.optionValues[src][letterIdx(answers[src]!)];
      if (srcV == null) continue;
      const p = srcV;
      if (p === qi || p + 1 === qi) {
        const partner = p === qi ? p + 1 : p;
        steps.push(simple(`Try looking at ${Q(qi)} and ${Q(src)}.`));
        if (answers[partner] != null) {
          steps.push(
            simple(
              `${Q(src)} says ${Q(p)} and ${Q(p + 1)} have the same answer. ${Q(partner)} is ${answers[partner]}, so ${Q(qi)} must be ${letter}.`,
            ),
          );
        } else {
          steps.push(
            simple(
              `${Q(src)} says ${Q(p)} and ${Q(p + 1)} have the same answer. Only ${letter} is possible for both, so ${Q(qi)} must be ${letter}.`,
            ),
          );
        }
        return steps;
      }
    }
  }

  if (rule === "TrueStatementForward") {
    for (let src = 0; src < n; src++) {
      const srcAns = answers[src];
      if (srcAns == null) continue;
      if (fp.questions[src].t !== RT_TRUE_STMT) continue;
      const claim = fp.optionClaims[src][letterIdx(srcAns)];
      if (!claim) continue;
      if (claim.type === "AnswerOf" && claim.questionIndex === qi) {
        steps.push(simple(`Try looking at ${Q(qi)} and ${Q(src)}.`));
        steps.push(
          simple(
            `${Q(src)}'s true statement says ${Q(qi)}'s answer is ${letter}. So ${Q(qi)} must be ${letter}.`,
          ),
        );
        return steps;
      }
      if ((claim.type === "FirstWith" || claim.type === "LastWith") && claim.value === qi) {
        steps.push(simple(`Try looking at ${Q(qi)} and ${Q(src)}.`));
        steps.push(
          simple(
            `${Q(src)}'s true statement says ${Q(qi)} has answer ${letter}. So ${Q(qi)} must be ${letter}.`,
          ),
        );
        return steps;
      }
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
    const q = fp.questions[src];
    const pred = countPred(q);
    if (!pred || !pred(targetLetter)) continue;
    const ai = letterIdx(answers[src]!);
    const value = fp.optionValues[src][ai];
    if (value == null) continue;
    const [from, to] = countRange(q, n);
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
  const q = fp.questions[qi];
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

  if (rule === "TrueStatementClaimInvalid") {
    const claim = fp.optionClaims[qi][oi];
    if (
      claim &&
      (claim.type === "FirstWith" || claim.type === "LastWith") &&
      claim.value < n &&
      answers[claim.value] != null
    ) {
      steps.push(simple(`Try looking at ${Q(qi)} and ${Q(claim.value)}.`));
      steps.push(simple(`What if ${Q(qi)} is ${letter}?`));
      steps.push(
        simple(
          `${Q(qi)} option ${letter}'s statement says ${Q(claim.value)} has answer ${claim.answer}, but ${Q(claim.value)} is ${answers[claim.value]}.`,
        ),
      );
      return steps;
    }
    if (
      claim &&
      claim.type === "AnswerOf" &&
      claim.questionIndex < n &&
      answers[claim.questionIndex] != null
    ) {
      steps.push(simple(`Try looking at ${Q(qi)} and ${Q(claim.questionIndex)}.`));
      steps.push(simple(`What if ${Q(qi)} is ${letter}?`));
      steps.push(
        simple(
          `${Q(qi)} option ${letter}'s statement says ${Q(claim.questionIndex)}'s answer is ${LETTERS[claim.value]}, but ${Q(claim.questionIndex)} is ${answers[claim.questionIndex]}.`,
        ),
      );
      return steps;
    }
    steps.push(simple(`What if ${Q(qi)} is ${letter}?`));
    steps.push(
      simple(`${Q(qi)} option ${letter}'s statement is contradicted by the current answers.`),
    );
    return steps;
  }

  if (rule === "TrueStatementSelfRef") {
    const claim = fp.optionClaims[qi][oi];
    if (claim && (claim.type === "FirstWith" || claim.type === "LastWith") && claim.value === qi) {
      steps.push(simple(`What if ${Q(qi)} is ${letter}?`));
      steps.push(
        simple(
          `${Q(qi)} option ${letter}'s statement says ${Q(qi)} has answer ${claim.answer}, but that contradicts ${Q(qi)} being ${letter}.`,
        ),
      );
      return steps;
    }
    if (claim && claim.type === "AnswerOf" && claim.questionIndex === qi) {
      steps.push(simple(`What if ${Q(qi)} is ${letter}?`));
      steps.push(
        simple(
          `${Q(qi)} option ${letter}'s statement says ${Q(qi)}'s answer is ${LETTERS[claim.value]}, but that contradicts ${Q(qi)} being ${letter}.`,
        ),
      );
      return steps;
    }
    steps.push(simple(`What if ${Q(qi)} is ${letter}?`));
    steps.push(simple(`${Q(qi)} option ${letter}'s statement contradicts itself.`));
    return steps;
  }

  if (rule === "OnlySameNoneForward") {
    for (let src = 0; src < n; src++) {
      if (fp.questions[src].t !== RT_ONLY_SAME || answers[src] == null) continue;
      const srcV = fp.optionValues[src][letterIdx(answers[src]!)];
      if (srcV != null) continue;
      if (answers[src] === letter) {
        steps.push(simple(`Try looking at ${Q(qi)} and ${Q(src)}.`));
        steps.push(simple(`What if ${Q(qi)} is ${letter}?`));
        steps.push(
          simple(
            `${Q(src)} is ${letter} and claims no other question shares that answer, so ${Q(qi)} can't be ${letter}.`,
          ),
        );
        return steps;
      }
    }
  }

  if (rule === "ConsecIdentForwardElim") {
    for (let src = 0; src < n; src++) {
      if (fp.questions[src].t !== RT_CONSEC_IDENT || answers[src] == null) continue;
      const srcV = fp.optionValues[src][letterIdx(answers[src]!)];
      if (srcV == null) continue;
      const p = srcV;
      if (p === qi || p + 1 === qi) {
        const partner = p === qi ? p + 1 : p;
        steps.push(simple(`Try looking at ${Q(qi)}, ${Q(partner)}, and ${Q(src)}.`));
        steps.push(simple(`What if ${Q(qi)} is ${letter}?`));
        steps.push(
          simple(
            `${Q(src)} says ${Q(p)} and ${Q(p + 1)} must have the same answer, but ${letter} is ruled out for ${Q(partner)}.`,
          ),
        );
        return steps;
      }
    }
  }

  if (rule === "ConsecIdentReverse") {
    for (let src = 0; src < n; src++) {
      if (fp.questions[src].t !== RT_CONSEC_IDENT) continue;
      const neighbor =
        qi > 0 && answers[qi - 1] === letter
          ? qi - 1
          : qi + 1 < n && answers[qi + 1] === letter
            ? qi + 1
            : null;
      if (neighbor != null) {
        steps.push(simple(`Try looking at ${Q(qi)}, ${Q(neighbor)}, and ${Q(src)}.`));
        steps.push(simple(`What if ${Q(qi)} is ${letter}?`));
        steps.push(
          simple(
            `${Q(qi)} and ${Q(neighbor)} would both be ${letter}, creating a consecutive pair — but ${Q(src)}'s remaining options don't allow that pair.`,
          ),
        );
      } else {
        steps.push(simple(`What if ${Q(qi)} is ${letter}?`));
        steps.push(
          simple(
            `That would create a consecutive pair not allowed by ${Q(src)}'s remaining options.`,
          ),
        );
      }
      return steps;
    }
  }

  if (rule === "VowelCrossElim" || rule === "ConsonantCrossElim") {
    steps.push(simple(`What if ${Q(qi)} is ${letter}?`));
    steps.push(
      simple(`${Q(qi)} option ${letter}: no compatible option exists on the other counting rule.`),
    );
    return steps;
  }

  const detail = explainElimDetail(q, qi, oi, v, answers, eliminated, n);
  if (detail && detail.otherQi != null) {
    steps.push(simple(`Try looking at ${Q(qi)} and ${Q(detail.otherQi)}.`));
  }
  steps.push(simple(`What if ${Q(qi)} is ${letter}?`));
  if (!detail)
    throw new Error(`No explainElimDetail for ${Q(qi)} option ${letter} (rule: ${rule})`);
  steps.push(simple(detail.text));
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
    const q = fp.questions[src];
    const pred = countPred(q);
    if (!pred) continue;
    const ai = letterIdx(answers[src]!);
    const value = fp.optionValues[src][ai];
    if (value == null) continue;
    const [from, to] = countRange(q, n);
    const cr = countAnswers(answers, eliminated, pred, from, to);
    if (cr.count === value && pred(LETTERS[oi])) {
      return {
        srcQi: src,
        text: `${Q(src)} says there are ${value} ${countRuleLabel(q, value)}. There are already ${value}, so ${Q(qi)} can't also be ${LETTERS[oi]}.`,
      };
    }
    if (cr.count + cr.remaining === value && cr.remaining > 0 && !pred(LETTERS[oi])) {
      return {
        srcQi: src,
        text: `${Q(src)} says there are ${value} ${countRuleLabel(q, value)}. Only ${cr.count} found so far, and all remaining unknowns must match — so ${Q(qi)} can't be ${LETTERS[oi]}.`,
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
  for (let src = 0; src < n; src++) {
    if (src === qi) continue;
    const srcR = fp.questions[src];
    if ((srcR.t === RT_ONLY_ODD || srcR.t === RT_ONLY_EVEN) && srcR.answer != null) {
      const parity = srcR.t === RT_ONLY_ODD ? "odd" : "even";
      return {
        text: `${Q(src)} asks for the only ${parity}-numbered question with answer ${srcR.answer}, limiting which ${parity} questions can have that answer.`,
        otherQi: src,
      };
    }
  }
  throw new Error(`No explainMultiElim source for ${Q(qi)}`);
}

interface ElimDetail {
  text: string;
  otherQi: number | null;
}

function d(text: string, otherQi: number | null = null): ElimDetail {
  return { text, otherQi };
}

function explainElimDetail(
  q: {
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
  const pred = countPred(q);
  if (pred && q.t !== RT_MOST_COMMON_COUNT) {
    const [from, to] = countRange(q, n);
    const cr = countAnswers(answers, eliminated, pred, from, to);
    if (v != null) {
      if (cr.count > v)
        return d(
          `${Q(qi)} option ${letter} claims ${v} ${countRuleLabel(q, v)}, but there are already ${cr.count}.`,
        );
      if (cr.count + cr.remaining < v)
        return d(
          `${Q(qi)} option ${letter} claims ${v} ${countRuleLabel(q, v)}, but at most ${cr.count + cr.remaining} are possible.`,
        );
    }
  }

  if (q.t === RT_ANSWER_OF) {
    if (v != null && v >= 0 && v < 5 && isElim(eliminated, q.questionIndex, v))
      return d(
        `${Q(qi)} option ${letter} claims ${Q(q.questionIndex)}'s answer is ${LETTERS[v]}, but ${LETTERS[v]} is ruled out for ${Q(q.questionIndex)}.`,
        q.questionIndex,
      );
  }

  if (q.t === RT_SAME_AS_WHICH) {
    const refAns = answers[q.questionIndex];
    if (refAns != null && v != null && v >= 0 && v < n) {
      const targetAns = answers[v];
      if (targetAns != null && targetAns !== refAns)
        return d(
          `${Q(qi)} option ${letter} claims ${Q(v)} has the same answer as ${Q(q.questionIndex)} (${refAns}), but ${Q(v)} is answered ${targetAns}.`,
          v,
        );
      if (targetAns == null && isElim(eliminated, v, letterIdx(refAns)))
        return d(
          `${Q(qi)} option ${letter} claims ${Q(v)} has the same answer as ${Q(q.questionIndex)} (${refAns}), but ${refAns} is ruled out for ${Q(v)}.`,
          v,
        );
    }
  }

  if (q.t === RT_LETTER_DIST) {
    const maxDist = Math.max(oi, 4 - oi);
    if (v != null && v > maxDist)
      return d(
        `${Q(qi)} option ${letter} claims letter distance ${v}, but ${letter} can be at most ${maxDist} letters from any answer.`,
      );
    const other = answers[q.questionIndex];
    if (other != null && v != null) {
      const dist = Math.abs(oi - letterIdx(other));
      if (dist !== v)
        return d(
          `${Q(qi)} option ${letter} claims letter distance ${v}, but ${letter} is ${dist} letters from ${Q(q.questionIndex)}'s answer ${other}.`,
          q.questionIndex,
        );
    }
    if (other == null && v != null) {
      let anyPossible = false;
      for (let ti = 0; ti < 5; ti++) {
        if (!isElim(eliminated, q.questionIndex, ti) && Math.abs(oi - ti) === v) anyPossible = true;
      }
      if (!anyPossible)
        return d(
          `${Q(qi)} option ${letter} claims letter distance ${v}, but no remaining answer for ${Q(q.questionIndex)} gives that distance from ${letter}.`,
          q.questionIndex,
        );
    }
  }

  if (q.t === RT_CLOSEST_AFTER || q.t === RT_FIRST_WITH) {
    const label = q.t === RT_FIRST_WITH ? "first" : "closest";
    const scanStart = q.t === RT_CLOSEST_AFTER ? q.afterIndex + 1 : 0;
    if (v != null) {
      if (v < scanStart || v >= n)
        return d(
          `${Q(qi)} option ${letter} claims ${label} ${q.answer} is #${v + 1}, but that's out of range.`,
        );
      if (answers[v] != null && answers[v] !== q.answer)
        return d(
          `${Q(qi)} option ${letter} claims ${label} ${q.answer} is ${Q(v)}, but ${Q(v)} is answered ${answers[v]}.`,
          v,
        );
      if (answers[v] == null && isElim(eliminated, v, L2I[q.answer!]))
        return d(
          `${Q(qi)} option ${letter} claims ${label} ${q.answer} is ${Q(v)}, but ${q.answer} is ruled out for ${Q(v)}.`,
          v,
        );
      for (let j = scanStart; j < v; j++) {
        if (answers[j] === q.answer)
          return d(
            `${Q(qi)} option ${letter} claims ${label} ${q.answer} is ${Q(v)}, but ${Q(j)} already has answer ${q.answer} and comes before ${Q(v)}.`,
            j,
          );
      }
      if (LETTERS[oi] === q.answer && qi >= scanStart && qi < v)
        return d(
          `${Q(qi)} option ${letter} claims ${label} ${q.answer} is ${Q(v)}, but ${Q(qi)} itself is before ${Q(v)} and would have answer ${q.answer}. Contradiction.`,
        );
    } else {
      for (let j = scanStart; j < n; j++) {
        if (answers[j] === q.answer)
          return d(
            `${Q(qi)} option ${letter} claims no question has answer ${q.answer}, but ${Q(j)} has answer ${q.answer}.`,
            j,
          );
      }
    }
  }

  if (q.t === RT_CLOSEST_BEFORE || q.t === RT_LAST_WITH) {
    const label = q.t === RT_LAST_WITH ? "last" : "closest";
    const beforeIdx = q.t === RT_CLOSEST_BEFORE ? q.beforeIndex : n;
    if (v != null) {
      if (v < 0 || v >= beforeIdx)
        return d(
          `${Q(qi)} option ${letter} claims ${label} ${q.answer} is #${v + 1}, but that's out of range.`,
        );
      if (answers[v] != null && answers[v] !== q.answer)
        return d(
          `${Q(qi)} option ${letter} claims ${label} ${q.answer} is ${Q(v)}, but ${Q(v)} is answered ${answers[v]}.`,
          v,
        );
      if (answers[v] == null && isElim(eliminated, v, L2I[q.answer!]))
        return d(
          `${Q(qi)} option ${letter} claims ${label} ${q.answer} is ${Q(v)}, but ${q.answer} is ruled out for ${Q(v)}.`,
          v,
        );
      for (let j = beforeIdx - 1; j > v; j--) {
        if (answers[j] === q.answer)
          return d(
            `${Q(qi)} option ${letter} claims ${label} ${q.answer} is ${Q(v)}, but ${Q(j)} has answer ${q.answer} and comes after ${Q(v)}.`,
            j,
          );
      }
      if (LETTERS[oi] === q.answer && qi > v && qi < beforeIdx)
        return d(
          `${Q(qi)} option ${letter} claims ${label} ${q.answer} is ${Q(v)}, but ${Q(qi)} itself is after ${Q(v)} and would have answer ${q.answer}. Contradiction.`,
        );
    } else {
      for (let j = 0; j < beforeIdx; j++) {
        if (answers[j] === q.answer)
          return d(
            `${Q(qi)} option ${letter} claims no question has answer ${q.answer}, but ${Q(j)} has answer ${q.answer}.`,
            j,
          );
      }
    }
  }

  if (q.t === RT_ONLY_ODD || q.t === RT_ONLY_EVEN) {
    const parity = q.t === RT_ONLY_ODD ? 1 : 0;
    const parityName = q.t === RT_ONLY_ODD ? "even" : "odd";
    if (v != null) {
      if ((v + 1) % 2 !== parity)
        return d(
          `${Q(qi)} option ${letter} claims ${Q(v)}, but ${Q(v)} is ${parityName}-numbered.`,
        );
      if (v >= 0 && v < n && answers[v] != null && answers[v] !== q.answer)
        return d(
          `${Q(qi)} option ${letter} claims ${Q(v)} has answer ${q.answer}, but ${Q(v)} is answered ${answers[v]}.`,
          v,
        );
      if (v >= 0 && v < n && answers[v] == null && isElim(eliminated, v, L2I[q.answer!]))
        return d(
          `${Q(qi)} option ${letter} claims ${Q(v)} has answer ${q.answer}, but ${q.answer} is ruled out for ${Q(v)}.`,
          v,
        );
    } else {
      for (let i = 0; i < n; i++) {
        if ((i + 1) % 2 === parity && answers[i] === q.answer)
          return d(
            `${Q(qi)} option ${letter} claims no ${parity === 1 ? "odd" : "even"}-numbered question has answer ${q.answer}, but ${Q(i)} does.`,
            i,
          );
      }
    }
  }

  if (q.t === RT_CONSEC_IDENT) {
    if (v != null && v + 1 >= n)
      return d(
        `${Q(qi)} option ${letter} claims ${Q(v)} and ${Q(v + 1)}, but that's out of range.`,
      );
    if (v != null && v + 1 < n) {
      if (v === qi || v + 1 === qi) {
        const partner = v === qi ? v + 1 : v;
        if (isElim(eliminated, partner, oi))
          return d(
            `${Q(qi)} option ${letter} claims ${Q(v)} and ${Q(v + 1)} are the consecutive pair, but ${letter} is ruled out for ${Q(partner)} so they can't match.`,
            partner,
          );
      }
      const possA = ~eliminated[v] & 0b11111;
      const possB = ~eliminated[v + 1] & 0b11111;
      if ((possA & possB) === 0)
        return d(
          `${Q(qi)} option ${letter} claims ${Q(v)} and ${Q(v + 1)} are the consecutive pair, but they share no possible answer.`,
          v,
        );
    } else if (v == null) {
      for (let i = 0; i < n - 1; i++) {
        if (answers[i] != null && answers[i + 1] != null && answers[i] === answers[i + 1])
          return d(
            `${Q(qi)} option ${letter} claims no consecutive pair exists, but ${Q(i)} and ${Q(i + 1)} both have answer ${answers[i]}.`,
            i,
          );
      }
    }
  }

  if (q.t === RT_PREV_SAME && v == null) {
    for (let j = 0; j < qi; j++) {
      if (answers[j] === letter)
        return d(
          `${Q(qi)} option ${letter} claims no previous question has answer ${letter}, but ${Q(j)} does.`,
          j,
        );
    }
  }

  if (q.t === RT_PREV_SAME && v != null) {
    if (v >= qi)
      return d(`${Q(qi)} option ${letter} claims ${Q(v)}, but ${Q(v)} is not before ${Q(qi)}.`);
    if (isElim(eliminated, v, oi))
      return d(
        `${Q(qi)} option ${letter} claims ${Q(v)} has the same answer, but ${letter} is ruled out for ${Q(v)}.`,
        v,
      );
    for (let j = qi - 1; j > v; j--) {
      if (answers[j] === LETTERS[oi])
        return d(
          `${Q(qi)} option ${letter} claims previous same answer is ${Q(v)}, but ${Q(j)} also has answer ${letter} and is closer.`,
          j,
        );
    }
  }

  if (q.t === RT_NEXT_SAME && v == null) {
    for (let j = qi + 1; j < n; j++) {
      if (answers[j] === letter)
        return d(
          `${Q(qi)} option ${letter} claims no later question has answer ${letter}, but ${Q(j)} does.`,
          j,
        );
    }
  }

  if (q.t === RT_NEXT_SAME && v != null) {
    if (v <= qi || v >= n)
      return d(`${Q(qi)} option ${letter} claims ${Q(v)}, but ${Q(v)} is not after ${Q(qi)}.`);
    if (isElim(eliminated, v, oi))
      return d(
        `${Q(qi)} option ${letter} claims ${Q(v)} has the same answer, but ${letter} is ruled out for ${Q(v)}.`,
        v,
      );
    for (let j = qi + 1; j < v; j++) {
      if (answers[j] === LETTERS[oi])
        return d(
          `${Q(qi)} option ${letter} claims next same answer is ${Q(v)}, but ${Q(j)} also has answer ${letter} and is closer.`,
          j,
        );
    }
  }

  if (q.t === RT_ONLY_SAME && v == null) {
    for (let j = 0; j < n; j++) {
      if (j !== qi && answers[j] === letter)
        return d(
          `${Q(qi)} option ${letter} claims no other question has answer ${letter}, but ${Q(j)} does.`,
          j,
        );
    }
  }

  if ((q.t === RT_ONLY_SAME || q.t === RT_SAME_AS) && v != null) {
    if (v === qi)
      return d(
        `${Q(qi)} option ${letter} points to ${Q(qi)} itself, but a question can't share an answer with itself.`,
      );
    if (v >= 0 && v < n && isElim(eliminated, v, oi))
      return d(
        `${Q(qi)} option ${letter} claims ${Q(v)} has the same answer, but ${letter} is ruled out for ${Q(v)}.`,
        v,
      );
    if (q.t === RT_ONLY_SAME && v >= 0 && v < n && v !== qi) {
      for (let j = 0; j < n; j++) {
        if (j !== qi && j !== v && answers[j] === letter)
          return d(
            `${Q(qi)} option ${letter} claims ${Q(v)} is the only other with answer ${letter}, but ${Q(j)} already has answer ${letter}.`,
            j,
          );
      }
    }
  }

  if (q.t === RT_UNIQUE) {
    for (let i = 0; i < n; i++) {
      if (answers[i] === letter)
        return d(
          `${Q(qi)} option ${letter} claims ${letter} is unique, but ${Q(i)} already has answer ${letter}.`,
          i,
        );
    }
  }

  if (q.t === RT_EQUAL_COUNT) {
    if (v != null && LETTERS[v] === q.answer)
      return d(
        `${Q(qi)} option ${letter} claims ${LETTERS[v]}, but the question asks for a different letter with the same count as ${q.answer}.`,
      );
    if (v != null && v >= 0 && v < 5) {
      const claimed = LETTERS[v];
      let rc = 0,
        rr = 0,
        sc = 0,
        sr = 0;
      const refMask = 1 << letterIdx(q.answer!);
      const claimedMask = 1 << v;
      for (let j = 0; j < n; j++) {
        if (answers[j] != null) {
          if (answers[j] === q.answer) rc++;
          if (answers[j] === claimed) sc++;
        } else {
          if ((eliminated[j] & refMask) === 0) rr++;
          if ((eliminated[j] & claimedMask) === 0) sr++;
        }
      }
      if (rc + rr < sc)
        return d(
          `${Q(qi)} option ${letter} claims ${claimed} has the same count as ${q.answer}, but ${q.answer} can have at most ${rc + rr} while ${claimed} already has ${sc}.`,
        );
      if (sc + sr < rc)
        return d(
          `${Q(qi)} option ${letter} claims ${claimed} has the same count as ${q.answer}, but ${claimed} can have at most ${sc + sr} while ${q.answer} already has ${rc}.`,
        );
    }
  }

  if (q.t === RT_LEAST_COMMON && v != null && v < 5) {
    const counts = [0, 0, 0, 0, 0];
    for (let j = 0; j < n; j++) {
      if (answers[j] != null) counts[letterIdx(answers[j]!)]++;
    }
    const claimed = LETTERS[v];
    const minCount = Math.min(...counts);
    const minLetters = LETTERS.filter((_, i) => counts[i] === minCount);
    if (counts[v] > minCount) {
      return d(
        `${Q(qi)} option ${letter} claims ${claimed} is the least common, but ${claimed} appears ${counts[v]} time(s) while ${minLetters[0]} appears only ${minCount}.`,
      );
    }
    if (minLetters.length > 1) {
      return d(
        `${Q(qi)} option ${letter} claims ${claimed} is the least common, but ${minLetters.join(" and ")} are tied at ${minCount} — no unique least.`,
      );
    }
    return d(
      `${Q(qi)} option ${letter} claims ${claimed} is the least common, but ${claimed} can't be uniquely least.`,
    );
  }

  if (q.t === RT_MOST_COMMON && v != null && v < 5) {
    const counts = [0, 0, 0, 0, 0];
    for (let j = 0; j < n; j++) {
      if (answers[j] != null) counts[letterIdx(answers[j]!)]++;
    }
    const claimed = LETTERS[v];
    const maxCount = Math.max(...counts);
    const maxLetters = LETTERS.filter((_, i) => counts[i] === maxCount);
    if (counts[v] < maxCount) {
      return d(
        `${Q(qi)} option ${letter} claims ${claimed} is the most common, but ${claimed} appears ${counts[v]} time(s) while ${maxLetters[0]} appears ${maxCount}.`,
      );
    }
    if (maxLetters.length > 1) {
      return d(
        `${Q(qi)} option ${letter} claims ${claimed} is the most common, but ${maxLetters.join(" and ")} are tied at ${maxCount} — no unique most.`,
      );
    }
    return d(
      `${Q(qi)} option ${letter} claims ${claimed} is the most common, but ${claimed} can't be uniquely most.`,
    );
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
  const q = fp.questions[qi];
  const n = fp.n;

  if (q.t === RT_ANSWER_OF && answers[q.questionIndex] != null)
    return `${Q(q.questionIndex)} is ${answers[q.questionIndex]}`;

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
        if ((a.questionMask >> i) & 1) {
          involvedQis.add(i);
          qis.push(i);
        }
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
        lines.push(
          `Eliminate ${Q(a.questionIndex)} option ${LETTERS[a.optionIndex]}: ${elimDetail.text}`,
        );
      } else {
        throw new Error(
          `No explain for: ELIM ${Q(a.questionIndex)} option ${LETTERS[a.optionIndex]} (rule: ${dr.rule})`,
        );
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
    const qList = [...involvedQis]
      .sort((a, b) => a - b)
      .map(Q)
      .join(", ");
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
  const q = fp.questions[qi];
  const v = fp.optionValues[qi][ai];
  const n = fp.n;

  const pred = countPred(q);
  if (pred && q.t !== RT_MOST_COMMON_COUNT) {
    const [from, to] = countRange(q, n);
    const cr = countAnswers(answers, eliminated, pred, from, to);
    if (v != null) {
      if (cr.count > v)
        return `${Q(qi)} claims ${v} ${countRuleLabel(q, v)}, but there are already ${cr.count}`;
      if (cr.count + cr.remaining < v)
        return `${Q(qi)} claims ${v} ${countRuleLabel(q, v)}, but at most ${cr.count + cr.remaining} are possible`;
    }
  }

  if (q.t === RT_ANSWER_OF) {
    const target = answers[q.questionIndex];
    if (target != null && v != null && letterIdx(target) !== v)
      return `${Q(qi)} claims ${Q(q.questionIndex)}'s answer is ${LETTERS[v]}, but ${Q(q.questionIndex)} is answered ${target}`;
  }

  if (q.t === RT_LETTER_DIST) {
    const other = answers[q.questionIndex];
    if (other != null && v != null) {
      const dist = Math.abs(ai - letterIdx(other));
      if (dist !== v)
        return `${Q(qi)} claims letter distance ${v}, but ${a} is ${dist} letters from ${Q(q.questionIndex)}'s answer ${other}`;
    }
  }

  if (q.t === RT_UNIQUE) {
    for (let i = 0; i < n; i++) {
      if (i !== qi && answers[i] === a)
        return `${Q(qi)} claims ${a} is unique, but ${Q(i)} already has answer ${a}`;
    }
  }

  if (q.t === RT_CLOSEST_AFTER || q.t === RT_FIRST_WITH) {
    const label = q.t === RT_FIRST_WITH ? "first" : "closest";
    const scanStart = q.t === RT_CLOSEST_AFTER ? q.afterIndex + 1 : 0;
    if (v != null && v >= 0 && v < n && answers[v] != null && answers[v] !== q.answer)
      return `${Q(qi)} claims ${label} ${q.answer} is ${Q(v)}, but ${Q(v)} is answered ${answers[v]}`;
    if (v != null) {
      for (let j = scanStart; j < v; j++) {
        if (answers[j] === q.answer)
          return `${Q(qi)} claims ${label} ${q.answer} is ${Q(v)}, but ${Q(j)} has answer ${q.answer} and comes before ${Q(v)}`;
      }
    }
    if (v == null) {
      for (let j = scanStart; j < n; j++) {
        if (answers[j] === q.answer)
          return `${Q(qi)} claims no question has answer ${q.answer}, but ${Q(j)} does`;
      }
    }
  }

  if (q.t === RT_CLOSEST_BEFORE || q.t === RT_LAST_WITH) {
    const label = q.t === RT_LAST_WITH ? "last" : "closest";
    const beforeIdx = q.t === RT_CLOSEST_BEFORE ? q.beforeIndex : n;
    if (v != null && v >= 0 && v < n && answers[v] != null && answers[v] !== q.answer)
      return `${Q(qi)} claims ${label} ${q.answer} is ${Q(v)}, but ${Q(v)} is answered ${answers[v]}`;
    if (v != null) {
      for (let j = beforeIdx - 1; j > v; j--) {
        if (answers[j] === q.answer)
          return `${Q(qi)} claims ${label} ${q.answer} is ${Q(v)}, but ${Q(j)} has answer ${q.answer} and comes after ${Q(v)}`;
      }
    }
    if (v == null) {
      for (let j = 0; j < beforeIdx; j++) {
        if (answers[j] === q.answer)
          return `${Q(qi)} claims no question has answer ${q.answer}, but ${Q(j)} does`;
      }
    }
  }

  if (q.t === RT_SAME_AS) {
    if (v != null && v >= 0 && v < n && answers[v] != null && answers[v] !== a)
      return `${Q(qi)} claims same answer as ${Q(v)}, but ${Q(v)} is ${answers[v]} and ${Q(qi)} is ${a}`;
  }

  return null;
}
