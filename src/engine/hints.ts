import type { AnswerLetter, Puzzle, FlatPuzzle, ValidationRule, Marks } from "./types.ts";
import {
  LETTERS,
  VOWELS,
  L2I,
  NONE,
  letterIdx,
  getFlatPuzzle,
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
  RT_ANSWER_OF,
  RT_LETTER_DIST,
  RT_UNIQUE,
  RT_SAME_AS,
} from "./types.ts";

type Action =
  | { type: "force"; questionIndex: number; letter: AnswerLetter }
  | { type: "eliminate"; questionIndex: number; optionIndex: number }
  | { type: "contradiction"; questionIndex: number }
  | null;

export interface Hint {
  steps: string[]; // progressive disclosure: vague → specific → full explanation
  action?:
    | { type: "force"; questionIndex: number; letter: AnswerLetter }
    | { type: "eliminate"; questionIndex: number; optionIndex: number }
    | { type: "contradiction"; questionIndex: number };
}

function hintSteps(
  qi: number,
  detail: string,
  explanation: string,
  action: Hint["action"],
  oi?: number,
  extraSteps?: string[],
): Hint {
  const steps = [`Try looking at ${Q(qi)}.`];
  if (oi != null) {
    steps.push(`Consider ${Q(qi)}, option ${LETTERS[oi]}.`);
  } else {
    steps.push(detail);
  }
  if (extraSteps) {
    for (const s of extraSteps) steps.push(s);
  }
  steps.push(explanation);
  return { steps, action };
}

export function findHint(puzzle: Puzzle, markSets: Marks[]): Hint | null {
  const n = puzzle.questions.length;
  const answers = deriveAnswers(markSets);

  return (
    findSimpleDeduction(puzzle, answers, markSets, n) ??
    findLookahead(puzzle, answers, markSets, n) ?? {
      steps: [
        "No obvious next step.",
        "Try making an assumption and see what follows.",
        "Pick the question with fewest remaining options and try each one.",
      ] as [string, string, string],
    }
  );
}

function findSimpleDeduction(
  puzzle: Puzzle,
  answers: (AnswerLetter | null)[],
  markSets: Marks[],
  n: number,
): Hint | null {
  return (
    findContradiction(puzzle, answers, n) ??
    findForced(puzzle, answers, markSets, n) ??
    findEliminable(puzzle, answers, markSets, n)
  );
}

function deriveAnswers(markSets: Marks[]): (AnswerLetter | null)[] {
  return markSets.map((m) => {
    const idx = m.indexOf("correct");
    return idx >= 0 ? LETTERS[idx] : null;
  });
}

function optVal(puzzle: Puzzle, qi: number, answer: AnswerLetter): string {
  return puzzle.questions[qi].options[L2I[answer]].label;
}

function Q(i: number): string {
  return `Q${i + 1}`;
}

function formatList(items: string[]): string {
  if (items.length <= 1) return items[0] ?? "";
  if (items.length === 2) return `${items[0]} and ${items[1]}`;
  return items.slice(0, -1).join(", ") + ", and " + items[items.length - 1];
}

// ── Counting helpers ──

interface CountResult {
  count: number;
  remaining: number;
}

function countMatching(
  answers: (AnswerLetter | null)[],
  pred: (a: AnswerLetter) => boolean,
  from: number,
  to: number,
): CountResult {
  let count = 0;
  let remaining = 0;
  for (let i = from; i < to && i < answers.length; i++) {
    const a = answers[i];
    if (a == null) remaining++;
    else if (pred(a)) count++;
  }
  return { count, remaining };
}

function countPred(rule: ValidationRule): ((a: AnswerLetter) => boolean) | null {
  switch (rule.type) {
    case "count_answer":
    case "count_answer_before":
    case "count_answer_after":
      return (a) => a === rule.answer;
    case "count_vowel_answers":
      return (a) => VOWELS.has(a);
    case "count_consonant_answers":
      return (a) => !VOWELS.has(a);
    default:
      return null;
  }
}

function countRange(rule: ValidationRule, n: number): [number, number] {
  switch (rule.type) {
    case "count_answer_before":
      return [0, rule.beforeIndex];
    case "count_answer_after":
      return [rule.afterIndex + 1, n];
    default:
      return [0, n];
  }
}

function countRuleLabel(rule: ValidationRule): string {
  switch (rule.type) {
    case "count_answer":
      return `questions with answer ${rule.answer}`;
    case "count_answer_before":
      return `questions before #${rule.beforeIndex + 1} with answer ${rule.answer}`;
    case "count_answer_after":
      return `questions after #${rule.afterIndex + 1} with answer ${rule.answer}`;
    case "count_vowel_answers":
      return "questions with a vowel answer";
    case "count_consonant_answers":
      return "questions with a consonant answer";
    default:
      return "matching questions";
  }
}

// ════════════════════════════════════════════════
// Priority 1: Explain definitive contradictions
// ════════════════════════════════════════════════

