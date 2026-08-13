//! Structural well-formedness of a parsed puzzle (option counts, index ranges,
//! statement shape) — is the *shape* legal, independent of any answer key? For the
//! semantic "is this claim true?" check see `check_answer::check_claim`.
//!
//! `Error` vs `Warning` — whether an already-published puzzle may keep the thing.
//! Nothing may keep an `Error`: a served puzzle carrying one gets edited, and the
//! engine may assume it away (see the `check_answer` module doc). A `Warning` is
//! grandfathered — still legal to load and check, only retired from generation.
//! `generated_puzzles_wellformed` enforces exactly that split: warnings tolerated on
//! served puzzles, never on later ones.

use crate::types::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug)]
pub struct FormError {
    pub qi: usize,
    pub message: String,
    pub severity: Severity,
}

// ── Internal form-check helpers ──
//
// Each returns `Option<(message, severity)>`. The caller wraps the message into
// a `FormError` and supplies the `qi` (the same qi is used whether we're
// checking a top-level question or one of a TrueStmt's per-option statements —
// errors attribute to the TrueStmt question in both cases).

// Generic over the message type so that `check_stmt_kind` can use them with a
// `&'static str`: it has to be a `const fn`, since `fill`'s `STMT_KINDS` derives from
// it at compile time.
const fn warning<M>(msg: M) -> Option<(M, Severity)> {
    Some((msg, Severity::Warning))
}

const fn error<M>(msg: M) -> Option<(M, Severity)> {
    Some((msg, Severity::Error))
}

/// Whether a question kind may appear as a TrueStmt statement, and how badly it may
/// not. The reason is prefixed with the kind at the call site, so phrase it to follow
/// one. The single authority for this: `fill`'s `STMT_KINDS` pick pool derives from it
/// at compile time (generating only the `None` kinds), so the pool and this check can't
/// drift apart.
///
/// The `Error` kinds say nothing standing alone — they lean on context a statement has
/// no room for (a candidate list) or on themselves (nesting, asserting their own truth)
/// — which is also why `check_claim` can't check one. The `Warning` kinds check fine and
/// are merely retired: they describe the statement's own row rather than their
/// question, or are excluded on taste.
pub(crate) const fn check_stmt_kind(kind: QuestionTypeKind) -> Option<(&'static str, Severity)> {
    use QuestionTypeKind::*;
    match kind {
        SameAs | SameAsWhich => {
            error("asks which of a candidate list matches, and a statement carries no list")
        }
        TrueStmt => error("cannot nest inside another statement"),
        AnswerIsSelf => error("holds whatever value it asserts"),

        PrevSame | NextSame | OnlySame | LetterDist | NoOtherHasAnswer | AnswerOf => {
            warning("is not a generated statement kind")
        }

        CountAnswer | CountAnswerBefore | CountAnswerAfter | CountVowel | CountConsonant
        | MostCommonCount | ClosestAfter | ClosestBefore | FirstWith | LastWith | OnlyOdd
        | OnlyEven | ConsecIdent | LeastCommon | MostCommon | EqualCount => None,
    }
}

