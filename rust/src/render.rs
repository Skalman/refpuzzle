//! Human-readable text for questions, options, and TrueStmt claims — the prose
//! shown on the board. The single source of truth (the frontend renders the
//! board through the wasm boundary); `explain` builds on `claim_label`.

use crate::types::*;

/// A 1-based question reference for prose, e.g. `#3` for index 2. Mirrors the
/// `Q()` helper the explain layer uses pervasively.
pub fn q(index: impl Into<usize>) -> String {
    format!("#{}", index.into() + 1)
}

/// The question prompt, e.g. "How many questions have answer A?".
pub fn question_text(qt: &QuestionType) -> String {
    use QuestionType::*;
    match qt {
        CountAnswer { answer } => format!("How many questions have answer {answer}?"),
        CountAnswerBefore {
            answer,
            before_index,
        } => format!(
            "How many questions before {} have answer {answer}?",
            q(*before_index)
        ),
        CountAnswerAfter {
            answer,
            after_index,
        } => format!(
            "How many questions after {} have answer {answer}?",
            q(*after_index)
        ),
        CountVowel => "How many questions have a vowel as the answer?".into(),
        CountConsonant => "How many questions have a consonant as the answer?".into(),
        MostCommonCount => "How many times does the most common answer occur?".into(),
        ClosestAfter {
            after_index,
            answer,
        } => format!(
            "Which is the closest question after {} that has answer {answer}?",
            q(*after_index)
        ),
        ClosestBefore {
            before_index,
            answer,
        } => format!(
            "Which is the closest question before {} that has answer {answer}?",
            q(*before_index)
        ),
        FirstWith { answer } => format!("Which is the first question with answer {answer}?"),
        LastWith { answer } => format!("Which is the last question with answer {answer}?"),
        PrevSame => "Which is the previous question that has the same answer as this one?".into(),
        NextSame => "Which is the next question that has the same answer as this one?".into(),
        OnlySame => "Which is the only other question with the same answer as this one?".into(),
        SameAs => "Which of these questions has the same answer as this one?".into(),
        SameAsWhich { question_index } => format!(
            "Which of these questions has the same answer as {}?",
            q(*question_index)
        ),
        OnlyOdd { answer } => {
            format!("Which is the only odd-numbered question with answer {answer}?")
        }
        OnlyEven { answer } => {
            format!("Which is the only even-numbered question with answer {answer}?")
        }
        ConsecIdent => {
            "Which are the only two consecutive questions with identical answers?".into()
        }
        AnswerOf { question_index } => {
            format!("What is the answer to question {}?", q(*question_index))
        }
        LeastCommon => "Which is the least common answer?".into(),
        MostCommon => "Which is the most common answer?".into(),
        NoOtherHasAnswer => "Which answer is not the answer to any other question?".into(),
        EqualCount { answer } => {
            format!("Which answer appears the same number of times as {answer}?")
        }
        AnswerIsSelf => "What is the answer to this question?".into(),
        LetterDist { question_index } => format!(
            "How many letters away is the answer to this question from the answer to question {}?",
            q(*question_index)
        ),
        TrueStmt => "Which statement is the only true statement?".into(),
    }
}

/// The label for one option `value` of `qt`. `NONE`/`UNUSED` render as the
/// type's empty marker ("None", or "?" for letter-valued types); `TrueStmt`
/// rows carry claim text instead (see [`claim_label`]) so their label is empty.
pub fn option_label(qt: &QuestionType, ov: OptionValue) -> String {
    use QuestionType::*;
    let ov = ov.is_num().then(|| ov.value());
    match qt {
        // TrueStmt rows carry claim text (see `claim_label`), not a plain label.
        TrueStmt => String::new(),
        // Letter-valued: the option is itself an answer letter ("?" if unknown).
        AnswerOf { .. } | LeastCommon | MostCommon | NoOtherHasAnswer | AnswerIsSelf => {
            ov.map_or_else(|| "?".to_string(), |ov| LETTERS[ov as usize].to_string())
        }
        // The consecutive pair, e.g. "4-5".
        ConsecIdent => ov.map_or_else(|| "None".to_string(), |x| format!("{}-{}", x + 1, x + 2)),
        // A letter with a matching count.
        EqualCount { .. } => {
            ov.map_or_else(|| "None".to_string(), |x| LETTERS[x as usize].to_string())
        }
        // 1-based question position.
        ClosestAfter { .. }
        | ClosestBefore { .. }
        | FirstWith { .. }
        | LastWith { .. }
        | PrevSame
        | NextSame
        | OnlySame
        | SameAs
        | SameAsWhich { .. }
        | OnlyOdd { .. }
        | OnlyEven { .. } => ov.map_or_else(|| "None".to_string(), |x| (x + 1).to_string()),
        // Raw number (a count, or a LetterDist distance).
        CountAnswer { .. }
        | CountAnswerBefore { .. }
        | CountAnswerAfter { .. }
        | CountVowel
        | CountConsonant
        | MostCommonCount
        | LetterDist { .. } => ov.map_or_else(|| "None".to_string(), |x| x.to_string()),
    }
}

