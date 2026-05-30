import type { Answer, Puzzle, QuestionTypeName } from "./types.ts";
import { LETTERS } from "./types.ts";

export type Severity = "warning" | "error";

// Types whose correct option value is always defined (null option is never the answer).
// SameAs allows null ("none" = no other question shares this answer); identity-option
// types (AnswerIsSelf, NoOtherHasAnswer) and TrueStmt are special-cased elsewhere.
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

export interface FormError {
  qi: number;
  message: string;
  severity: Severity;
}

export function checkForm(puzzle: Puzzle, solution: Answer[] = []): FormError[] {
  const errors: FormError[] = [];
  const n = puzzle.questions.length;
  const oc = puzzle.optionCount ?? 5;

  for (let qi = 0; qi < n; qi++) {
    const q = puzzle.questions[qi];
    const qt = q.questionType;
    const opts = q.options.slice(0, oc);

    // Collect numeric values (null for TrueStmt/identity options)
    const vals = opts.map((o) => o.value);

    // ── Type-specific reference checks ──
    if (qt.type === "AnswerOf" || qt.type === "LetterDist" || qt.type === "SameAsWhich") {
      const ref = qt.questionIndex;
      if (ref < 0 || ref >= n) {
        errors.push({
          qi,
          message: `${qt.type} references out-of-range question ${String(ref)}`,
          severity: "error",
        });
      } else if (ref === qi) {
        errors.push({ qi, message: `${qt.type} references itself`, severity: "error" });
      }
    }

    // ── Answer letter within option count ──
    if (
      qt.type === "CountAnswer" ||
      qt.type === "CountAnswerBefore" ||
      qt.type === "CountAnswerAfter" ||
      qt.type === "ClosestAfter" ||
      qt.type === "ClosestBefore" ||
      qt.type === "FirstWith" ||
      qt.type === "LastWith" ||
      qt.type === "OnlyOdd" ||
      qt.type === "OnlyEven" ||
      qt.type === "EqualCount"
    ) {
      const answerIdx = "ABCDE".indexOf(qt.answer);
      if (answerIdx >= oc) {
        errors.push({
          qi,
          message: `References answer ${qt.answer} which is outside option count ${oc}`,
          severity: "warning",
        });
      }
    }

    // ── SameAs checks ──
    if (qt.type === "SameAs") {
      // "none" is a legitimate option; duplicate targets/nulls are caught by the
      // general distinct-option-values check below.
      for (let oi = 0; oi < oc; oi++) {
        const v = vals[oi];
        if (v == null) continue;
        if (v === qi) {
          errors.push({ qi, message: `SameAs option ${oi} references itself`, severity: "error" });
        } else if (v < 0 || v >= n) {
          errors.push({
            qi,
            message: `SameAs option ${oi} references out-of-range question ${String(v)}`,
            severity: "error",
          });
        }
      }
    }

    // ── OnlySame: no self-references ──
    if (qt.type === "OnlySame") {
      for (let oi = 0; oi < oc; oi++) {
        if (vals[oi] === qi) {
          errors.push({
            qi,
            message: `OnlySame option ${oi} references itself`,
            severity: "warning",
          });
        }
      }
    }

    // ── EqualCount: option value must not point to the reference letter ──
    if (qt.type === "EqualCount") {
      const refIdx = "ABCDE".indexOf(qt.answer);
      for (let oi = 0; oi < oc; oi++) {
        if (vals[oi] === refIdx) {
          errors.push({
            qi,
            message: `EqualCount(${qt.answer}) option ${oi} points to ${qt.answer} (self-referencing)`,
            severity: "warning",
          });
        }
      }
    }

    // ── NoOtherHasAnswer: every other letter must appear in at least one other question ──
    if (qt.type === "NoOtherHasAnswer" && solution.length > 0) {
      const selfAns = solution[qi];
      const otherAnswers = solution.filter((_, j) => j !== qi);
      for (const letter of LETTERS.slice(0, oc)) {
        if (letter !== selfAns && !otherAnswers.includes(letter)) {
          errors.push({
            qi,
            message: `NoOtherHasAnswer: letter ${letter} also has no other question with that answer, so the correct option is ambiguous`,
            severity: "warning",
          });
        }
      }
    }

    // ── No duplicate option values (incl. at most one "none") ──
    if (qt.type !== "TrueStmt" && qt.type !== "AnswerIsSelf" && qt.type !== "NoOtherHasAnswer") {
      const unique = new Set(vals);
      if (unique.size < vals.length) {
        errors.push({ qi, message: "Duplicate option values", severity: "warning" });
      }
    }

    // ── Null disallowed for types whose value is always defined ──
    if (NULL_NOT_ALLOWED.has(qt.type)) {
      for (let oi = 0; oi < oc; oi++) {
        if (vals[oi] == null) {
          errors.push({
            qi,
            message: `Option ${String(oi)} is null but ${qt.type} requires a value`,
            severity: "warning",
          });
        }
      }
    }

    // ── Option values in valid range ──
    for (let oi = 0; oi < oc; oi++) {
      const v = vals[oi];
      if (v == null) continue;

      switch (qt.type) {
        case "CountAnswer":
        case "CountVowel":
        case "CountConsonant":
        case "MostCommonCount":
          if (v < 0 || v > n) {
            errors.push({
              qi,
              message: `Option ${String(oi)} value ${String(v)} out of range`,
              severity: "warning",
            });
          }
          break;
        case "CountAnswerBefore":
          if (v < 0 || v > qt.beforeIndex) {
            errors.push({
              qi,
              message: `Option ${String(oi)} value ${String(v)} out of range`,
              severity: "warning",
            });
          }
          break;
        case "CountAnswerAfter":
          if (v < 0 || v > n - qt.afterIndex - 1) {
            errors.push({
              qi,
              message: `Option ${String(oi)} value ${String(v)} out of range`,
              severity: "warning",
            });
          }
          break;
        case "AnswerOf":
        case "LeastCommon":
        case "MostCommon":
        case "NoOtherHasAnswer":
        case "EqualCount":
        case "AnswerIsSelf":
        case "LetterDist":
          if (v < 0 || v >= oc) {
            errors.push({
              qi,
              message: `Option ${String(oi)} value ${String(v)} out of range`,
              severity: "warning",
            });
          }
          break;
        case "FirstWith":
        case "LastWith":
        case "OnlySame":
        case "SameAs":
        case "SameAsWhich":
          if (v < 0 || v >= n) {
            errors.push({
              qi,
              message: `Option ${String(oi)} value ${String(v)} out of range`,
              severity: "warning",
            });
          }
          break;
        case "NextSame":
          if (v <= qi || v >= n) {
            errors.push({
              qi,
              message: `Option ${String(oi)} value ${String(v)} out of range`,
              severity: "warning",
            });
          }
          break;
        case "PrevSame":
          if (v < 0 || v >= qi) {
            errors.push({
              qi,
              message: `Option ${String(oi)} value ${String(v)} out of range`,
              severity: "warning",
            });
          }
          break;
        case "ClosestAfter":
          if (v <= qt.afterIndex || v >= n) {
            errors.push({
              qi,
              message: `Option ${String(oi)} value ${String(v)} out of range`,
              severity: "warning",
            });
          }
          break;
        case "ClosestBefore":
          if (v < 0 || v >= qt.beforeIndex) {
            errors.push({
              qi,
              message: `Option ${String(oi)} value ${String(v)} out of range`,
              severity: "warning",
            });
          }
          break;
        case "OnlyOdd":
          if (v < 0 || v >= n || v % 2 !== 0) {
            errors.push({
              qi,
              message: `Option ${String(oi)} value ${String(v)} out of range`,
              severity: "warning",
            });
          }
          break;
        case "OnlyEven":
          if (v < 0 || v >= n || v % 2 !== 1) {
            errors.push({
              qi,
              message: `Option ${String(oi)} value ${String(v)} out of range`,
              severity: "warning",
            });
          }
          break;
        case "ConsecIdent":
          if (v < 0 || v >= n - 1) {
            errors.push({
              qi,
              message: `Option ${String(oi)} value ${String(v)} out of range`,
              severity: "warning",
            });
          }
          break;
      }
    }

    // ── TrueStmt claim checks ──
    if (qt.type === "TrueStmt") {
      for (let oi = 0; oi < oc; oi++) {
        const opt = opts[oi];
        if (!("claim" in opt)) continue;
        const c = opt.claim;
        const cqt = c.questionType;
        if (cqt.type === "EqualCount" && c.value === "ABCDE".indexOf(cqt.answer)) {
          errors.push({
            qi,
            message: `TrueStmt option ${oi} has EqualCount(${cqt.answer}) pointing to ${cqt.answer} (self-referencing)`,
            severity: "warning",
          });
        }
      }
    }
  }

  return errors;
}