function findContradiction(
  puzzle: Puzzle,
  answers: (AnswerLetter | null)[],
  n: number,
): Hint | null {
  for (let qi = 0; qi < n; qi++) {
    if (answers[qi] == null) continue;
    const rule = puzzle.questions[qi].rule;
    const ov = optVal(puzzle, qi, answers[qi]!);
    const msg = explainContradiction(rule, qi, answers, ov, n);
    if (msg) {
      const answer = answers[qi]!;
      return hintSteps(qi, `Reconsider ${Q(qi)}'s answer (${answer}).`, msg, {
        type: "contradiction",
        questionIndex: qi,
      });
    }
  }
  return null;
}

function explainContradiction(
  rule: ValidationRule,
  qi: number,
  answers: (AnswerLetter | null)[],
  ov: string,
  n: number,
): string | null {
  // Counting rules
  const pred = countPred(rule);
  if (pred && rule.type !== "most_common_count") {
    const [from, to] = countRange(rule, n);
    const { count, remaining } = countMatching(answers, pred, from, to);
    const claimed = Number(ov);
    if (!Number.isNaN(claimed)) {
      if (count > claimed) {
        return `${Q(qi)}: you marked "${claimed} ${countRuleLabel(rule)}", but there are already ${count}.`;
      }
      if (count + remaining < claimed) {
        return `${Q(qi)}: you marked "${claimed} ${countRuleLabel(rule)}", but at most ${count + remaining} are possible.`;
      }
    }
  }

  // most_common_count
  if (rule.type === "most_common_count") {
    const counts = LETTERS.map((l) => countMatching(answers, (a) => a === l, 0, n).count);
    const max = Math.max(...counts);
    const claimed = Number(ov);
    if (!Number.isNaN(claimed) && max > claimed) {
      return `${Q(qi)}: you marked "most common answer appears ${claimed} times", but ${LETTERS[counts.indexOf(max)]} already appears ${max} times.`;
    }
  }

  // answer_of_question
  if (rule.type === "answer_of_question") {
    const target = answers[rule.questionIndex];
    if (target != null && target !== ov) {
      return `${Q(qi)}: you said ${Q(rule.questionIndex)}'s answer is ${ov}, but ${Q(rule.questionIndex)} is marked ${target}.`;
    }
  }

  // letter_distance
  if (rule.type === "letter_distance") {
    const other = answers[rule.otherQuestionIndex];
    if (other != null) {
      const dist = Math.abs(L2I[answers[qi]!] - L2I[other]);
      if (String(dist) !== ov) {
        return `${Q(qi)}: you marked distance = ${ov}, but ${answers[qi]!} and ${other} (${Q(rule.otherQuestionIndex)}) are ${dist} apart.`;
      }
    }
  }

  // Positional rules
  return explainPositionalContradiction(rule, qi, answers, ov, n);
}

function explainPositionalContradiction(
  rule: ValidationRule,
  qi: number,
  answers: (AnswerLetter | null)[],
  ov: string,
  n: number,
): string | null {
  if (rule.type === "closest_after") {
    return checkClosestAfter(qi, answers, ov, rule.afterIndex, rule.answer, n);
  }
  if (rule.type === "closest_before") {
    return checkClosestBefore(qi, answers, ov, rule.beforeIndex, rule.answer);
  }
  if (rule.type === "first_with_answer") {
    return checkFirstWith(qi, answers, ov, rule.answer, n);
  }
  if (rule.type === "last_with_answer") {
    return checkLastWith(qi, answers, ov, rule.answer, n);
  }
  if (rule.type === "unique_answer") {
    const count = countMatching(answers, (a) => a === answers[qi], 0, n).count;
    if (count > 1) {
      return `${Q(qi)}: you marked ${answers[qi]!} as unique, but ${count} questions have answer ${answers[qi]!}.`;
    }
  }
  if (rule.type === "same_answer_as") {
    const targetQ = Number(ov) - 1;
    if (
      targetQ >= 0 &&
      targetQ < n &&
      answers[targetQ] != null &&
      answers[targetQ] !== answers[qi]
    ) {
      return `${Q(qi)}: you said this has the same answer as ${Q(targetQ)}, but ${Q(targetQ)} is ${answers[targetQ]} and this is ${answers[qi]}.`;
    }
  }
  return null;
}

function checkClosestAfter(
  qi: number,
  answers: (AnswerLetter | null)[],
  ov: string,
  afterIndex: number,
  target: AnswerLetter,
  n: number,
): string | null {
  if (ov === "None") {
    for (let i = afterIndex + 1; i < n; i++) {
      if (answers[i] === target) {
        return `${Q(qi)}: you said no ${target} after #${afterIndex + 1}, but ${Q(i)} has answer ${target}.`;
      }
    }
    return null;
  }
  const claimedPos = Number(ov) - 1;
  if (claimedPos < 0 || claimedPos >= n) return null;

  if (answers[claimedPos] != null && answers[claimedPos] !== target) {
    return `${Q(qi)}: you said closest ${target} after #${afterIndex + 1} is ${Q(claimedPos)}, but ${Q(claimedPos)} is marked ${answers[claimedPos]}.`;
  }
  for (let i = afterIndex + 1; i < claimedPos; i++) {
    if (answers[i] === target) {
      return `${Q(qi)}: you said closest ${target} after #${afterIndex + 1} is ${Q(claimedPos)}, but ${Q(i)} also has ${target} and is closer.`;
    }
  }
  return null;
}

