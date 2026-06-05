import type { Answer, OptionPos, Puzzle, QuestionType, QuestionTypeName } from "./types.ts";
import { LETTERS } from "./types.ts";

export type Severity = "warning" | "error";

export interface FormError {
  qi: number;
  message: string;
  severity: Severity;
}

// ── Internal form-check helpers ──
//
// Each returns `[message, severity] | null`. The caller wraps into a FormError
// and supplies the `qi` (the same qi is used whether we're checking a top-
// level question or one of a TrueStmt's per-option claims — errors attribute
// to the TrueStmt question in both cases). All three are wellformedness
// checks — for the **semantic** "is this claim true?" check see
// `check-answer.ts`'s `checkClaim`.

function warning(msg: string): [string, Severity] {
  return [msg, "warning"];
}
function error(msg: string): [string, Severity] {
  return [msg, "error"];
}

/**
 * Per-qt structural checks (value-independent): question_index references in
 * range and not self-ref (AnswerOf/LetterDist/SameAsWhich), and answer letter
 * within option count for types that carry an `answer` field. `qi` is the
 * owning question — when checking one of a TrueStmt's per-option claims,
 * this is the TrueStmt's qi.
 */
function checkQuestionForm(
  n: number,
  oc: number,
  qi: number,
  qt: QuestionType,
): [string, Severity] | null {
  // Reference checks (AnswerOf/LetterDist/SameAsWhich).
  if (qt.type === "AnswerOf" || qt.type === "LetterDist" || qt.type === "SameAsWhich") {
    const refQi = qt.questionIndex;
    if (refQi < 0 || refQi >= n) {
      return error(`${qt.type} references out-of-range question ${String(refQi)}`);
    }
    if (refQi === qi) {
      return error(`${qt.type} references itself`);
    }
  }

  // Answer letter within option count (for types with an `answer` field).
  if ("answer" in qt && LETTERS.indexOf(qt.answer) >= oc) {
    return warning(`answer ${qt.answer} outside option count ${String(oc)}`);
  }

  return null;
}

/**
 * Per-(qt, value) wellformedness. Answer-letter and reference checks live in
 * `checkQuestionForm`; this function focuses on value-level checks (range,
 * parity, EqualCount self-reference, per-option self-reference for SameAs /
 * OnlySame). Returns the first error found.
 */
function checkClaimForm(
  n: number,
  oc: number,
  opt: OptionPos,
  qt: QuestionType,
  value: number | null,
): [string, Severity] | null {
  // Null short-circuits the value-range check. Whether null is *disallowed*
  // for the type is enforced separately in `checkForm`'s main loop.
  if (value == null) return null;

  const qi = opt.qi;

  switch (qt.type) {
    case "CountAnswer":
    case "CountVowel":
    case "CountConsonant":
    case "MostCommonCount":
      if (value < 0 || value > n) return warning(`value ${String(value)} out of range`);
      return null;
    case "CountAnswerBefore":
      if (value < 0 || value > qt.beforeIndex)
        return warning(`value ${String(value)} out of range`);
      return null;
    case "CountAnswerAfter":
      if (value < 0 || value > n - qt.afterIndex - 1)
        return warning(`value ${String(value)} out of range`);
      return null;
    case "FirstWith":
    case "LastWith":
      if (value < 0 || value >= n) return warning(`value ${String(value)} out of range`);
      return null;
    case "ClosestAfter":
      if (value <= qt.afterIndex || value >= n)
        return warning(`value ${String(value)} out of range`);
      return null;
    case "ClosestBefore":
      if (value < 0 || value >= qt.beforeIndex)
        return warning(`value ${String(value)} out of range`);
      return null;
    case "NextSame":
      if (value <= qi || value >= n) return warning(`value ${String(value)} out of range`);
      return null;
    case "PrevSame":
      if (value < 0 || value >= qi) return warning(`value ${String(value)} out of range`);
      return null;
    case "SameAs":
      if (value === qi) return error(`SameAs option ${String(opt.oi)} references itself`);
      if (value < 0 || value >= n)
        return error(
          `SameAs option ${String(opt.oi)} references out-of-range question ${String(value)}`,
        );
      return null;
    case "OnlySame":
      if (value === qi) return warning(`OnlySame option ${String(opt.oi)} references itself`);
      if (value < 0 || value >= n) return warning(`value ${String(value)} out of range`);
      return null;
    case "SameAsWhich":
      if (value < 0 || value >= n) return warning(`value ${String(value)} out of range`);
      return null;
    case "AnswerOf":
    case "LeastCommon":
    case "MostCommon":
    case "NoOtherHasAnswer":
      if (value < 0 || value >= oc)
        return warning(`letter index ${String(value)} outside option count ${String(oc)}`);
      return null;
    case "EqualCount":
      if (value === LETTERS.indexOf(qt.answer))
        return warning(`EqualCount(${qt.answer}) points to ${qt.answer} (self-referencing)`);
      if (value < 0 || value >= oc) return warning(`value ${String(value)} out of range`);
      return null;
    case "OnlyOdd":
      if (value < 0 || value >= n || value % 2 !== 0)
        return warning(`value ${String(value)} out of range`);
      return null;
    case "OnlyEven":
      if (value < 0 || value >= n || value % 2 !== 1)
        return warning(`value ${String(value)} out of range`);
      return null;
    case "ConsecIdent":
      if (value < 0 || value >= n - 1) return warning(`value ${String(value)} out of range`);
      return null;
    case "AnswerIsSelf":
    case "LetterDist":
      if (value < 0 || value >= oc) return warning(`value ${String(value)} out of range`);
      return null;
    case "TrueStmt":
      // Claims cannot be TrueStmt — nesting is not allowed.
      return error("TrueStmt is not a valid claim type");
  }
  return null;
}

