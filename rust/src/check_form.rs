//! Structural well-formedness of a parsed puzzle (option counts, index ranges,
//! claim shape) — is the *shape* legal, independent of any answer key? For the
//! semantic "is this claim true?" check see `check_answer::check_claim`.

use crate::types::*;

#[derive(Debug)]
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
// checking a top-level question or one of a TrueStmt's per-option claims —
// errors attribute to the TrueStmt question in both cases). Both are
// wellformedness checks — for the **semantic** "is this claim true?" check
// see `check_answer::check_claim`.

fn warning(msg: impl Into<String>) -> Option<(String, Severity)> {
    Some((msg.into(), Severity::Warning))
}

fn error(msg: impl Into<String>) -> Option<(String, Severity)> {
    Some((msg.into(), Severity::Error))
}

/// Per-qt structural checks (value-independent): question_index references
/// in range and not self-ref (AnswerOf/LetterDist/SameAsWhich), and answer
/// letter within option count for types that carry an `answer` field. `qi` is
/// the owning question — when checking one of a TrueStmt's per-option claims,
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
    let oor = || (format!("value {ov} out of range"), Severity::Warning);

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
                warning(format!("OnlySame option {} references itself", opt.oi))
            } else if ov >= n {
                Some(oor())
            } else {
                None
            }
        }
        QuestionType::SameAsWhich { question_index } => {
            // Self / subject-ref / out-of-range are all structurally invalid targets
            // (check_answer rejects them unconditionally) and un-eliminable by deduce.
            // Error, mirroring SameAs — the value can never be a correct answer.
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
                warning(format!(
                    "EqualCount({}) points to {} (self-referencing)",
                    answer.as_char(),
                    answer.as_char()
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
        // Claims cannot be TrueStmt — nesting is not allowed.
        QuestionType::TrueStmt => error("TrueStmt is not a valid claim type".to_string()),
    }
}

pub fn check_form(fp: &FlatPuzzle) -> Vec<FormError> {
    let mut errors = Vec::new();
    let n = fp.n;
    let oc = fp.option_count;

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
            // TrueStmt: claim types live on the puzzle, claim values in this
            // row's options. Run the form checks per claim using SoA reads.
            for oi in 0..oc {
                let opt = OptionPos { qi, oi };
                let Some(claim) = fp.claim_at(qi, oi) else {
                    // Every option of a TrueStmt within `oc` must carry a claim.
                    errors.push(FormError {
                        qi,
                        message: format!("TrueStmt option {oi} has no claim"),
                        severity: Severity::Error,
                    });
                    continue;
                };
                let cqt = &claim.question_type;
                let cv = claim.value;
                // The claim's own QT also needs structural checks.
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
            // Per-qi: duplicate option values. Identity-option types are excluded.
            if !qt.has_identity_options() {
                let vals: Vec<OptionValue> = (0..oc).map(|oi| fp.options[qi][oi]).collect();
                let unique: std::collections::HashSet<OptionValue> = vals.iter().copied().collect();
                if unique.len() < vals.len() {
                    errors.push(FormError {
                        qi,
                        message: "Duplicate option values".into(),
                        severity: Severity::Warning,
                    });
                }
            }

            // Per-qi: null disallowed for types whose value is always defined.
            let null_not_allowed = matches!(
                qt,
                QuestionType::CountAnswer { .. }
                    | QuestionType::CountAnswerBefore { .. }
                    | QuestionType::CountAnswerAfter { .. }
                    | QuestionType::CountVowel
                    | QuestionType::CountConsonant
                    | QuestionType::MostCommonCount
                    | QuestionType::AnswerOf { .. }
                    | QuestionType::LeastCommon
                    | QuestionType::MostCommon
                    | QuestionType::LetterDist { .. }
                    | QuestionType::SameAsWhich { .. }
            );
            if null_not_allowed {
                for oi in 0..oc {
                    if fp.options[qi][oi].is_none() {
                        errors.push(FormError {
                            qi,
                            message: format!(
                                "Option {oi} is null but {:?} requires a value",
                                qt.kind()
                            ),
                            severity: Severity::Warning,
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