function checkClosestBefore(
  qi: number,
  answers: (AnswerLetter | null)[],
  ov: string,
  beforeIndex: number,
  target: AnswerLetter,
): string | null {
  if (ov === "None") {
    for (let i = beforeIndex - 1; i >= 0; i--) {
      if (answers[i] === target) {
        return `${Q(qi)}: you said no ${target} before #${beforeIndex + 1}, but ${Q(i)} has answer ${target}.`;
      }
    }
    return null;
  }
  const claimedPos = Number(ov) - 1;
  if (claimedPos < 0) return null;

  if (answers[claimedPos] != null && answers[claimedPos] !== target) {
    return `${Q(qi)}: you said closest ${target} before #${beforeIndex + 1} is ${Q(claimedPos)}, but ${Q(claimedPos)} is marked ${answers[claimedPos]}.`;
  }
  for (let i = beforeIndex - 1; i > claimedPos; i--) {
    if (answers[i] === target) {
      return `${Q(qi)}: you said closest ${target} before #${beforeIndex + 1} is ${Q(claimedPos)}, but ${Q(i)} also has ${target} and is closer.`;
    }
  }
  return null;
}

function checkFirstWith(
  qi: number,
  answers: (AnswerLetter | null)[],
  ov: string,
  target: AnswerLetter,
  n: number,
): string | null {
  if (ov === "None") {
    for (let i = 0; i < n; i++) {
      if (answers[i] === target) {
        return `${Q(qi)}: you said no question has answer ${target}, but ${Q(i)} does.`;
      }
    }
    return null;
  }
  const claimedPos = Number(ov) - 1;
  if (claimedPos < 0 || claimedPos >= n) return null;

  if (answers[claimedPos] != null && answers[claimedPos] !== target) {
    return `${Q(qi)}: you said the first ${target} is ${Q(claimedPos)}, but ${Q(claimedPos)} is marked ${answers[claimedPos]}.`;
  }
  for (let i = 0; i < claimedPos; i++) {
    if (answers[i] === target) {
      return `${Q(qi)}: you said the first ${target} is ${Q(claimedPos)}, but ${Q(i)} also has ${target} and comes earlier.`;
    }
  }
  return null;
}

function checkLastWith(
  qi: number,
  answers: (AnswerLetter | null)[],
  ov: string,
  target: AnswerLetter,
  n: number,
): string | null {
  if (ov === "None") {
    for (let i = 0; i < n; i++) {
      if (answers[i] === target) {
        return `${Q(qi)}: you said no question has answer ${target}, but ${Q(i)} does.`;
      }
    }
    return null;
  }
  const claimedPos = Number(ov) - 1;
  if (claimedPos < 0 || claimedPos >= n) return null;

  if (answers[claimedPos] != null && answers[claimedPos] !== target) {
    return `${Q(qi)}: you said the last ${target} is ${Q(claimedPos)}, but ${Q(claimedPos)} is marked ${answers[claimedPos]}.`;
  }
  for (let i = n - 1; i > claimedPos; i--) {
    if (answers[i] === target) {
      return `${Q(qi)}: you said the last ${target} is ${Q(claimedPos)}, but ${Q(i)} also has ${target} and comes later.`;
    }
  }
  return null;
}

// ════════════════════════════════════════════════
// Priority 2: Find forced values
// ════════════════════════════════════════════════