/**
 * Per-(qt, value) checks that depend on the puzzle's solution. Currently only
 * `NoOtherHasAnswer` ambiguity.
 */
function checkCorrectClaimForm(
  oc: number,
  solution: Answer[],
  opt: OptionPos,
  qt: QuestionType,
  value: number | null,
): [string, Severity] | null {
  if (qt.type !== "NoOtherHasAnswer") return null;
  if (value == null || value < 0 || value >= oc) return null;
  const selfAns = LETTERS[value];
  for (let li = 0; li < oc; li++) {
    const letter = LETTERS[li];
    if (letter === selfAns) continue;
    let hasOther = false;
    for (let j = 0; j < solution.length; j++) {
      if (j !== opt.qi && solution[j] === letter) {
        hasOther = true;
        break;
      }
    }
    if (!hasOther) {
      return warning(
        `NoOtherHasAnswer: letter ${letter} also has no other question with that answer, so the correct option is ambiguous`,
      );
    }
  }
  return null;
}

const NULL_NOT_ALLOWED: ReadonlySet<QuestionTypeName> = new Set([
  "CountAnswer",
  "CountAnswerBefore",
  "CountAnswerAfter",
  "CountVowel",
  "CountConsonant",
  "MostCommonCount",
  "AnswerOf",
  "LeastCommon",
  "MostCommon",
  "LetterDist",
]);

const IDENTITY_OPTION: ReadonlySet<QuestionTypeName> = new Set([
  "AnswerIsSelf",
  "NoOtherHasAnswer",
]);

export function checkForm(puzzle: Puzzle, solution: Answer[] = []): FormError[] {
  const errors: FormError[] = [];
  const n = puzzle.questions.length;
  const oc = puzzle.optionCount ?? 5;
  const haveSolution = solution.length > 0;

  for (let qi = 0; qi < n; qi++) {
    const q = puzzle.questions[qi];
    const qt = q.questionType;

    // Per-qt structural checks.
    const qErr = checkQuestionForm(n, oc, qi, qt);
    if (qErr) errors.push({ qi, message: qErr[0], severity: qErr[1] });

    if (qt.type === "TrueStmt") {
      // TrueStmt: per-option types live on the puzzle, per-option values in
      // this row's options. Run the form checks against the SoA reads.
      const types = puzzle.trueStmtQuestionTypes;
      if (!types) continue;
      for (let oi = 0; oi < oc; oi++) {
        const opt: OptionPos = { qi, oi };
        const value = q.options[oi]?.value ?? null;
        const push = (err: [string, Severity] | null) => {
          if (err) {
            errors.push({
              qi,
              message: `TrueStmt option ${String(oi)}: ${err[0]}`,
              severity: err[1],
            });
          }
        };
        push(checkQuestionForm(n, oc, qi, types[oi]));
        push(checkClaimForm(n, oc, opt, types[oi], value));
        if (haveSolution) push(checkCorrectClaimForm(oc, solution, opt, types[oi], value));
      }
      continue;
    }

    // Per-qi: duplicate option values. Identity-option types excluded.
    if (!IDENTITY_OPTION.has(qt.type)) {
      const vals = q.options.slice(0, oc).map((o) => o.value);
      const unique = new Set(vals);
      if (unique.size < vals.length) {
        errors.push({ qi, message: "Duplicate option values", severity: "warning" });
      }
    }

    // Per-qi: null disallowed for types whose value is always defined.
    if (NULL_NOT_ALLOWED.has(qt.type)) {
      for (let oi = 0; oi < oc; oi++) {
        if (q.options[oi].value == null) {
          errors.push({
            qi,
            message: `Option ${String(oi)} is null but ${qt.type} requires a value`,
            severity: "warning",
          });
        }
      }
    }

    // Per-oi: pass the option's (qt, value) to check_claim_form.
    for (let oi = 0; oi < oc; oi++) {
      const value = q.options[oi].value;
      if (value == null) continue;
      const cErr = checkClaimForm(n, oc, { qi, oi }, qt, value);
      if (cErr) {
        errors.push({
          qi,
          message: `Option ${String(oi)}: ${cErr[0]}`,
          severity: cErr[1],
        });
      }
    }

    // Per-qi solution-dependent (currently NoOtherHasAnswer ambiguity).
    // For NoOtherHasAnswer, the asserted letter is the solution at qi.
    if (haveSolution) {
      const value = qt.type === "NoOtherHasAnswer" ? LETTERS.indexOf(solution[qi]) : 0;
      const ccErr = checkCorrectClaimForm(oc, solution, { qi, oi: 0 }, qt, value);
      if (ccErr) errors.push({ qi, message: ccErr[0], severity: ccErr[1] });
    }
  }

  return errors;
}