/// A TrueStmt claim rendered as its question text plus the option it asserts,
/// e.g. "How many questions have answer A? 3".
pub fn claim_label(claim: &Claim) -> String {
    format!(
        "{} {}",
        question_text(&claim.question_type),
        option_label(&claim.question_type, claim.value)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Parity fixture: every question kind's prompt must match `render.ts`
    // (src/engine/render.ts::renderQuestionText) verbatim.
    #[test]
    fn question_text_matches_ts() {
        use QuestionType::*;
        let a = Answer::A; // "A"
        let cases: &[(QuestionType, &str)] = &[
            (
                CountAnswer { answer: a },
                "How many questions have answer A?",
            ),
            (
                CountAnswerBefore {
                    answer: a,
                    before_index: 2,
                },
                "How many questions before #3 have answer A?",
            ),
            (
                CountAnswerAfter {
                    answer: a,
                    after_index: 2,
                },
                "How many questions after #3 have answer A?",
            ),
            (CountVowel, "How many questions have a vowel as the answer?"),
            (
                CountConsonant,
                "How many questions have a consonant as the answer?",
            ),
            (
                MostCommonCount,
                "How many times does the most common answer occur?",
            ),
            (
                ClosestAfter {
                    after_index: 2,
                    answer: a,
                },
                "Which is the closest question after #3 that has answer A?",
            ),
            (
                ClosestBefore {
                    before_index: 2,
                    answer: a,
                },
                "Which is the closest question before #3 that has answer A?",
            ),
            (
                FirstWith { answer: a },
                "Which is the first question with answer A?",
            ),
            (
                LastWith { answer: a },
                "Which is the last question with answer A?",
            ),
            (
                PrevSame,
                "Which is the previous question that has the same answer as this one?",
            ),
            (
                NextSame,
                "Which is the next question that has the same answer as this one?",
            ),
            (
                OnlySame,
                "Which is the only other question with the same answer as this one?",
            ),
            (
                SameAs,
                "Which of these questions has the same answer as this one?",
            ),
            (
                SameAsWhich { question_index: 2 },
                "Which of these questions has the same answer as #3?",
            ),
            (
                OnlyOdd { answer: a },
                "Which is the only odd-numbered question with answer A?",
            ),
            (
                OnlyEven { answer: a },
                "Which is the only even-numbered question with answer A?",
            ),
            (
                ConsecIdent,
                "Which are the only two consecutive questions with identical answers?",
            ),
            (
                AnswerOf { question_index: 2 },
                "What is the answer to question #3?",
            ),
            (LeastCommon, "Which is the least common answer?"),
            (MostCommon, "Which is the most common answer?"),
            (
                NoOtherHasAnswer,
                "Which answer is not the answer to any other question?",
            ),
            (
                EqualCount { answer: a },
                "Which answer appears the same number of times as A?",
            ),
            (AnswerIsSelf, "What is the answer to this question?"),
            (
                LetterDist { question_index: 2 },
                "How many letters away is the answer to this question from the answer to question #3?",
            ),
            (TrueStmt, "Which statement is the only true statement?"),
        ];
        for (qt, expected) in cases {
            assert_eq!(&question_text(qt), expected, "{qt:?}");
        }
    }

    #[test]
    fn option_label_matches_ts() {
        use QuestionType::*;
        let num = OptionValue::num;
        // Letter-valued.
        assert_eq!(option_label(&AnswerOf { question_index: 0 }, num(2)), "C");
        assert_eq!(option_label(&LeastCommon, OptionValue::NONE), "?");
        // ConsecIdent pair.
        assert_eq!(option_label(&ConsecIdent, num(3)), "4-5");
        assert_eq!(option_label(&ConsecIdent, OptionValue::NONE), "None");
        // EqualCount letter / None.
        assert_eq!(option_label(&EqualCount { answer: Answer::A }, num(1)), "B");
        assert_eq!(
            option_label(&EqualCount { answer: Answer::A }, OptionValue::NONE),
            "None"
        );
        // Positional (1-based) / None.
        assert_eq!(option_label(&FirstWith { answer: Answer::A }, num(4)), "5");
        assert_eq!(
            option_label(&FirstWith { answer: Answer::A }, OptionValue::NONE),
            "None"
        );
        // Count-valued (raw) / None.
        assert_eq!(option_label(&CountVowel, num(3)), "3");
        assert_eq!(option_label(&CountVowel, OptionValue::NONE), "None");
        // TrueStmt row has no plain label.
        assert_eq!(option_label(&TrueStmt, num(0)), "");
    }

    #[test]
    fn claim_label_joins_question_and_option() {
        let claim = Claim {
            question_type: QuestionType::CountAnswer { answer: Answer::A },
            value: OptionValue::num(3),
        };
        assert_eq!(claim_label(&claim), "How many questions have answer A? 3");
    }
}
