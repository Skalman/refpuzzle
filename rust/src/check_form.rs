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
// errors attribute to the TrueStmt question in both cases). All three are
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
    value: i16,
) -> Option<(String, Severity)> {
    let n = fp.n;
    let oc = fp.option_count;
    let qi = opt.qi;

    // Null short-circuit: no value-level checks apply when the option's value
    // is NONE_VAL. Whether null is *disallowed* for the type is enforced
    // separately in `check_form`'s main loop.
    if value == NONE_VAL {
        return None;
    }

    match qt {
        QuestionType::CountAnswer { .. }
        | QuestionType::CountVowel
        | QuestionType::CountConsonant
        | QuestionType::MostCommonCount => {
            if value < 0 || value > n as i16 {
                return warning(format!("value {value} out of range"));
            }
            None
        }
        QuestionType::CountAnswerBefore { before_index, .. } => {
            if value < 0 || value > *before_index as i16 {
                return warning(format!("value {value} out of range"));
            }
            None
        }
        QuestionType::CountAnswerAfter { after_index, .. } => {
            if value < 0 || value > (n as i16 - 1 - *after_index as i16) {
                return warning(format!("value {value} out of range"));
            }
            None
        }
        QuestionType::FirstWith { .. } | QuestionType::LastWith { .. } => {
            if value < 0 || value >= n as i16 {
                return warning(format!("value {value} out of range"));
            }
            None
        }
        QuestionType::ClosestAfter { after_index, .. } => {
            if value <= *after_index as i16 || value >= n as i16 {
                return warning(format!("value {value} out of range"));
            }
            None
        }
        QuestionType::ClosestBefore { before_index, .. } => {
            if value < 0 || value >= *before_index as i16 {
                return warning(format!("value {value} out of range"));
            }
            None
        }
        QuestionType::NextSame => {
            if value <= qi as i16 || value >= n as i16 {
                return warning(format!("value {value} out of range"));
            }
            None
        }
        QuestionType::PrevSame => {
            if value < 0 || value >= qi as i16 {
                return warning(format!("value {value} out of range"));
            }
            None
        }
        QuestionType::SameAs => {
            if value as usize == qi {
                return error(format!("SameAs option {} references itself", opt.oi));
            }
            if value < 0 || value >= n as i16 {
                return error(format!(
                    "SameAs option {} references out-of-range question {value}",
                    opt.oi
                ));
            }
            None
        }
        QuestionType::OnlySame => {
            if value == qi as i16 {
                return warning(format!("OnlySame option {} references itself", opt.oi));
            }
            if value < 0 || value >= n as i16 {
                return warning(format!("value {value} out of range"));
            }
            None
        }
        QuestionType::SameAsWhich { .. } => {
            if value < 0 || value >= n as i16 {
                return warning(format!("value {value} out of range"));
            }
            None
        }
        QuestionType::AnswerOf { .. }
        | QuestionType::LeastCommon
        | QuestionType::MostCommon
        | QuestionType::NoOtherHasAnswer => {
            if value < 0 || value >= oc as i16 {
                return warning(format!("letter index {value} outside option count {oc}"));
            }
            None
        }
        QuestionType::EqualCount { answer } => {
            if value == answer.idx() as i16 {
                return warning(format!(
                    "EqualCount({}) points to {} (self-referencing)",
                    answer.as_char(),
                    answer.as_char()
                ));
            }
            if value < 0 || value >= oc as i16 {
                return warning(format!("value {value} out of range"));
            }
            None
        }
        QuestionType::OnlyOdd { .. } => {
            if value < 0 || value >= n as i16 || value % 2 != 0 {
                return warning(format!("value {value} out of range"));
            }
            None
        }
        QuestionType::OnlyEven { .. } => {
            if value < 0 || value >= n as i16 || value % 2 != 1 {
                return warning(format!("value {value} out of range"));
            }
            None
        }
        QuestionType::ConsecIdent => {
            if value < 0 || value >= n as i16 - 1 {
                return warning(format!("value {value} out of range"));
            }
            None
        }
        QuestionType::AnswerIsSelf | QuestionType::LetterDist { .. } => {
            if value < 0 || value >= oc as i16 {
                return warning(format!("value {value} out of range"));
            }
            None
        }
        // Claims cannot be TrueStmt — nesting is not allowed.
        QuestionType::TrueStmt => error("TrueStmt is not a valid claim type".to_string()),
    }
}