function findForced(
  puzzle: Puzzle,
  answers: (AnswerLetter | null)[],
  markSets: Marks[],
  n: number,
): Hint | null {
  for (let qi = 0; qi < n; qi++) {
    if (answers[qi] != null) continue;

    // Forced by elimination: only 1 option not marked incorrect
    const remaining: number[] = [];
    for (let oi = 0; oi < 5; oi++) {
      if (markSets[qi][oi] !== "incorrect") remaining.push(oi);
    }
    if (remaining.length === 1) {
      const letter = LETTERS[remaining[0]];
      return hintSteps(
        qi,
        `${Q(qi)} can be determined now.`,
        `${Q(qi)} has only one option left — it must be ${letter}.`,
        { type: "force", questionIndex: qi, letter },
      );
    }

    // Forced by answer_of_question (forward: target is known → this is forced)
    const rule = puzzle.questions[qi].rule;
    if (rule.type === "answer_of_question" && answers[rule.questionIndex] != null) {
      const target = answers[rule.questionIndex]!;
      const oi = puzzle.questions[qi].options.findIndex((o) => o.label === target);
      if (oi >= 0) {
        const letter = LETTERS[oi];
        return hintSteps(
          qi,
          `${Q(qi)} can be determined now.`,
          `${Q(qi)} asks for ${Q(rule.questionIndex)}'s answer — ${Q(rule.questionIndex)} is ${target}, so ${Q(qi)} must be ${letter}.`,
          { type: "force", questionIndex: qi, letter },
        );
      }
    }

    // Forced by reverse answer_of_question (some other answered question references this one)
    for (let other = 0; other < n; other++) {
      const otherAns = answers[other];
      if (otherAns == null) continue;
      const otherRule = puzzle.questions[other].rule;
      if (otherRule.type === "answer_of_question" && otherRule.questionIndex === qi) {
        const implied = optVal(puzzle, other, otherAns);
        const idx = L2I[implied] ?? -1;
        if (idx >= 0) {
          return hintSteps(
            qi,
            `${Q(qi)} can be determined now.`,
            `${Q(other)} is ${otherAns}, which says ${Q(qi)}'s answer is ${implied}.`,
            { type: "force", questionIndex: qi, letter: LETTERS[idx] },
          );
        }
      }
      if (otherRule.type === "same_answer_as") {
        const targetQ = Number(optVal(puzzle, other, otherAns)) - 1;
        if (targetQ === qi) {
          return hintSteps(
            qi,
            `${Q(qi)} can be determined now.`,
            `${Q(other)} is ${otherAns}, meaning it shares an answer with ${Q(qi)} — so ${Q(qi)} must be ${otherAns}.`,
            { type: "force", questionIndex: qi, letter: otherAns },
          );
        }
      }
    }

    // Forced by letter_distance when other question is answered
    if (rule.type === "letter_distance" && answers[rule.otherQuestionIndex] != null) {
      const otherAnswer = answers[rule.otherQuestionIndex]!;
      const otherIdx = L2I[otherAnswer];
      const validLetters: AnswerLetter[] = [];
      for (let oi = 0; oi < 5; oi++) {
        if (markSets[qi][oi] === "incorrect") continue;
        const dist = Math.abs(oi - otherIdx);
        const ov2 = puzzle.questions[qi].options[oi].label;
        if (String(dist) === ov2) validLetters.push(LETTERS[oi]);
      }
      if (validLetters.length === 1) {
        return hintSteps(
          qi,
          `${Q(qi)} can be determined now.`,
          `${Q(qi)}: ${Q(rule.otherQuestionIndex)} is ${otherAnswer}, and only option ${validLetters[0]} gives the right distance.`,
          { type: "force", questionIndex: qi, letter: validLetters[0] },
        );
      }
    }

    // Forced counting: all questions in range are answered
    const pred = countPred(rule);
    if (pred) {
      const [from, to] = countRange(rule, n);
      const { count, remaining: rem } = countMatching(answers, pred, from, to);
      if (rem === 0) {
        for (let oi = 0; oi < 5; oi++) {
          if (markSets[qi][oi] === "incorrect") continue;
          if (puzzle.questions[qi].options[oi].label === String(count)) {
            return hintSteps(
              qi,
              `${Q(qi)} can be determined now.`,
              `${Q(qi)}: all relevant questions are answered — there are ${count} ${countRuleLabel(rule)}, so it must be ${LETTERS[oi]}.`,
              { type: "force", questionIndex: qi, letter: LETTERS[oi] },
            );
          }
        }
      }
    }
  }
  return null;
}

// ════════════════════════════════════════════════
// Priority 3: Find eliminable options
// ════════════════════════════════════════════════

function findEliminable(
  puzzle: Puzzle,
  answers: (AnswerLetter | null)[],
  markSets: Marks[],
  n: number,
): Hint | null {
  for (let qi = 0; qi < n; qi++) {
    if (answers[qi] != null) continue;
    const rule = puzzle.questions[qi].rule;

    for (let oi = 0; oi < 5; oi++) {
      if (markSets[qi][oi] !== "unmarked") continue;
      const ov = puzzle.questions[qi].options[oi].label;
      const msg = canEliminate(rule, qi, oi, ov, answers, n);
      if (msg) {
        return hintSteps(
          qi,
          `Consider ${Q(qi)}, option ${LETTERS[oi]}.`,
          `${Q(qi)}, option ${LETTERS[oi]}: ${msg}`,
          { type: "eliminate", questionIndex: qi, optionIndex: oi },
          oi,
        );
      }
    }
  }
  return null;
}

