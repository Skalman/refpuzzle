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

pub fn check_form(fp: &FlatPuzzle, solution: Option<&[Answer]>) -> Vec<FormError> {
    let mut errors = Vec::new();
    let n = fp.n;
    let oc = fp.option_count;

    for qi in 0..n {
        let qt = &fp.question_types[qi];

        // Type-specific reference checks
        match qt {
            QuestionType::AnswerOf { question_index }
            | QuestionType::LetterDist { question_index }
            | QuestionType::SameAsWhich { question_index } => {
                let type_name = match qt {
                    QuestionType::AnswerOf { .. } => "AnswerOf",
                    QuestionType::LetterDist { .. } => "LetterDist",
                    QuestionType::SameAsWhich { .. } => "SameAsWhich",
                    _ => unreachable!(),
                };
                let ref_qi = *question_index as usize;
                if ref_qi >= n {
                    errors.push(FormError {
                        qi,
                        message: format!("{type_name} references out-of-range question {ref_qi}"),
                        severity: Severity::Error,
                    });
                } else if ref_qi == qi {
                    errors.push(FormError {
                        qi,
                        message: format!("{type_name} references itself"),
                        severity: Severity::Error,
                    });
                }
            }
            _ => {}
        }

        // Answer letter within option count
        match qt {
            QuestionType::CountAnswer { answer }
            | QuestionType::CountAnswerBefore { answer, .. }
            | QuestionType::CountAnswerAfter { answer, .. }
            | QuestionType::ClosestAfter { answer, .. }
            | QuestionType::ClosestBefore { answer, .. }
            | QuestionType::FirstWith { answer }
            | QuestionType::LastWith { answer }
            | QuestionType::OnlyOdd { answer }
            | QuestionType::OnlyEven { answer }
            | QuestionType::EqualCount { answer }
                if answer.idx() >= oc =>
            {
                errors.push(FormError {
                    qi,
                    message: format!(
                        "References answer {} which is outside option count {oc}",
                        answer.as_char()
                    ),
                    severity: Severity::Warning,
                });
            }
            _ => {}
        }

        // SameAs checks. "none" is a legitimate option; duplicate targets/nulls are
        // caught by the general distinct-option-values check below.
        if matches!(qt, QuestionType::SameAs) {
            for oi in 0..oc {
                let v = fp.option_nums[qi][oi];
                if v == NONE_VAL {
                    continue;
                } else if v as usize == qi {
                    errors.push(FormError {
                        qi,
                        message: format!("SameAs option {oi} references itself"),
                        severity: Severity::Error,
                    });
                } else if v < 0 || v as usize >= n {
                    errors.push(FormError {
                        qi,
                        message: format!("SameAs option {oi} references out-of-range question {v}"),
                        severity: Severity::Error,
                    });
                }
            }
        }

        // OnlySame self-references
        if matches!(qt, QuestionType::OnlySame) {
            for oi in 0..oc {
                if fp.option_nums[qi][oi] == qi as i16 {
                    errors.push(FormError {
                        qi,
                        message: format!("OnlySame option {oi} references itself"),
                        severity: Severity::Warning,
                    });
                }
            }
        }

        // EqualCount self-referencing
        if let QuestionType::EqualCount { answer } = qt {
            for oi in 0..oc {
                if fp.option_nums[qi][oi] == answer.idx() as i16 {
                    errors.push(FormError {
                        qi,
                        message: format!(
                            "EqualCount({}) option {oi} points to {} (self-referencing)",
                            answer.as_char(),
                            answer.as_char()
                        ),
                        severity: Severity::Warning,
                    });
                }
            }
        }

        // NoOtherHasAnswer: every other letter must appear in at least one other question
        if matches!(qt, QuestionType::NoOtherHasAnswer)
            && let Some(sol) = solution
        {
            let self_ans = sol[qi];
            for letter in LETTERS.iter().take(oc) {
                if *letter != self_ans
                    && !sol
                        .iter()
                        .enumerate()
                        .any(|(j, &a)| j != qi && a == *letter)
                {
                    errors.push(FormError {
                        qi,
                        message: format!(
                            "NoOtherHasAnswer: letter {} also has no other question with that answer, so the correct option is ambiguous",
                            letter.as_char()
                        ),
                        severity: Severity::Warning,
                    });
                }
            }
        }

        // Duplicate option values (incl. at most one "none"). Letter-valued slots
        // (AnswerOf/LeastCommon/MostCommon) carry NAN_VAL in option_nums and store the
        // letter in option_answers, so compare on the unified value. TrueStmt (claims)
        // and identity-option types have no comparable value and are excluded.
        if !qt.has_identity_options() && !matches!(qt, QuestionType::TrueStmt) {
            let vals: Vec<i16> = (0..oc)
                .map(|oi| {
                    let v = fp.option_nums[qi][oi];
                    if v == NAN_VAL {
                        fp.option_answers[qi][oi] as i16
                    } else {
                        v
                    }
                })
                .collect();
            let unique: std::collections::HashSet<i16> = vals.iter().copied().collect();
            if unique.len() < vals.len() {
                errors.push(FormError {
                    qi,
                    message: "Duplicate option values".into(),
                    severity: Severity::Warning,
                });
            }
        }

        // Letter-valued options stored in option_answers (AnswerOf, LeastCommon, MostCommon)
        if matches!(
            qt,
            QuestionType::AnswerOf { .. } | QuestionType::LeastCommon | QuestionType::MostCommon
        ) {
            for oi in 0..oc {
                let a = fp.option_answers[qi][oi];
                if a != 0xFF && a as usize >= oc {
                    errors.push(FormError {
                        qi,
                        message: format!(
                            "Option {oi} letter {} is outside option count {oc}",
                            (b'A' + a) as char
                        ),
                        severity: Severity::Warning,
                    });
                }
            }
        }

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
                if fp.option_nums[qi][oi] == NONE_VAL {
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

        // Option values in valid range
        for oi in 0..oc {
            let v = fp.option_nums[qi][oi];
            if v == NONE_VAL || v == NAN_VAL {
                continue;
            }
            let out_of_range = match qt {
                QuestionType::CountAnswer { .. }
                | QuestionType::CountVowel
                | QuestionType::CountConsonant
                | QuestionType::MostCommonCount => v < 0 || v > n as i16,
                QuestionType::CountAnswerBefore { before_index, .. } => {
                    v < 0 || v > *before_index as i16
                }
                QuestionType::CountAnswerAfter { after_index, .. } => {
                    v < 0 || v > (n as i16 - 1 - *after_index as i16)
                }
                QuestionType::NoOtherHasAnswer
                | QuestionType::EqualCount { .. }
                | QuestionType::AnswerIsSelf
                | QuestionType::LetterDist { .. } => !(0..oc as i16).contains(&v),
                QuestionType::FirstWith { .. }
                | QuestionType::LastWith { .. }
                | QuestionType::OnlySame
                | QuestionType::SameAs
                | QuestionType::SameAsWhich { .. } => v < 0 || v >= n as i16,
                QuestionType::NextSame => v <= qi as i16 || v >= n as i16,
                QuestionType::PrevSame => v < 0 || v >= qi as i16,
                QuestionType::ClosestAfter { after_index, .. } => {
                    v <= *after_index as i16 || v >= n as i16
                }
                QuestionType::ClosestBefore { before_index, .. } => {
                    v < 0 || v >= *before_index as i16
                }
                QuestionType::OnlyOdd { .. } => v < 0 || v >= n as i16 || v % 2 != 0,
                QuestionType::OnlyEven { .. } => v < 0 || v >= n as i16 || v % 2 != 1,
                QuestionType::ConsecIdent => v < 0 || v >= n as i16 - 1,
                _ => false,
            };
            if out_of_range {
                errors.push(FormError {
                    qi,
                    message: format!("Option {oi} value {v} out of range"),
                    severity: Severity::Warning,
                });
            }
        }

        // TrueStmt claim checks
        if matches!(qt, QuestionType::TrueStmt) {
            for oi in 0..oc {
                if let Some(claim) = &fp.option_claims[qi][oi]
                    && let QuestionType::EqualCount { answer } = &claim.question_type
                    && claim.value == answer.idx() as i16
                {
                    errors.push(FormError {
                        qi,
                        message: format!(
                            "TrueStmt option {oi} has EqualCount({}) pointing to {} (self-referencing)",
                            answer.as_char(),
                            answer.as_char()
                        ),
                        severity: Severity::Warning,
                    });
                }
            }
        }
    }

    errors
}