/// Per-qt structural checks (value-independent): question_index references
/// in range and not self-ref (AnswerOf/LetterDist/SameAsWhich), and answer
/// letter within option count for types that carry an `answer` field. `qi` is
/// the owning question — when checking one of a TrueStmt's per-option statements,
/// this is the TrueStmt's qi.
fn check_question_form(
    fp: &FlatPuzzle,
    qi: usize,
    qt: &QuestionType,
) -> Option<(String, Severity)> {
    let n = fp.n;
    let oc = fp.option_count;

    // Reference checks (AnswerOf/LetterDist/SameAsWhich).
    if let QuestionType::AnswerOf { question_index }
    | QuestionType::LetterDist { question_index }
    | QuestionType::SameAsWhich { question_index } = qt
    {
        let ref_qi = *question_index as usize;
        if ref_qi >= n {
            return error(format!(
                "{:?} references out-of-range question {ref_qi}",
                qt.kind()
            ));
        }
        if ref_qi == qi {
            return error(format!("{:?} references itself", qt.kind()));
        }
    }

    // Positional index in range: `before_index` is an exclusive bound (so `n` is
    // fine); `after_index` is a position that needs a question after it.
    match qt {
        QuestionType::CountAnswerBefore { before_index, .. }
        | QuestionType::ClosestBefore { before_index, .. }
            if usize::from(*before_index) > n =>
        {
            return error(format!(
                "{:?} references out-of-range position {before_index}",
                qt.kind()
            ));
        }
        QuestionType::CountAnswerAfter { after_index, .. }
        | QuestionType::ClosestAfter { after_index, .. }
            if usize::from(*after_index) + 1 >= n =>
        {
            return error(format!(
                "{:?} references out-of-range position {after_index}",
                qt.kind()
            ));
        }
        _ => {}
    }

    // Answer letter within option count (for types with an `answer` field).
    let answer = match qt {
        QuestionType::CountAnswer { answer }
        | QuestionType::CountAnswerBefore { answer, .. }
        | QuestionType::CountAnswerAfter { answer, .. }
        | QuestionType::ClosestAfter { answer, .. }
        | QuestionType::ClosestBefore { answer, .. }
        | QuestionType::FirstWith { answer }
        | QuestionType::LastWith { answer }
        | QuestionType::OnlyOdd { answer }
        | QuestionType::OnlyEven { answer }
        | QuestionType::EqualCount { answer } => Some(*answer),
        _ => None,
    };
    if let Some(a) = answer
        && a.idx() >= oc
    {
        return warning(format!("answer {} outside option count {oc}", a.as_char()));
    }

    None
}

/// Per-(qt, value) wellformedness. Answer-letter and reference checks live in
/// `check_question_form`; this function focuses on value-level checks (range,
/// parity, EqualCount self-reference, per-option self-reference for SameAs /
/// OnlySame). Returns the first error found.
fn check_claim_form(
    fp: &FlatPuzzle,
    opt: OptionPos,
    qt: &QuestionType,
    ov: OptionValue,
) -> Option<(String, Severity)> {
    let n = fp.n;
    let oc = fp.option_count;
    let qi = opt.qi;

    // NONE / UNUSED: no value-level checks apply. Whether NONE is *disallowed*
    // for the type is enforced separately in `check_form`'s main loop.
    if !ov.is_num() {
        return None;
    }
    let ov = usize::from(ov.value());
    let oor = || (format!("value {ov} out of range"), Severity::Error);

    match qt {
        QuestionType::CountAnswer { .. }
        | QuestionType::CountVowel
        | QuestionType::CountConsonant
        | QuestionType::MostCommonCount => (ov > n).then(oor),
        QuestionType::CountAnswerBefore { before_index, .. } => {
            (ov > usize::from(*before_index)).then(oor)
        }
        QuestionType::CountAnswerAfter { after_index, .. } => {
            (ov + 1 + usize::from(*after_index) > n).then(oor)
        }
        QuestionType::FirstWith { .. } | QuestionType::LastWith { .. } => (ov >= n).then(oor),
        QuestionType::ClosestAfter { after_index, .. } => {
            (ov <= usize::from(*after_index) || ov >= n).then(oor)
        }
        QuestionType::ClosestBefore { before_index, .. } => {
            (ov >= usize::from(*before_index)).then(oor)
        }
        QuestionType::NextSame => (ov <= qi || ov >= n).then(oor),
        QuestionType::PrevSame => (ov >= qi).then(oor),
        QuestionType::SameAs => {
            if ov == qi {
                error(format!("SameAs option {} references itself", opt.oi))
            } else if ov >= n {
                error(format!(
                    "SameAs option {} references out-of-range question {ov}",
                    opt.oi
                ))
            } else {
                None
            }
        }
        QuestionType::OnlySame => {
            if ov == qi {
                error(format!("OnlySame option {} references itself", opt.oi))
            } else if ov >= n {
                Some(oor())
            } else {
                None
            }
        }
        QuestionType::SameAsWhich { question_index } => {
            // All three read as nonsense, so none can be the intended answer: offering
            // the subject question as a candidate for matching itself, offering this
            // question when picking it is what decides its answer, or naming a question
            // that doesn't exist. Error, mirroring SameAs.
            if ov == qi {
                error(format!("SameAsWhich option {} references itself", opt.oi))
            } else if ov == usize::from(*question_index) {
                error(format!(
                    "SameAsWhich option {} references its subject question {ov}",
                    opt.oi
                ))
            } else if ov >= n {
                error(format!(
                    "SameAsWhich option {} references out-of-range question {ov}",
                    opt.oi
                ))
            } else {
                None
            }
        }
        QuestionType::AnswerOf { .. }
        | QuestionType::LeastCommon
        | QuestionType::MostCommon
        | QuestionType::NoOtherHasAnswer => (ov >= oc).then(|| {
            (
                format!("letter index {ov} outside option count {oc}"),
                Severity::Error,
            )
        }),
        QuestionType::EqualCount { answer } => {
            if ov == answer.idx() {
                // Vacuous ("the same count as itself"), so it can never be the answer.
                // `valid_values` doesn't offer it and no shipped puzzle carries one.
                error(format!(
                    "EqualCount({answer}) points to {answer} (self-referencing)",
                ))
            } else if ov >= oc {
                Some(oor())
            } else {
                None
            }
        }
        QuestionType::OnlyOdd { .. } => (ov >= n || ov % 2 != 0).then(oor),
        QuestionType::OnlyEven { .. } => (ov >= n || ov % 2 != 1).then(oor),
        QuestionType::ConsecIdent => (ov + 1 >= n).then(oor),
        QuestionType::AnswerIsSelf | QuestionType::LetterDist { .. } => (ov >= oc).then(oor),
        // Nesting is rejected by `check_form`'s `check_stmt_kind` check, along with
        // the other kinds a statement can't express — no value-level check to add here.
        QuestionType::TrueStmt => None,
    }
}