function canEliminate(
  rule: ValidationRule,
  qi: number,
  oi: number,
  ov: string,
  answers: (AnswerLetter | null)[],
  n: number,
): string | null {
  // Count bounds
  const pred = countPred(rule);
  if (pred && rule.type !== "most_common_count") {
    const [from, to] = countRange(rule, n);
    const { count, remaining } = countMatching(answers, pred, from, to);
    const claimed = Number(ov);
    if (!Number.isNaN(claimed)) {
      if (count > claimed) {
        return `says ${claimed}, but already ${count} ${countRuleLabel(rule)}.`;
      }
      if (count + remaining < claimed) {
        return `says ${claimed}, but at most ${count + remaining} ${countRuleLabel(rule)} are possible.`;
      }
    }
  }

  // answer_of_question: target already answered with different letter
  if (rule.type === "answer_of_question") {
    const target = answers[rule.questionIndex];
    if (target != null && target !== ov) {
      return `says ${Q(rule.questionIndex)} is ${ov}, but it's marked ${target}.`;
    }
  }

  // letter_distance: other question answered, distance doesn't match
  if (rule.type === "letter_distance") {
    const other = answers[rule.otherQuestionIndex];
    if (other != null) {
      const dist = Math.abs(oi - L2I[other]);
      if (String(dist) !== ov) {
        return `says distance is ${ov}, but ${LETTERS[oi]} and ${other} are ${dist} apart.`;
      }
    }
  }

  // Positional: claimed position has wrong answer
  if (isPositionalRule(rule.type) && ov !== "None") {
    const claimedPos = Number(ov) - 1;
    if (claimedPos >= 0 && claimedPos < n) {
      const posAnswer = answers[claimedPos];
      if (posAnswer != null) {
        const msg = checkPositionalElim(rule, qi, oi, ov, claimedPos, posAnswer, answers, n);
        if (msg) return msg;
      }
    }
  }

  // Positional "None" but a match exists
  if (isPositionalRule(rule.type) && ov === "None") {
    const msg = checkNoneElim(rule, qi, answers, n);
    if (msg) return msg;
  }

  // Closer match exists for closest_after/before, first/last
  if (ov !== "None") {
    const msg = checkCloserExists(rule, qi, ov, answers, n);
    if (msg) return msg;
  }

  return null;
}

function isPositionalRule(type: string): boolean {
  return [
    "closest_after",
    "closest_before",
    "first_with_answer",
    "last_with_answer",
    "only_same_answer",
    "only_odd_with_answer",
    "same_answer_as",
  ].includes(type);
}

function checkPositionalElim(
  rule: ValidationRule,
  _qi: number,
  _oi: number,
  _ov: string,
  claimedPos: number,
  posAnswer: AnswerLetter,
  _answers: (AnswerLetter | null)[],
  _n: number,
): string | null {
  if (
    rule.type === "closest_after" ||
    rule.type === "first_with_answer" ||
    rule.type === "last_with_answer"
  ) {
    if (posAnswer !== rule.answer) {
      return `says ${Q(claimedPos)} has answer ${rule.answer}, but it's marked ${posAnswer}.`;
    }
  }
  if (rule.type === "closest_before") {
    if (posAnswer !== rule.answer) {
      return `says ${Q(claimedPos)} has answer ${rule.answer}, but it's marked ${posAnswer}.`;
    }
  }
  if (rule.type === "same_answer_as") {
    return null; // handled separately since it depends on this question's answer
  }
  return null;
}

function checkNoneElim(
  rule: ValidationRule,
  _qi: number,
  answers: (AnswerLetter | null)[],
  n: number,
): string | null {
  if (rule.type === "closest_after") {
    for (let i = rule.afterIndex + 1; i < n; i++) {
      if (answers[i] === rule.answer) {
        return `says "None", but ${Q(i)} has answer ${rule.answer}.`;
      }
    }
  }
  if (rule.type === "closest_before") {
    for (let i = rule.beforeIndex - 1; i >= 0; i--) {
      if (answers[i] === rule.answer) {
        return `says "None", but ${Q(i)} has answer ${rule.answer}.`;
      }
    }
  }
  if (rule.type === "first_with_answer" || rule.type === "last_with_answer") {
    for (let i = 0; i < n; i++) {
      if (answers[i] === rule.answer) {
        return `says "None", but ${Q(i)} has answer ${rule.answer}.`;
      }
    }
  }
  return null;
}

function checkCloserExists(
  rule: ValidationRule,
  _qi: number,
  ov: string,
  answers: (AnswerLetter | null)[],
  n: number,
): string | null {
  const claimedPos = Number(ov) - 1;
  if (Number.isNaN(claimedPos) || claimedPos < 0) return null;

  if (rule.type === "closest_after") {
    for (let i = rule.afterIndex + 1; i < claimedPos; i++) {
      if (answers[i] === rule.answer) {
        return `says closest ${rule.answer} after #${rule.afterIndex + 1} is ${Q(claimedPos)}, but ${Q(i)} has ${rule.answer} and is closer.`;
      }
    }
  }
  if (rule.type === "closest_before") {
    for (let i = rule.beforeIndex - 1; i > claimedPos; i--) {
      if (answers[i] === rule.answer) {
        return `says closest ${rule.answer} before #${rule.beforeIndex + 1} is ${Q(claimedPos)}, but ${Q(i)} has ${rule.answer} and is closer.`;
      }
    }
  }
  if (rule.type === "first_with_answer") {
    for (let i = 0; i < claimedPos; i++) {
      if (answers[i] === rule.answer) {
        return `says first ${rule.answer} is ${Q(claimedPos)}, but ${Q(i)} has ${rule.answer} and comes earlier.`;
      }
    }
  }
  if (rule.type === "last_with_answer") {
    for (let i = n - 1; i > claimedPos; i--) {
      if (answers[i] === rule.answer) {
        return `says last ${rule.answer} is ${Q(claimedPos)}, but ${Q(i)} has ${rule.answer} and comes later.`;
      }
    }
  }
  return null;
}

