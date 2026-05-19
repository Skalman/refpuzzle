import type { Puzzle } from "./types.ts";

export interface FormError {
  qi: number;
  message: string;
}

export function validatePuzzleForm(puzzle: Puzzle): FormError[] {
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
        errors.push({ qi, message: `${qt.type} references out-of-range question ${String(ref)}` });
      } else if (ref === qi) {
        errors.push({ qi, message: `${qt.type} references itself` });
      }
    }

    // ── SameAs checks ──
    if (qt.type === "SameAs") {
      for (let oi = 0; oi < oc; oi++) {
        const v = vals[oi];
        if (v == null) {
          errors.push({ qi, message: `SameAs option ${oi} is null` });
        } else if (v === qi) {
          errors.push({ qi, message: `SameAs option ${oi} references itself` });
        } else if (v < 0 || v >= n) {
          errors.push({
            qi,
            message: `SameAs option ${oi} references out-of-range question ${String(v)}`,
          });
        }
      }
      const targets = vals.filter((v) => v != null);
      const unique = new Set(targets);
      if (unique.size < targets.length) {
        errors.push({ qi, message: "SameAs has duplicate option targets" });
      }
    }

    // ── OnlySame: no self-references ──
    if (qt.type === "OnlySame") {
      for (let oi = 0; oi < oc; oi++) {
        if (vals[oi] === qi) {
          errors.push({ qi, message: `OnlySame option ${oi} references itself` });
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
          });
        }
      }
    }

    // ── No duplicate option values (for types where values should be unique) ──
    if (qt.type !== "TrueStmt" && qt.type !== "AnswerIsSelf" && qt.type !== "NoOtherHasAnswer") {
      const nonNull = vals.filter((v): v is number => v != null);
      const unique = new Set(nonNull);
      if (unique.size < nonNull.length) {
        errors.push({ qi, message: "Duplicate option values" });
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
              message: `Option ${oi} value ${String(v)} out of range [0,${String(n)}]`,
            });
          }
          break;
        case "CountAnswerBefore":
          if (v < 0 || v > qt.beforeIndex) {
            errors.push({
              qi,
              message: `Option ${oi} value ${String(v)} out of range [0,${String(qt.beforeIndex)}]`,
            });
          }
          break;
        case "CountAnswerAfter":
          if (v < 0 || v > n - qt.afterIndex - 1) {
            errors.push({ qi, message: `Option ${oi} value ${String(v)} out of range` });
          }
          break;
        case "AnswerOf":
        case "LeastCommon":
        case "MostCommon":
        case "NoOtherHasAnswer":
        case "EqualCount":
        case "AnswerIsSelf":
          if (v < 0 || v > 4) {
            errors.push({ qi, message: `Option ${oi} value ${String(v)} out of range [0,4]` });
          }
          break;
        case "LetterDist":
          if (v < 0 || v > 4) {
            errors.push({ qi, message: `Option ${oi} value ${String(v)} out of range [0,4]` });
          }
          break;
        case "FirstWith":
        case "LastWith":
        case "ClosestAfter":
        case "ClosestBefore":
        case "PrevSame":
        case "NextSame":
        case "OnlySame":
        case "SameAs":
        case "ConsecIdent":
        case "OnlyOdd":
        case "OnlyEven":
        case "SameAsWhich":
          if (v < 0 || v >= n) {
            errors.push({
              qi,
              message: `Option ${oi} value ${String(v)} out of range [0,${String(n - 1)}]`,
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
          });
        }
      }
    }
  }

  return errors;
}
