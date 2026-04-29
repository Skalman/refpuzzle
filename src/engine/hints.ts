import type { AnswerLetter, Puzzle, FlatPuzzle, ValidationRule, Marks } from "./types.ts";
import { evaluate as evaluateRule } from "./evaluators.ts";
import { renderOptionLabel } from "./render.ts";
import {
  LETTERS,
  VOWELS,
  L2I,
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
  RT_ONLY_ODD,
  RT_CONSEC_IDENT,
  RT_PREV_SAME,
  RT_NEXT_SAME,
  RT_ONLY_SAME,
} from "./types.ts";

type Action =
  | { type: "force"; questionIndex: number; letter: AnswerLetter }
  | { type: "eliminate"; questionIndex: number; optionIndex: number }
  | { type: "contradiction"; questionIndex: number }
  | null;

interface Hint {
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
    findError(puzzle, answers, n) ??
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

function findError(
  puzzle: Puzzle,
  answers: (AnswerLetter | null)[],
  n: number,
): Hint | null {
  const hasContradiction = answers.some((a, qi) => {
    if (a == null) return false;
    const rule = puzzle.questions[qi].rule;
    const ov = optDisplay(puzzle, qi, a);
    const v = optValue(puzzle, qi, a);
    return explainContradiction(rule, qi, answers, ov, v, n) != null;
  });
  if (!hasContradiction) return null;

  const solution = autoSolve(puzzle, n);
  if (!solution) {
    return {
      steps: ["It looks like you've made an error somewhere."],
      action: { type: "contradiction", questionIndex: 0 },
    };
  }

  for (let qi = 0; qi < n; qi++) {
    if (answers[qi] != null && answers[qi] !== solution[qi]) {
      return {
        steps: [
          "It looks like you've made an error somewhere.",
          `Check ${Q(qi)}.`,
          `${Q(qi)} should not be ${answers[qi]!}.`,
        ],
        action: { type: "contradiction", questionIndex: qi },
      };
    }
  }

  return null;
}

function autoSolve(puzzle: Puzzle, n: number): AnswerLetter[] | null {
  const marks: Marks[] = Array.from(
    { length: n },
    () => ["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"] as Marks,
  );
  const answers: (AnswerLetter | null)[] = new Array(n).fill(null);

  for (let iter = 0; iter < n * 15; iter++) {
    if (answers.every((a) => a != null)) return answers;
    const action = findActionFast(puzzle, answers, marks, n);
    if (action) {
      if (action.type === "contradiction") return null;
      applyAutoAction(action, answers, marks);
      continue;
    }
    const hint = findLookahead(puzzle, answers, marks, n);
    if (!hint?.action) return null;
    applyAutoAction(hint.action, answers, marks);
  }
  return answers.every((a) => a != null) ? (answers) : null;
}

function applyAutoAction(
  action: NonNullable<Hint["action"]>,
  answers: (AnswerLetter | null)[],
  marks: Marks[],
): void {
  if (action.type === "force") {
    answers[action.questionIndex] = action.letter;
    for (let j = 0; j < 5; j++) marks[action.questionIndex][j] = "incorrect";
    marks[action.questionIndex][L2I[action.letter]] = "correct";
  } else if (action.type === "eliminate") {
    marks[action.questionIndex][action.optionIndex] = "incorrect";
  }
}

function findSimpleDeduction(
  puzzle: Puzzle,
  answers: (AnswerLetter | null)[],
  markSets: Marks[],
  n: number,
): Hint | null {
  const action = findActionFast(puzzle, answers, markSets, n);
  if (!action || action.type === "contradiction") return null;
  if (action.type === "force") {
    return explainForce(puzzle, answers, markSets, n, action);
  }
  return explainElimination(puzzle, answers, markSets, n, action);
}

function explainForce(
  puzzle: Puzzle,
  answers: (AnswerLetter | null)[],
  markSets: Marks[],
  n: number,
  action: { type: "force"; questionIndex: number; letter: AnswerLetter },
): Hint {
  const qi = action.questionIndex;
  const letter = action.letter;
  // Try the user-facing findForced for a nice explanation
  const userHint = findForced(puzzle, answers, markSets, n);
  if (userHint?.action?.type === "force" && userHint.action.questionIndex === qi) {
    return userHint;
  }
  return hintSteps(qi, `Look at ${Q(qi)}.`, `${Q(qi)} must be ${letter}.`, action);
}

function explainElimination(
  puzzle: Puzzle,
  answers: (AnswerLetter | null)[],
  markSets: Marks[],
  n: number,
  action: { type: "eliminate"; questionIndex: number; optionIndex: number },
): Hint {
  const qi = action.questionIndex;
  const oi = action.optionIndex;
  // Try the user-facing findEliminable for a nice explanation
  const userHint = findEliminable(puzzle, answers, markSets, n);
  if (
    userHint?.action?.type === "eliminate" &&
    userHint.action.questionIndex === qi &&
    userHint.action.optionIndex === oi
  ) {
    return userHint;
  }
  const letter = LETTERS[oi];
  return hintSteps(
    qi,
    `Consider ${Q(qi)}, option ${letter}.`,
    `${Q(qi)} can't be ${letter}.`,
    action,
    oi,
  );
}

function deriveAnswers(markSets: Marks[]): (AnswerLetter | null)[] {
  return markSets.map((m) => {
    const idx = m.indexOf("correct");
    return idx >= 0 ? LETTERS[idx] : null;
  });
}

// Get the semantic value for the selected answer
function optValue(puzzle: Puzzle, qi: number, answer: AnswerLetter): number | null {
  return puzzle.questions[qi].options[L2I[answer]].value;
}

// Get the display string for the selected answer
function optDisplay(puzzle: Puzzle, qi: number, answer: AnswerLetter): string {
  const rule = puzzle.questions[qi].rule;
  const v = puzzle.questions[qi].options[L2I[answer]].value;
  return renderOptionLabel(rule, v, qi);
}

function Q(i: number): string {
  return `Q${i + 1}`;
}

function formatList(items: string[]): string {
  if (items.length <= 1) return items[0] ?? "";
  if (items.length === 2) return `${items[0]} and ${items[1]}`;
  return items.slice(0, -1).join(", ") + ", and " + items[items.length - 1];
}

// ── State helpers (same API as Rust for easy porting) ──

function isElim(marks: Marks[], qi: number, oi: number): boolean {
  return marks[qi][oi] === "incorrect";
}

function remCount(marks: Marks[], qi: number): number {
  let c = 0;
  for (let i = 0; i < 5; i++) if (marks[qi][i] !== "incorrect") c++;
  return c;
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

function countRuleLabel(rule: ValidationRule, count?: number): string {
  const q = count === 1 ? "question" : "questions";
  switch (rule.type) {
    case "count_answer":
      return `${q} with answer ${rule.answer}`;
    case "count_answer_before":
      return `${q} before #${rule.beforeIndex + 1} with answer ${rule.answer}`;
    case "count_answer_after":
      return `${q} after #${rule.afterIndex + 1} with answer ${rule.answer}`;
    case "count_vowel_answers":
      return `${q} with a vowel answer`;
    case "count_consonant_answers":
      return `${q} with a consonant answer`;
    default:
      return `matching ${q}`;
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
    const ov = optDisplay(puzzle, qi, answers[qi]!);
    const v = optValue(puzzle, qi, answers[qi]!);
    const msg = explainContradiction(rule, qi, answers, ov, v, n);
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
  v: number | null,
  n: number,
): string | null {
  // Counting rules
  const pred = countPred(rule);
  if (pred && rule.type !== "most_common_count") {
    const [from, to] = countRange(rule, n);
    const { count, remaining } = countMatching(answers, pred, from, to);
    if (v != null) {
      if (count > v) {
        return `${Q(qi)}: "${v} ${countRuleLabel(rule, v)}", but there are already ${count}.`;
      }
      if (count + remaining < v) {
        return `${Q(qi)}: "${v} ${countRuleLabel(rule, v)}", but at most ${count + remaining} are possible.`;
      }
    }
  }

  // most_common_count
  if (rule.type === "most_common_count") {
    const counts = LETTERS.map((l) => countMatching(answers, (a) => a === l, 0, n).count);
    const max = Math.max(...counts);
    if (v != null && max > v) {
      return `${Q(qi)}: "most common answer appears ${v} times", but ${LETTERS[counts.indexOf(max)]} already appears ${max} times.`;
    }
  }

  // answer_of_question
  if (rule.type === "answer_of_question") {
    const target = answers[rule.questionIndex];
    if (target != null && v != null && letterIdx(target) !== v) {
      return `${Q(qi)}: says ${Q(rule.questionIndex)}'s answer is ${LETTERS[v]}, but ${Q(rule.questionIndex)} is marked ${target}.`;
    }
  }

  // letter_distance
  if (rule.type === "letter_distance") {
    const other = answers[rule.questionIndex];
    if (other != null) {
      const dist = Math.abs(L2I[answers[qi]!] - L2I[other]);
      if (v != null && dist !== v) {
        return `${Q(qi)}: distance = ${ov}, but ${answers[qi]!} and ${other} (${Q(rule.questionIndex)}) are ${dist} apart.`;
      }
    }
  }

  // Positional rules
  return explainPositionalContradiction(rule, qi, answers, v, n);
}

function explainPositionalContradiction(
  rule: ValidationRule,
  qi: number,
  answers: (AnswerLetter | null)[],
  v: number | null,
  n: number,
): string | null {
  if (rule.type === "closest_after") {
    return checkClosestAfter(qi, answers, v, rule.afterIndex, rule.answer, n);
  }
  if (rule.type === "closest_before") {
    return checkClosestBefore(qi, answers, v, rule.beforeIndex, rule.answer);
  }
  if (rule.type === "first_with_answer") {
    return checkFirstWith(qi, answers, v, rule.answer, n);
  }
  if (rule.type === "last_with_answer") {
    return checkLastWith(qi, answers, v, rule.answer, n);
  }
  if (rule.type === "unique_answer") {
    const a = answers[qi]!;
    let count = 0;
    for (let i = 0; i < n; i++) if (answers[i] === a) count++;
    if (count > 1) {
      return `${Q(qi)}: ${a} is marked unique, but ${count} questions have answer ${a}.`;
    }
  }
  if (rule.type === "same_answer_as") {
    if (v != null && v >= 0 && v < n && answers[v] != null && answers[v] !== answers[qi]) {
      return `${Q(qi)}: says this has the same answer as ${Q(v)}, but ${Q(v)} is ${answers[v]} and this is ${answers[qi]}.`;
    }
  }
  return null;
}

function checkClosestAfter(
  qi: number,
  answers: (AnswerLetter | null)[],
  v: number | null,
  afterIndex: number,
  target: AnswerLetter,
  n: number,
): string | null {
  if (v == null) {
    for (let i = afterIndex + 1; i < n; i++) {
      if (answers[i] === target) {
        return `${Q(qi)}: says no ${target} after #${afterIndex + 1}, but ${Q(i)} has answer ${target}.`;
      }
    }
    return null;
  }
  if (v < 0 || v >= n) return null;

  if (answers[v] != null && answers[v] !== target) {
    return `${Q(qi)}: says closest ${target} after #${afterIndex + 1} is ${Q(v)}, but ${Q(v)} is marked ${answers[v]}.`;
  }
  for (let i = afterIndex + 1; i < v; i++) {
    if (answers[i] === target) {
      return `${Q(qi)}: says closest ${target} after #${afterIndex + 1} is ${Q(v)}, but ${Q(i)} also has ${target} and is closer.`;
    }
  }
  return null;
}

function checkClosestBefore(
  qi: number,
  answers: (AnswerLetter | null)[],
  v: number | null,
  beforeIndex: number,
  target: AnswerLetter,
): string | null {
  if (v == null) {
    for (let i = beforeIndex - 1; i >= 0; i--) {
      if (answers[i] === target) {
        return `${Q(qi)}: says no ${target} before #${beforeIndex + 1}, but ${Q(i)} has answer ${target}.`;
      }
    }
    return null;
  }
  if (v < 0) return null;

  if (answers[v] != null && answers[v] !== target) {
    return `${Q(qi)}: says closest ${target} before #${beforeIndex + 1} is ${Q(v)}, but ${Q(v)} is marked ${answers[v]}.`;
  }
  for (let i = beforeIndex - 1; i > v; i--) {
    if (answers[i] === target) {
      return `${Q(qi)}: says closest ${target} before #${beforeIndex + 1} is ${Q(v)}, but ${Q(i)} also has ${target} and is closer.`;
    }
  }
  return null;
}

function checkFirstWith(
  qi: number,
  answers: (AnswerLetter | null)[],
  v: number | null,
  target: AnswerLetter,
  n: number,
): string | null {
  if (v == null) {
    for (let i = 0; i < n; i++) {
      if (answers[i] === target) {
        return `${Q(qi)}: says no question has answer ${target}, but ${Q(i)} does.`;
      }
    }
    return null;
  }
  if (v < 0 || v >= n) return null;

  if (answers[v] != null && answers[v] !== target) {
    return `${Q(qi)}: says the first ${target} is ${Q(v)}, but ${Q(v)} is marked ${answers[v]}.`;
  }
  for (let i = 0; i < v; i++) {
    if (answers[i] === target) {
      return `${Q(qi)}: says the first ${target} is ${Q(v)}, but ${Q(i)} also has ${target} and comes earlier.`;
    }
  }
  return null;
}

function checkLastWith(
  qi: number,
  answers: (AnswerLetter | null)[],
  v: number | null,
  target: AnswerLetter,
  n: number,
): string | null {
  if (v == null) {
    for (let i = 0; i < n; i++) {
      if (answers[i] === target) {
        return `${Q(qi)}: says no question has answer ${target}, but ${Q(i)} does.`;
      }
    }
    return null;
  }
  if (v < 0 || v >= n) return null;

  if (answers[v] != null && answers[v] !== target) {
    return `${Q(qi)}: says the last ${target} is ${Q(v)}, but ${Q(v)} is marked ${answers[v]}.`;
  }
  for (let i = n - 1; i > v; i--) {
    if (answers[i] === target) {
      return `${Q(qi)}: says the last ${target} is ${Q(v)}, but ${Q(i)} also has ${target} and comes later.`;
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
      if (!isElim(markSets, qi, oi)) remaining.push(oi);
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
      const targetIdx = letterIdx(target);
      const oi = puzzle.questions[qi].options.findIndex((o) => o.value === targetIdx);
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
        const impliedIdx = optValue(puzzle, other, otherAns);
        if (impliedIdx != null && impliedIdx >= 0 && impliedIdx < 5) {
          const implied = LETTERS[impliedIdx];
          return hintSteps(
            qi,
            `${Q(qi)} can be determined now.`,
            `${Q(other)} is ${otherAns}, which says ${Q(qi)}'s answer is ${implied}.`,
            { type: "force", questionIndex: qi, letter: implied },
          );
        }
      }
      if (otherRule.type === "same_answer_as") {
        const targetQ = optValue(puzzle, other, otherAns);
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
    if (rule.type === "letter_distance" && answers[rule.questionIndex] != null) {
      const otherAnswer = answers[rule.questionIndex]!;
      const otherIdx = letterIdx(otherAnswer);
      const validLetters: AnswerLetter[] = [];
      for (let oi = 0; oi < 5; oi++) {
        if (isElim(markSets, qi, oi)) continue;
        const dist = Math.abs(oi - otherIdx);
        const optV = puzzle.questions[qi].options[oi].value;
        if (optV != null && dist === optV) validLetters.push(LETTERS[oi]);
      }
      if (validLetters.length === 1) {
        return hintSteps(
          qi,
          `${Q(qi)} can be determined now.`,
          `${Q(qi)}: ${Q(rule.questionIndex)} is ${otherAnswer}, and only option ${validLetters[0]} gives the right distance.`,
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
          if (isElim(markSets, qi, oi)) continue;
          if (puzzle.questions[qi].options[oi].value === count) {
            return hintSteps(
              qi,
              `${Q(qi)} can be determined now.`,
              `${Q(qi)}: all relevant questions are answered — there are ${count} ${countRuleLabel(rule, count)}, so it must be ${LETTERS[oi]}.`,
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
      const v = puzzle.questions[qi].options[oi].value;
      const ov = renderOptionLabel(rule, v, qi);
      const msg = canEliminate(rule, qi, oi, ov, v, answers, markSets, n);
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
  v: number | null,
  answers: (AnswerLetter | null)[],
  markSets: Marks[],
  n: number,
): string | null {
  // Count bounds
  const pred = countPred(rule);
  if (pred && rule.type !== "most_common_count") {
    const [from, to] = countRange(rule, n);
    const { count, remaining } = countMatching(answers, pred, from, to);
    if (v != null) {
      if (count > v) {
        return `says ${v}, but already ${count} ${countRuleLabel(rule, count)}.`;
      }
      if (count + remaining < v) {
        return `says ${v}, but at most ${count + remaining} ${countRuleLabel(rule, count + remaining)} are possible.`;
      }
    }
  }

  // answer_of_question: target already answered with different letter
  if (rule.type === "answer_of_question") {
    const target = answers[rule.questionIndex];
    if (target != null && v != null && letterIdx(target) !== v) {
      return `says ${Q(rule.questionIndex)} is ${LETTERS[v]}, but it's marked ${target}.`;
    }
  }

  // letter_distance: other question answered, distance doesn't match
  if (rule.type === "letter_distance") {
    const other = answers[rule.questionIndex];
    if (other != null && v != null) {
      const dist = Math.abs(oi - letterIdx(other));
      if (dist !== v) {
        return `says distance is ${ov}, but ${LETTERS[oi]} and ${other} are ${dist} apart.`;
      }
    }
  }

  // Positional: claimed position has wrong answer or answer eliminated
  if (isPositionalRule(rule.type) && v != null) {
    if (v >= 0 && v < n) {
      const posAnswer = answers[v];
      if (posAnswer != null) {
        const msg = checkPositionalElim(rule, v, posAnswer);
        if (msg) return msg;
      } else if (markSets) {
        const reqAnswer = positionalAnswer(rule);
        if (reqAnswer && isElim(markSets, v, L2I[reqAnswer])) {
          return `says ${Q(v)}, but ${reqAnswer} is already ruled out there.`;
        }
      }
    }
  }

  // Positional "None" but a match exists
  if (isPositionalRule(rule.type) && v == null) {
    const msg = checkNoneElim(rule, qi, answers, n);
    if (msg) return msg;
  }

  // Closer match exists for closest_after/before, first/last
  if (v != null) {
    const msg = checkCloserExists(rule, v, answers, n);
    if (msg) return msg;
  }

  // OnlySame/SameAs: can't point to self
  if ((rule.type === "only_same_answer" || rule.type === "same_answer_as") && v != null) {
    if (v === qi) return `can't refer to itself.`;
    if (v >= 0 && v < n && answers[v] != null && answers[qi] != null && answers[v] !== answers[qi]) {
      return `says ${Q(v)}, but ${Q(v)} is ${answers[v]} while this is ${answers[qi]}.`;
    }
  }

  // PrevSame: must point before this question
  if (rule.type === "previous_same_answer" && v != null) {
    if (v >= qi) return `says ${Q(v)}, but that's not before ${Q(qi)}.`;
    if (v >= 0 && v < n && answers[v] != null && answers[qi] != null && answers[v] !== answers[qi]) {
      return `says ${Q(v)}, but ${Q(v)} is ${answers[v]} while this is ${answers[qi]}.`;
    }
  }

  // NextSame: must point after this question
  if (rule.type === "next_same_answer" && v != null) {
    if (v <= qi || v >= n) return `says ${Q(v)}, but that's not after ${Q(qi)}.`;
  }

  // ConsecIdent: pair must have same answer
  if (rule.type === "consecutive_identical" && v != null) {
    if (v >= 0 && v + 1 < n && answers[v] != null && answers[v + 1] != null && answers[v] !== answers[v + 1]) {
      return `says ${Q(v)} & ${Q(v + 1)}, but they have different answers.`;
    }
  } else if (rule.type === "consecutive_identical" && v == null) {
    for (let i = 0; i < n - 1; i++) {
      if (answers[i] != null && answers[i + 1] != null && answers[i] === answers[i + 1]) {
        return `says none, but ${Q(i)} and ${Q(i + 1)} both have answer ${answers[i]}.`;
      }
    }
  }

  return null;
}

function positionalAnswer(rule: ValidationRule): AnswerLetter | null {
  if ("answer" in rule && rule.answer) return rule.answer;
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
  pos: number,
  posAnswer: AnswerLetter,
): string | null {
  if (
    rule.type === "closest_after" ||
    rule.type === "closest_before" ||
    rule.type === "first_with_answer" ||
    rule.type === "last_with_answer"
  ) {
    if (posAnswer !== rule.answer) {
      return `says ${Q(pos)} has answer ${rule.answer}, but it's marked ${posAnswer}.`;
    }
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
  pos: number,
  answers: (AnswerLetter | null)[],
  n: number,
): string | null {
  if (rule.type === "closest_after") {
    for (let i = rule.afterIndex + 1; i < pos; i++) {
      if (answers[i] === rule.answer) {
        return `says closest ${rule.answer} after #${rule.afterIndex + 1} is ${Q(pos)}, but ${Q(i)} has ${rule.answer} and is closer.`;
      }
    }
  }
  if (rule.type === "closest_before") {
    for (let i = rule.beforeIndex - 1; i > pos; i--) {
      if (answers[i] === rule.answer) {
        return `says closest ${rule.answer} before #${rule.beforeIndex + 1} is ${Q(pos)}, but ${Q(i)} has ${rule.answer} and is closer.`;
      }
    }
  }
  if (rule.type === "first_with_answer") {
    for (let i = 0; i < pos; i++) {
      if (answers[i] === rule.answer) {
        return `says first ${rule.answer} is ${Q(pos)}, but ${Q(i)} has ${rule.answer} and comes earlier.`;
      }
    }
  }
  if (rule.type === "last_with_answer") {
    for (let i = n - 1; i > pos; i--) {
      if (answers[i] === rule.answer) {
        return `says last ${rule.answer} is ${Q(pos)}, but ${Q(i)} has ${rule.answer} and comes later.`;
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
    const v = fp.optionValues[qi][ai];

    if (
      r.t === RT_COUNT_ANSWER ||
      r.t === RT_COUNT_ANSWER_BEFORE ||
      r.t === RT_COUNT_ANSWER_AFTER ||
      r.t === RT_COUNT_VOWEL ||
      r.t === RT_COUNT_CONSONANT
    ) {
      if (v != null) {
        const pred = countPred2(r);
        if (pred) {
          const [from, to] = countRange2(r, n);
          const cr = countMatchingAware(answers, markSets, pred, from, to);
          if (cr.count > v || cr.count + cr.remaining < v) {
            return { type: "contradiction", questionIndex: qi };
          }
        }
      }
    }
    if (r.t === RT_ANSWER_OF) {
      const target = answers[r.questionIndex];
      if (target != null && v != null && letterIdx(target) !== v) return { type: "contradiction", questionIndex: qi };
    }
    if (r.t === RT_LETTER_DIST) {
      const other = answers[r.questionIndex];
      if (other != null) {
        const dist = Math.abs(ai - letterIdx(other));
        if (dist !== v) return { type: "contradiction", questionIndex: qi };
      }
    }
    if (r.t === RT_UNIQUE) {
      let count = 0;
      for (let i = 0; i < n; i++) if (answers[i] === a) count++;
      if (count > 1) return { type: "contradiction", questionIndex: qi };
    }
    if (r.t === RT_CLOSEST_AFTER || r.t === RT_FIRST_WITH) {
      const afterIdx = r.t === RT_CLOSEST_AFTER ? r.afterIndex : -1;
      if (v != null) {
        if (v >= 0 && v < n && answers[v] != null && answers[v] !== r.answer) {
          return { type: "contradiction", questionIndex: qi };
        }
        for (let j = afterIdx + 1; j < v; j++) {
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
      if (v != null) {
        if (v >= 0 && v < n && answers[v] != null && answers[v] !== r.answer) {
          return { type: "contradiction", questionIndex: qi };
        }
        for (let j = beforeIdx - 1; j > v; j--) {
          if (answers[j] === r.answer) return { type: "contradiction", questionIndex: qi };
        }
      } else {
        for (let j = 0; j < beforeIdx; j++) {
          if (answers[j] === r.answer) return { type: "contradiction", questionIndex: qi };
        }
      }
    }
    if (r.t === RT_SAME_AS) {
      if (v != null && v >= 0 && v < n && answers[v] != null && answers[v] !== a) {
        return { type: "contradiction", questionIndex: qi };
      }
    }

    // Fallback: full evaluate for rule types without specialized checks
    if (
      r.t !== RT_COUNT_ANSWER &&
      r.t !== RT_COUNT_ANSWER_BEFORE &&
      r.t !== RT_COUNT_ANSWER_AFTER &&
      r.t !== RT_COUNT_VOWEL &&
      r.t !== RT_COUNT_CONSONANT &&
      r.t !== RT_ANSWER_OF &&
      r.t !== RT_LETTER_DIST &&
      r.t !== RT_UNIQUE &&
      r.t !== RT_CLOSEST_AFTER &&
      r.t !== RT_CLOSEST_BEFORE &&
      r.t !== RT_FIRST_WITH &&
      r.t !== RT_LAST_WITH &&
      r.t !== RT_SAME_AS
    ) {
      if (answers.slice(0, n).every((x) => x != null)) {
        if (!evaluateRule(r, qi, a, answers, fp)) {
          return { type: "contradiction", questionIndex: qi };
        }
      }
    }
  }

  // Count saturation
  for (let qi = 0; qi < n; qi++) {
    if (answers[qi] == null) continue;
    const r = fp.rules[qi];
    const pred = countPred2(r);
    if (!pred) continue;
    const ai = letterIdx(answers[qi]!);
    const v = fp.optionValues[qi][ai];
    if (v == null) continue;
    const [from, to] = countRange2(r, n);
    const cr = countMatchingAware(answers, markSets, pred, from, to);
    if (cr.count === v && cr.remaining > 0) {
      for (let j = from; j < to; j++) {
        if (answers[j] != null) continue;
        for (let oi = 0; oi < 5; oi++) {
          if (!isElim(markSets, j, oi) && pred(LETTERS[oi])) {
            return { type: "eliminate", questionIndex: j, optionIndex: oi };
          }
        }
      }
    }
    if (cr.count + cr.remaining === v && cr.remaining > 0) {
      for (let j = from; j < to; j++) {
        if (answers[j] != null || !canStillMatch(pred, markSets[j])) continue;
        for (let oi = 0; oi < 5; oi++) {
          if (!isElim(markSets, j, oi) && !pred(LETTERS[oi])) {
            return { type: "eliminate", questionIndex: j, optionIndex: oi };
          }
        }
      }
    }
  }

  // Vowel/consonant cross-elimination
  {
    let vowelQi = -1;
    let consonantQi = -1;
    for (let i = 0; i < n; i++) {
      if (answers[i] != null) continue;
      if (fp.rules[i].t === RT_COUNT_VOWEL) vowelQi = i;
      if (fp.rules[i].t === RT_COUNT_CONSONANT) consonantQi = i;
    }
    if (vowelQi >= 0 && consonantQi >= 0) {
      for (let oi = 0; oi < 5; oi++) {
        if (markSets[vowelQi][oi] !== "unmarked") continue;
        const vv = fp.optionValues[vowelQi][oi];
        if (vv == null) continue;
        const need = n - vv;
        let has = false;
        for (let coi = 0; coi < 5; coi++) {
          if (!isElim(markSets, consonantQi, coi) && fp.optionValues[consonantQi][coi] === need) {
            has = true;
            break;
          }
        }
        if (!has) return { type: "eliminate", questionIndex: vowelQi, optionIndex: oi };
      }
      for (let oi = 0; oi < 5; oi++) {
        if (markSets[consonantQi][oi] !== "unmarked") continue;
        const vv = fp.optionValues[consonantQi][oi];
        if (vv == null) continue;
        const need = n - vv;
        let has = false;
        for (let voi = 0; voi < 5; voi++) {
          if (!isElim(markSets, vowelQi, voi) && fp.optionValues[vowelQi][voi] === need) {
            has = true;
            break;
          }
        }
        if (!has) return { type: "eliminate", questionIndex: consonantQi, optionIndex: oi };
      }
    }
  }

  // Forced values
  for (let qi = 0; qi < n; qi++) {
    if (answers[qi] != null) continue;
    const r = fp.rules[qi];

    if (remCount(markSets, qi) === 0) {
      return { type: "contradiction", questionIndex: qi };
    }
    const remaining: number[] = [];
    for (let oi = 0; oi < 5; oi++) {
      if (!isElim(markSets, qi, oi)) remaining.push(oi);
    }
    if (remaining.length === 1) {
      return { type: "force", questionIndex: qi, letter: LETTERS[remaining[0]] };
    }

    if (r.t === RT_ANSWER_OF && answers[r.questionIndex] != null) {
      const target = answers[r.questionIndex]!;
      const targetIdx = letterIdx(target);
      for (let oi = 0; oi < 5; oi++) {
        if (fp.optionValues[qi][oi] === targetIdx) {
          return { type: "force", questionIndex: qi, letter: LETTERS[oi] };
        }
      }
    }

    // Reverse answer_of_question / SameAs
    for (let other = 0; other < n; other++) {
      const otherAns = answers[other];
      if (otherAns == null) continue;
      const otherR = fp.rules[other];
      if (otherR.t === RT_ANSWER_OF && otherR.questionIndex === qi) {
        const impliedIdx = fp.optionValues[other][letterIdx(otherAns)];
        if (impliedIdx != null && impliedIdx >= 0 && impliedIdx < 5) {
          return { type: "force", questionIndex: qi, letter: LETTERS[impliedIdx] };
        }
      }
      if (otherR.t === RT_SAME_AS) {
        const targetQ = fp.optionValues[other][letterIdx(otherAns)];
        if (targetQ != null && targetQ >= 0 && targetQ === qi) {
          return { type: "force", questionIndex: qi, letter: otherAns };
        }
      }
    }

    // LetterDist forced value
    if (r.t === RT_LETTER_DIST) {
      const otherAns = answers[r.questionIndex];
      if (otherAns != null) {
        const otherIdx = letterIdx(otherAns);
        let validCount = 0;
        let validLetter: AnswerLetter = "A";
        for (let oi = 0; oi < 5; oi++) {
          if (isElim(markSets, qi, oi)) continue;
          const dist = Math.abs(oi - otherIdx);
          if (dist === fp.optionValues[qi][oi]) {
            validCount++;
            validLetter = LETTERS[oi];
          }
        }
        if (validCount === 1) {
          return { type: "force", questionIndex: qi, letter: validLetter };
        }
      }
    }

    const pred = countPred2(r);
    if (pred) {
      const [from, to] = countRange2(r, n);
      const cr = countMatching(answers, pred, from, to);
      if (cr.remaining === 0) {
        for (let oi = 0; oi < 5; oi++) {
          if (isElim(markSets, qi, oi)) continue;
          if (fp.optionValues[qi][oi] === cr.count) {
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
      const v = fp.optionValues[qi][oi];

      const pred = countPred2(r);
      if (pred && r.t !== RT_MOST_COMMON_COUNT) {
        const [from, to] = countRange2(r, n);
        const cr = countMatchingAware(answers, markSets, pred, from, to);
        if (v != null && (cr.count > v || cr.count + cr.remaining < v)) {
          return { type: "eliminate", questionIndex: qi, optionIndex: oi };
        }
      }

      if (r.t === RT_ANSWER_OF) {
        const target = answers[r.questionIndex];
        if (target != null && v != null && letterIdx(target) !== v) {
          return { type: "eliminate", questionIndex: qi, optionIndex: oi };
        }
        // AnswerOf propagation: if the claimed letter is eliminated from the target
        if (target == null && v != null && v >= 0 && v < 5) {
          if (isElim(markSets, r.questionIndex, v)) {
            return { type: "eliminate", questionIndex: qi, optionIndex: oi };
          }
        }
      }

      if (r.t === RT_LETTER_DIST) {
        const other = answers[r.questionIndex];
        if (other != null && v != null && Math.abs(oi - letterIdx(other)) !== v) {
          return { type: "eliminate", questionIndex: qi, optionIndex: oi };
        }
      }

      if ((r.t === RT_CLOSEST_AFTER || r.t === RT_FIRST_WITH) && v != null) {
        const scanStart = r.t === RT_CLOSEST_AFTER ? r.afterIndex + 1 : 0;
        if (v < scanStart || v >= n) {
          return { type: "eliminate", questionIndex: qi, optionIndex: oi };
        }
        if (v >= 0 && v < n) {
          if (answers[v] != null && answers[v] !== r.answer) {
            return { type: "eliminate", questionIndex: qi, optionIndex: oi };
          }
          if (answers[v] == null && isElim(markSets, v, L2I[r.answer!])) {
            return { type: "eliminate", questionIndex: qi, optionIndex: oi };
          }
        }
        for (let j = scanStart; j < v; j++) {
          if (answers[j] === r.answer)
            return { type: "eliminate", questionIndex: qi, optionIndex: oi };
        }
      }
      if ((r.t === RT_CLOSEST_BEFORE || r.t === RT_LAST_WITH) && v != null) {
        const beforeIdx = r.t === RT_CLOSEST_BEFORE ? r.beforeIndex : n;
        if (v < 0 || v >= beforeIdx) {
          return { type: "eliminate", questionIndex: qi, optionIndex: oi };
        }
        if (v >= 0 && v < n) {
          if (answers[v] != null && answers[v] !== r.answer) {
            return { type: "eliminate", questionIndex: qi, optionIndex: oi };
          }
          if (answers[v] == null && isElim(markSets, v, L2I[r.answer!])) {
            return { type: "eliminate", questionIndex: qi, optionIndex: oi };
          }
        }
        for (let j = beforeIdx - 1; j > v; j--) {
          if (answers[j] === r.answer)
            return { type: "eliminate", questionIndex: qi, optionIndex: oi };
        }
      }
      if (v == null && (r.t === RT_CLOSEST_AFTER || r.t === RT_FIRST_WITH)) {
        const afterIdx = r.t === RT_CLOSEST_AFTER ? r.afterIndex : -1;
        for (let j = afterIdx + 1; j < n; j++) {
          if (answers[j] === r.answer)
            return { type: "eliminate", questionIndex: qi, optionIndex: oi };
        }
      }
      if (v == null && (r.t === RT_CLOSEST_BEFORE || r.t === RT_LAST_WITH)) {
        const beforeIdx = r.t === RT_CLOSEST_BEFORE ? r.beforeIndex : n;
        for (let j = beforeIdx - 1; j >= 0; j--) {
          if (answers[j] === r.answer)
            return { type: "eliminate", questionIndex: qi, optionIndex: oi };
        }
      }

      // OnlyOdd: position must be odd, in range, and could have the answer
      if (r.t === RT_ONLY_ODD) {
        if (v != null) {
          if ((v + 1) % 2 === 0) return { type: "eliminate", questionIndex: qi, optionIndex: oi };
          if (v >= 0 && v < n) {
            if (answers[v] != null && answers[v] !== r.answer) {
              return { type: "eliminate", questionIndex: qi, optionIndex: oi };
            }
            if (answers[v] == null && isElim(markSets, v, L2I[r.answer!])) {
              return { type: "eliminate", questionIndex: qi, optionIndex: oi };
            }
          }
        } else {
          for (let i = 0; i < n; i++) {
            if ((i + 1) % 2 === 1 && answers[i] === r.answer) {
              return { type: "eliminate", questionIndex: qi, optionIndex: oi };
            }
          }
        }
      }

      // ConsecIdent: pair must have same answer
      if (r.t === RT_CONSEC_IDENT) {
        if (v != null) {
          if (v >= 0 && v + 1 < n) {
            if (answers[v] != null && answers[v + 1] != null && answers[v] !== answers[v + 1]) {
              return { type: "eliminate", questionIndex: qi, optionIndex: oi };
            }
          }
        } else {
          for (let i = 0; i < n - 1; i++) {
            if (answers[i] != null && answers[i + 1] != null && answers[i] === answers[i + 1]) {
              return { type: "eliminate", questionIndex: qi, optionIndex: oi };
            }
          }
        }
      }

      // PrevSame: must point before qi
      if (r.t === RT_PREV_SAME && v != null) {
        if (v >= qi) return { type: "eliminate", questionIndex: qi, optionIndex: oi };
      }

      // NextSame: must point after qi
      if (r.t === RT_NEXT_SAME && v != null) {
        if (v <= qi || v >= n)
          return { type: "eliminate", questionIndex: qi, optionIndex: oi };
      }

      // OnlySame/SameAs: can't point to self
      if ((r.t === RT_ONLY_SAME || r.t === RT_SAME_AS) && v != null) {
        if (v === qi) return { type: "eliminate", questionIndex: qi, optionIndex: oi };
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

function canStillMatch(pred: (a: AnswerLetter) => boolean, marks: Marks): boolean {
  for (let oi = 0; oi < 5; oi++) {
    if (marks[oi] !== "incorrect" && pred(LETTERS[oi])) return true;
  }
  return false;
}

function countMatchingAware(
  answers: (AnswerLetter | null)[],
  markSets: Marks[],
  pred: (a: AnswerLetter) => boolean,
  from: number,
  to: number,
): CountResult {
  let count = 0;
  let remaining = 0;
  for (let i = from; i < to && i < answers.length; i++) {
    const a = answers[i];
    if (a == null) {
      if (canStillMatch(pred, markSets[i])) remaining++;
    } else if (pred(a)) count++;
  }
  return { count, remaining };
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
      if (isElim(markSets, qi, oi)) continue;

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
        const expl = result.explanation.replace(new RegExp(`^${Q(qi)}: `), "");
        const step4 = `${Q(qi)} can't be ${letter}: ${expl}`;
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