// ════════════════════════════════════════════════
// Fast action-only deduction (no message strings)
// Used by traceAssumption and checkSolvable
// ════════════════════════════════════════════════

export function findActionFast(
  puzzle: Puzzle,
  answers: (AnswerLetter | null)[],
  markSets: Marks[],
  n: number,
): Action {
  return findActionFp(getFlatPuzzle(puzzle), answers, markSets, n);
}

function findActionFp(
  fp: FlatPuzzle,
  answers: (AnswerLetter | null)[],
  markSets: Marks[],
  n: number,
): Action {
  // Contradictions
  for (let qi = 0; qi < n; qi++) {
    const a = answers[qi];
    if (a == null) continue;
    const r = fp.rules[qi];
    const ai = letterIdx(a);
    const ov = fp.optionLabels[qi][ai];
    const on = fp.optionNums[qi][ai];

    if (
      r.t === RT_COUNT_ANSWER ||
      r.t === RT_COUNT_ANSWER_BEFORE ||
      r.t === RT_COUNT_ANSWER_AFTER ||
      r.t === RT_COUNT_VOWEL ||
      r.t === RT_COUNT_CONSONANT
    ) {
      if (!Number.isNaN(on)) {
        const pred = countPred2(r);
        if (pred) {
          const [from, to] = countRange2(r, n);
          const cr = countMatching(answers, pred, from, to);
          if (cr.count > on || cr.count + cr.remaining < on) {
            return { type: "contradiction", questionIndex: qi };
          }
        }
      }
    }
    if (r.t === RT_ANSWER_OF) {
      const target = answers[r.questionIndex];
      if (target != null && target !== ov) return { type: "contradiction", questionIndex: qi };
    }
    if (r.t === RT_LETTER_DIST) {
      const other = answers[r.otherQuestionIndex];
      if (other != null) {
        const dist = Math.abs(ai - letterIdx(other));
        if (dist !== on) return { type: "contradiction", questionIndex: qi };
      }
    }
    if (r.t === RT_UNIQUE) {
      if (countMatchingSimple(answers, a, 0, n) > 1)
        return { type: "contradiction", questionIndex: qi };
    }
    if (r.t === RT_CLOSEST_AFTER || r.t === RT_FIRST_WITH) {
      const afterIdx = r.t === RT_CLOSEST_AFTER ? r.afterIndex : -1;
      if (on !== NONE) {
        const pos = on - 1;
        if (pos >= 0 && pos < n && answers[pos] != null && answers[pos] !== r.answer) {
          return { type: "contradiction", questionIndex: qi };
        }
        for (let j = afterIdx + 1; j < pos; j++) {
          if (answers[j] === r.answer) return { type: "contradiction", questionIndex: qi };
        }
      } else {
        for (let j = afterIdx + 1; j < n; j++) {
          if (answers[j] === r.answer) return { type: "contradiction", questionIndex: qi };
        }
      }
    }
    if (r.t === RT_CLOSEST_BEFORE || r.t === RT_LAST_WITH) {
      const beforeIdx = r.t === RT_CLOSEST_BEFORE ? r.beforeIndex : n;
      if (on !== NONE) {
        const pos = on - 1;
        if (pos >= 0 && pos < n && answers[pos] != null && answers[pos] !== r.answer) {
          return { type: "contradiction", questionIndex: qi };
        }
        for (let j = beforeIdx - 1; j > pos; j--) {
          if (answers[j] === r.answer) return { type: "contradiction", questionIndex: qi };
        }
      } else {
        for (let j = 0; j < beforeIdx; j++) {
          if (answers[j] === r.answer) return { type: "contradiction", questionIndex: qi };
        }
      }
    }
    if (r.t === RT_SAME_AS) {
      const tq = on - 1;
      if (tq >= 0 && tq < n && answers[tq] != null && answers[tq] !== a) {
        return { type: "contradiction", questionIndex: qi };
      }
    }
  }

  // Forced values
  for (let qi = 0; qi < n; qi++) {
    if (answers[qi] != null) continue;
    const r = fp.rules[qi];

    const remaining: number[] = [];
    for (let oi = 0; oi < 5; oi++) {
      if (markSets[qi][oi] !== "incorrect") remaining.push(oi);
    }
    if (remaining.length === 1) {
      return { type: "force", questionIndex: qi, letter: LETTERS[remaining[0]] };
    }

    if (r.t === RT_ANSWER_OF && answers[r.questionIndex] != null) {
      const target = answers[r.questionIndex]!;
      for (let oi = 0; oi < 5; oi++) {
        if (fp.optionLabels[qi][oi] === target) {
          return { type: "force", questionIndex: qi, letter: LETTERS[oi] };
        }
      }
    }

    // Reverse answer_of_question
    for (let other = 0; other < n; other++) {
      const otherAns = answers[other];
      if (otherAns == null) continue;
      const otherR = fp.rules[other];
      if (otherR.t === RT_ANSWER_OF && otherR.questionIndex === qi) {
        const implied = fp.optionLabels[other][letterIdx(otherAns)];
        const idx = L2I[implied] ?? -1;
        if (idx >= 0) return { type: "force", questionIndex: qi, letter: LETTERS[idx] };
      }
    }

    const pred = countPred2(r);
    if (pred) {
      const [from, to] = countRange2(r, n);
      const cr = countMatching(answers, pred, from, to);
      if (cr.remaining === 0) {
        for (let oi = 0; oi < 5; oi++) {
          if (markSets[qi][oi] === "incorrect") continue;
          if (fp.optionNums[qi][oi] === cr.count) {
            return { type: "force", questionIndex: qi, letter: LETTERS[oi] };
          }
        }
      }
    }
  }

  // Eliminations
  for (let qi = 0; qi < n; qi++) {
    if (answers[qi] != null) continue;
    const r = fp.rules[qi];

    for (let oi = 0; oi < 5; oi++) {
      if (markSets[qi][oi] !== "unmarked") continue;
      const ov = fp.optionLabels[qi][oi];
      const on = fp.optionNums[qi][oi];

      const pred = countPred2(r);
      if (pred && r.t !== RT_MOST_COMMON_COUNT) {
        const [from, to] = countRange2(r, n);
        const cr = countMatching(answers, pred, from, to);
        if (!Number.isNaN(on) && (cr.count > on || cr.count + cr.remaining < on)) {
          return { type: "eliminate", questionIndex: qi, optionIndex: oi };
        }
      }

      if (r.t === RT_ANSWER_OF) {
        const target = answers[r.questionIndex];
        if (target != null && target !== ov) {
          return { type: "eliminate", questionIndex: qi, optionIndex: oi };
        }
      }

      if (r.t === RT_LETTER_DIST) {
        const other = answers[r.otherQuestionIndex];
        if (other != null && Math.abs(oi - letterIdx(other)) !== on) {
          return { type: "eliminate", questionIndex: qi, optionIndex: oi };
        }
      }

      if ((r.t === RT_CLOSEST_AFTER || r.t === RT_FIRST_WITH) && on !== NONE) {
        const pos = on - 1;
        const afterIdx = r.t === RT_CLOSEST_AFTER ? r.afterIndex : -1;
        if (pos >= 0 && pos < n && answers[pos] != null && answers[pos] !== r.answer) {
          return { type: "eliminate", questionIndex: qi, optionIndex: oi };
        }
        for (let j = afterIdx + 1; j < pos; j++) {
          if (answers[j] === r.answer)
            return { type: "eliminate", questionIndex: qi, optionIndex: oi };
        }
      }
      if ((r.t === RT_CLOSEST_BEFORE || r.t === RT_LAST_WITH) && on !== NONE) {
        const pos = on - 1;
        const beforeIdx = r.t === RT_CLOSEST_BEFORE ? r.beforeIndex : n;
        if (pos >= 0 && pos < n && answers[pos] != null && answers[pos] !== r.answer) {
          return { type: "eliminate", questionIndex: qi, optionIndex: oi };
        }
        for (let j = beforeIdx - 1; j > pos; j--) {
          if (answers[j] === r.answer)
            return { type: "eliminate", questionIndex: qi, optionIndex: oi };
        }
      }
      if (on === NONE && (r.t === RT_CLOSEST_AFTER || r.t === RT_FIRST_WITH)) {
        const afterIdx = r.t === RT_CLOSEST_AFTER ? r.afterIndex : -1;
        for (let j = afterIdx + 1; j < n; j++) {
          if (answers[j] === r.answer)
            return { type: "eliminate", questionIndex: qi, optionIndex: oi };
        }
      }
      if (on === NONE && (r.t === RT_CLOSEST_BEFORE || r.t === RT_LAST_WITH)) {
        const beforeIdx = r.t === RT_CLOSEST_BEFORE ? r.beforeIndex : n;
        for (let j = beforeIdx - 1; j >= 0; j--) {
          if (answers[j] === r.answer)
            return { type: "eliminate", questionIndex: qi, optionIndex: oi };
        }
      }
    }
  }

  return null;
}