/// Per-(qt, value) checks that depend on the puzzle's solution. Currently only
/// `NoOtherHasAnswer` ambiguity: if the asserted letter has no other question
/// answering it AND another letter also has no other question answering it,
/// the correct option is ambiguous.
fn check_correct_claim_form(
    fp: &FlatPuzzle,
    solution: &[Answer],
    opt: OptionPos,
    qt: &QuestionType,
    value: i16,
) -> Option<(String, Severity)> {
    let oc = fp.option_count;
    if !matches!(qt, QuestionType::NoOtherHasAnswer) {
        return None;
    }
    if value < 0 || value >= oc as i16 {
        return None; // out-of-range letter caught by check_claim_form
    }
    let self_ans = LETTERS[value as usize];
    for letter in LETTERS.iter().take(oc) {
        if *letter != self_ans
            && !solution
                .iter()
                .enumerate()
                .any(|(j, &a)| j != opt.qi && a == *letter)
        {
            return warning(format!(
                "NoOtherHasAnswer: letter {} also has no other question with that answer, so the correct option is ambiguous",
                letter.as_char()
            ));
        }
    }
    None
}

pub fn check_form(fp: &FlatPuzzle, solution: Option<&[Answer]>) -> Vec<FormError> {
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
                    continue;
                };
                let cqt = &claim.question_type;
                let cv = claim.value.to_i16();
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
                if let Some(sol) = solution
                    && let Some((msg, sev)) = check_correct_claim_form(fp, sol, opt, cqt, cv)
                {
                    errors.push(FormError {
                        qi,
                        message: format!("TrueStmt option {oi}: {msg}"),
                        severity: sev,
                    });
                }
            }
        } else {
            // Per-qi: duplicate option values. Letter-valued slots carry NAN_VAL in
            // option_nums and store the letter in option_answers, so compare on the
            // unified value. Identity-option types are excluded.
            if !qt.has_identity_options() {
                let vals: Vec<i16> = (0..oc).map(|oi| fp.options[qi][oi].to_i16()).collect();
                let unique: std::collections::HashSet<i16> = vals.iter().copied().collect();
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
            );
            if null_not_allowed {
                for oi in 0..oc {
                    if fp.options[qi][oi].to_i16() == NONE_VAL {
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

            // Per-oi: look up the option's value (letter index for letter-typed,
            // numeric otherwise) and check.
            let letter_valued = matches!(
                qt,
                QuestionType::AnswerOf { .. }
                    | QuestionType::LeastCommon
                    | QuestionType::MostCommon
            );
            for oi in 0..oc {
                let opt = OptionPos { qi, oi };
                let value = if letter_valued {
                    let s = fp.options[qi][oi];
                    if s.is_num() {
                        s.value() as i16
                    } else {
                        NAN_VAL
                    }
                } else {
                    fp.options[qi][oi].to_i16()
                };
                if value == NAN_VAL {
                    continue;
                }
                if let Some((msg, sev)) = check_claim_form(fp, opt, qt, value) {
                    errors.push(FormError {
                        qi,
                        message: format!("Option {oi}: {msg}"),
                        severity: sev,
                    });
                }
            }

            // Per-qi solution-dependent (currently NoOtherHasAnswer ambiguity).
            // For NoOtherHasAnswer, the asserted letter is the puzzle's answer at qi;
            // for other types the function no-ops so we pass a placeholder value.
            if let Some(sol) = solution {
                let value = match qt {
                    QuestionType::NoOtherHasAnswer => sol[qi].idx() as i16,
                    _ => 0,
                };
                if let Some((msg, sev)) =
                    check_correct_claim_form(fp, sol, OptionPos { qi, oi: 0 }, qt, value)
                {
                    errors.push(FormError {
                        qi,
                        message: msg,
                        severity: sev,
                    });
                }
            }
        }
    }

    errors
}