pub fn check_form(fp: &FlatPuzzle) -> Vec<FormError> {
    let mut errors = Vec::new();
    let n = fp.n;
    let oc = fp.option_count;

    // option_count must be 3..=5; the per-question checks below assume a valid count.
    if !(3..=5).contains(&oc) {
        errors.push(FormError {
            qi: 0,
            message: format!("option count {oc} is not 3, 4, or 5"),
            severity: Severity::Error,
        });
        return errors;
    }

    // At most one TrueStmt question. The statement array is puzzle-wide,
    // so a second TrueStmt would silently share it and mis-evaluate. (A TrueStmt with
    // no statement array at all is caught per-option below.)
    let true_stmt_count = fp.question_types[..n]
        .iter()
        .filter(|qt| matches!(qt, QuestionType::TrueStmt))
        .count();
    if true_stmt_count > 1 {
        errors.push(FormError {
            qi: 0,
            message: format!("{true_stmt_count} TrueStmt questions; at most one is allowed"),
            severity: Severity::Error,
        });
    }

    for qi in 0..n {
        let qt = &fp.question_types[qi];

        // Per-qt structural reference checks.
        if let Some((msg, sev)) = check_question_form(fp, qi, qt) {
            errors.push(FormError {
                qi,
                message: msg,
                severity: sev,
            });
        }

        if matches!(qt, QuestionType::TrueStmt) {
            // TrueStmt: statement types live on the puzzle, their values in this
            // row's options. Run the form checks per statement using SoA reads.
            for oi in 0..oc {
                let opt = OptionPos { qi, oi };
                let Some(claim) = fp.claim_at(qi, oi) else {
                    // Every option of a TrueStmt within `oc` must carry a statement.
                    errors.push(FormError {
                        qi,
                        message: format!("TrueStmt option {oi} has no statement"),
                        severity: Severity::Error,
                    });
                    continue;
                };
                let cqt = &claim.question_type;
                let cv = claim.value;
                if let Some((reason, severity)) = check_stmt_kind(cqt.kind()) {
                    errors.push(FormError {
                        qi,
                        message: format!("TrueStmt option {oi}: {:?} {reason}", cqt.kind()),
                        severity,
                    });
                }
                // A statement must assert a concrete value. NONE is sometimes technically valid, but
                // shouldn't ever be emitted (it's considered "ugly").
                if cv.is_none() {
                    errors.push(FormError {
                        qi,
                        message: format!("TrueStmt option {oi}: statement asserts none"),
                        severity: Severity::Warning,
                    });
                }
                // The statement's own QT also needs structural checks.
                if let Some((msg, sev)) = check_question_form(fp, qi, cqt) {
                    errors.push(FormError {
                        qi,
                        message: format!("TrueStmt option {oi}: {msg}"),
                        severity: sev,
                    });
                }
                if let Some((msg, sev)) = check_claim_form(fp, opt, cqt, cv) {
                    errors.push(FormError {
                        qi,
                        message: format!("TrueStmt option {oi}: {msg}"),
                        severity: sev,
                    });
                }
            }
        } else {
            // Per-qi: duplicate option values — the same choice offered twice, so
            // two options are equally right. Identity-option types are excluded (their
            // values are fixed by position), and so is TrueStmt, above: distinct statements
            // may legitimately assert the same number.
            if !qt.has_identity_options() {
                let vals: Vec<OptionValue> = (0..oc).map(|oi| fp.options[qi][oi]).collect();
                let unique: std::collections::HashSet<OptionValue> = vals.iter().copied().collect();
                if unique.len() < vals.len() {
                    errors.push(FormError {
                        qi,
                        message: "Duplicate option values".into(),
                        severity: Severity::Error,
                    });
                }
            }

            // Per-qi: NONE disallowed for kinds whose answer is always a value.
            if !qt.kind().may_be_none() {
                for oi in 0..oc {
                    if fp.options[qi][oi].is_none() {
                        errors.push(FormError {
                            qi,
                            message: format!(
                                "Option {oi} is null but {:?} requires a value",
                                qt.kind()
                            ),
                            severity: Severity::Error,
                        });
                    }
                }
            }

            // Per-oi: pass the option value to check_claim_form (which handles
            // NONE / UNUSED internally by returning no error).
            for oi in 0..oc {
                let opt = OptionPos { qi, oi };
                let ov = fp.options[qi][oi];
                if ov.is_unused() {
                    // UNUSED is only legal past `oc`.
                    errors.push(FormError {
                        qi,
                        message: format!("Option {oi} is UNUSED but within option count {oc}"),
                        severity: Severity::Error,
                    });
                    continue;
                }
                if let Some((msg, sev)) = check_claim_form(fp, opt, qt, ov) {
                    errors.push(FormError {
                        qi,
                        message: format!("Option {oi}: {msg}"),
                        severity: sev,
                    });
                }
            }
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a `FlatPuzzle` directly from question types, option rows, and an
    /// optional statement array — no JSON round-trip, so tests can construct the
    /// malformed shapes `parse_puzzle` would otherwise coerce or reject.
    fn flat(
        question_types: &[QuestionType],
        options: &[[OptionValue; 5]],
        true_stmt_question_types: Option<[QuestionType; 5]>,
        option_count: usize,
    ) -> FlatPuzzle {
        let n = question_types.len();
        let mut qts = [QuestionType::AnswerIsSelf; MAX_N];
        let mut opts = [[OptionValue::UNUSED; 5]; MAX_N];
        qts[..n].copy_from_slice(question_types);
        opts[..options.len()].copy_from_slice(options);
        let (affected_by, global_indices) = FlatPuzzle::build_deps(&qts, n);
        FlatPuzzle {
            question_types: qts,
            options: opts,
            true_stmt_question_types,
            affected_by,
            global_indices,
            n,
            option_count,
            initial_state: State::initial(option_count),
        }
    }

    #[test]
    fn option_count_must_be_3_to_5() {
        // oc=2 is what a ragged first option row can yield; parse never validates it.
        let fp = flat(
            &[QuestionType::AnswerIsSelf],
            &[[OptionValue::UNUSED; 5]],
            None,
            2,
        );
        let errs = check_form(&fp);
        assert!(
            errs.iter()
                .any(|e| e.severity == Severity::Error && e.message.contains("option count 2")),
            "oc=2 should be a fatal form error: {errs:?}"
        );
    }

    #[test]
    fn multiple_true_stmt_is_error() {
        let claims = [QuestionType::CountAnswer { answer: Answer::A }; 5];
        let fp = flat(
            &[QuestionType::TrueStmt, QuestionType::TrueStmt],
            &[[OptionValue::num(0); 5], [OptionValue::num(0); 5]],
            Some(claims),
            5,
        );
        let errs = check_form(&fp);
        assert!(
            errs.iter()
                .any(|e| e.severity == Severity::Error && e.message.contains("TrueStmt questions")),
            "two TrueStmt questions should be a fatal form error: {errs:?}"
        );
    }

    /// Shared fixtures: each case is a puzzle plus whether it should raise a *fatal*
    /// error. Warnings are deliberately not asserted on — they're advisory, and
    /// `expectError: false` cases carry some.
    #[test]
    fn test_shared_check_form() {
        let json_str = std::fs::read_to_string("../tests/check-form.json")
            .expect("can't read tests/check-form.json");
        let suite: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        let mut passed = 0;
        let mut failed = 0;
        for test in suite["tests"].as_array().unwrap() {
            if test.get("section").is_some() {
                continue;
            }
            let name = test["name"].as_str().unwrap();
            let expect_error = test["expectError"].as_bool().unwrap();
            let Some(fp) = crate::serialize::parse_puzzle(&test["puzzle"]) else {
                eprintln!("FAIL: {name}: parse failed");
                failed += 1;
                continue;
            };
            let errors = check_form(&fp);
            let got = errors.iter().any(|e| e.severity == Severity::Error);
            if got == expect_error {
                passed += 1;
                continue;
            }
            failed += 1;
            eprintln!("FAIL: {name}: expected fatal={expect_error}, got {got}");
            for e in &errors {
                eprintln!("    Q{} {:?}: {}", e.qi + 1, e.severity, e.message);
            }
        }

        eprintln!("{passed}/{} passed", passed + failed);
        assert_eq!(failed, 0, "{failed} check-form case(s) failed");
    }

    /// `check_answer` asserts on this rather than checking it, so the fatal severity is
    /// what keeps it out — worth pinning directly.
    #[test]
    fn equal_count_self_reference_is_error() {
        let fp = flat(
            &[QuestionType::EqualCount { answer: Answer::A }],
            &[[
                OptionValue::num(0),
                OptionValue::num(1),
                OptionValue::num(2),
                OptionValue::NONE,
                OptionValue::UNUSED,
            ]],
            None,
            4,
        );
        let errs = check_form(&fp);
        assert!(
            errs.iter()
                .any(|e| e.severity == Severity::Error && e.message.contains("self-referencing")),
            "EqualCount pointing at its own letter should be fatal: {errs:?}"
        );
    }

    #[test]
    fn none_stmt_value_is_warning() {
        // A NONE statement value is flagged for any kind — even a may_be_none one like
        // ConsecIdent, which the (removed) may_be_none-gated check would have missed.
        let claims = [QuestionType::ConsecIdent; 5];
        let fp = flat(
            &[QuestionType::TrueStmt],
            &[[OptionValue::NONE; 5]],
            Some(claims),
            5,
        );
        let errs = check_form(&fp);
        assert!(
            errs.iter()
                .any(|e| e.severity == Severity::Warning
                    && e.message.contains("statement asserts none")),
            "a NONE claim value should warn: {errs:?}"
        );
    }
}