// FlatRule-aware versions of countPred/countRange
function countPred2(r: {
  t: number;
  answer: string | null;
}): ((a: AnswerLetter) => boolean) | null {
  switch (r.t) {
    case RT_COUNT_ANSWER:
    case RT_COUNT_ANSWER_BEFORE:
    case RT_COUNT_ANSWER_AFTER:
      return (a) => a === r.answer;
    case RT_COUNT_VOWEL:
      return (a) => a === "A" || a === "E";
    case RT_COUNT_CONSONANT:
      return (a) => a !== "A" && a !== "E";
    default:
      return null;
  }
}

function countRange2(
  r: { t: number; beforeIndex: number; afterIndex: number },
  n: number,
): [number, number] {
  if (r.t === RT_COUNT_ANSWER_BEFORE) return [0, r.beforeIndex];
  if (r.t === RT_COUNT_ANSWER_AFTER) return [r.afterIndex + 1, n];
  return [0, n];
}

function countMatchingSimple(
  answers: (AnswerLetter | null)[],
  target: AnswerLetter,
  from: number,
  to: number,
): number {
  let c = 0;
  for (let i = from; i < to; i++) {
    if (answers[i] === target) c++;
  }
  return c;
}

// ════════════════════════════════════════════════
// Priority 4: Look-ahead — assume an option, apply
// forced deductions, check for contradiction
// ════════════════════════════════════════════════

function findLookahead(
  puzzle: Puzzle,
  answers: (AnswerLetter | null)[],
  markSets: Marks[],
  n: number,
): Hint | null {
  for (let qi = 0; qi < n; qi++) {
    if (answers[qi] != null) continue;

    for (let oi = 0; oi < 5; oi++) {
      if (markSets[qi][oi] === "incorrect") continue;

      const result = traceAssumption(puzzle, answers, markSets, n, qi, oi);
      if (result) {
        const letter = LETTERS[oi];
        const involved = result.involvedQuestions;
        const otherQs = involved.filter((q) => q !== qi);
        const step1 = `Try looking at ${Q(qi)}.`;
        const allQs = [qi, ...otherQs].map(Q);
        const step2 =
          otherQs.length > 0
            ? `Try looking at ${formatList(allQs)}.`
            : `Consider ${Q(qi)}, option ${letter}.`;
        const step3 = `What if ${Q(qi)} is ${letter}?`;
        const step4 = `${Q(qi)} can't be ${letter}: ${result.explanation}`;
        return {
          steps: [step1, step2, step3, step4],
          action: { type: "eliminate", questionIndex: qi, optionIndex: oi },
        };
      }
    }
  }
  return null;
}

// Scratch arrays reused across traceAssumption calls (not safe for recursive/concurrent use)
let scratchAns: (AnswerLetter | null)[] = [];
let scratchMarks: Marks[] = [];

interface TraceResult {
  explanation: string;
  involvedQuestions: number[];
}

function traceAssumption(
  puzzle: Puzzle,
  origAnswers: (AnswerLetter | null)[],
  origMarks: Marks[],
  n: number,
  assumeQi: number,
  assumeOi: number,
): TraceResult | null {
  if (scratchAns.length !== n) {
    scratchAns = new Array(n);
    scratchMarks = Array.from(
      { length: n },
      () => ["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"] as Marks,
    );
  }
  for (let i = 0; i < n; i++) {
    scratchAns[i] = origAnswers[i];
    for (let j = 0; j < 5; j++) scratchMarks[i][j] = origMarks[i][j];
  }
  const ans = scratchAns;
  const marks = scratchMarks;

  ans[assumeQi] = LETTERS[assumeOi];
  for (let j = 0; j < 5; j++) marks[assumeQi][j] = "unmarked";
  marks[assumeQi][assumeOi] = "correct";

  const chainSteps: string[] = [];
  const involved = new Set<number>([assumeQi]);

  for (let iter = 0; iter < n * 5; iter++) {
    const action = findActionFast(puzzle, ans, marks, n);
    if (!action) break;

    if (action.type === "contradiction") {
      involved.add(action.questionIndex);
      const userHint = findContradiction(puzzle, ans, n);
      const detail = userHint
        ? userHint.steps[userHint.steps.length - 1].replace(/^Q\d+: /, "")
        : "leads to a contradiction.";
      const chain = chainSteps.length > 0 ? chainSteps.join(", then ") + " — but " : "";
      return {
        explanation: chain + detail,
        involvedQuestions: [...involved],
      };
    }

    if (action.type === "force") {
      const oi = L2I[action.letter];
      for (let j = 0; j < 5; j++) marks[action.questionIndex][j] = "incorrect";
      marks[action.questionIndex][oi] = "correct";
      ans[action.questionIndex] = action.letter;
      involved.add(action.questionIndex);
      chainSteps.push(`${Q(action.questionIndex)} must be ${action.letter}`);
    } else if (action.type === "eliminate") {
      marks[action.questionIndex][action.optionIndex] = "incorrect";
    }
  }

  return null;
}
