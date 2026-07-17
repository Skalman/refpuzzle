//! Human-readable text for questions, options, and TrueStmt claims — the prose
//! shown on the board. The single source of truth (the frontend renders the
//! board through the wasm boundary); `explain` builds on `claim_label`.

use crate::types::*;
use serde::Serialize;

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

/// What an L1 hint arrow should point at, resolved from the question type alone
/// (independent of the current fill). Like [`question_text`], this is the single
/// source: the frontend renders the geometry and models no question types
/// itself. `None` for the kinds L1 never uses — arrows are an L1-only aid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ArrowReferent {
    /// An option column: the `oi`-th option of every row. `boundary` clips it to
    /// the rows on one side of a question (before/after kinds).
    Column {
        oi: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        boundary: Option<Boundary>,
    },
    /// Another question's whole row.
    Question { qi: usize },
    /// A scan from question `qi`'s row toward the nearest row with answer `oi`,
    /// going up (`dir = -1`) or down (`dir = 1`).
    Scan { qi: usize, dir: i8, oi: usize },
    /// This question's own row, scanning up/down for the nearest row sharing its
    /// (as-yet-unknown) answer.
    SameRun { dir: i8 },
    /// This question's row related to a fixed set of candidate rows (the question
    /// numbers offered as its options).
    Candidates { qis: Vec<usize> },
    /// The whole grid — rendered as a per-letter tally badge, not a line.
    Tally,
}

/// A column clip: the rows above (`side = -1`) or below (`side = 1`) question
/// `qi`, which itself is drawn as the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Boundary {
    pub qi: usize,
    pub side: i8,
}

/// The arrow referent for question `qi` (see [`ArrowReferent`]), or `None` for
/// kinds outside L1's mix. Mirrors [`question_text`]'s match on the question
/// type; `SameAs` additionally reads the offered candidate options.
pub fn arrow_referent(fp: &FlatPuzzle, qi: usize) -> Option<ArrowReferent> {
    use QuestionType::*;
    Some(match fp.question_types[qi] {
        // Group A — an option column (optionally clipped by a boundary row).
        CountAnswer { answer } => ArrowReferent::Column {
            oi: answer.idx(),
            boundary: None,
        },
        CountAnswerBefore {
            answer,
            before_index,
        } => ArrowReferent::Column {
            oi: answer.idx(),
            boundary: Some(Boundary {
                qi: before_index as usize,
                side: -1,
            }),
        },
        CountAnswerAfter {
            answer,
            after_index,
        } => ArrowReferent::Column {
            oi: answer.idx(),
            boundary: Some(Boundary {
                qi: after_index as usize,
                side: 1,
            }),
        },
        // First/last differ only in reading direction (top vs bottom); both point
        // at the same column — the reading nuance is left to the visuals.
        FirstWith { answer } | LastWith { answer } => ArrowReferent::Column {
            oi: answer.idx(),
            boundary: None,
        },
        // Group B — another question's row.
        AnswerOf { question_index } => ArrowReferent::Question {
            qi: question_index as usize,
        },
        ClosestAfter {
            after_index,
            answer,
        } => ArrowReferent::Scan {
            qi: after_index as usize,
            dir: 1,
            oi: answer.idx(),
        },
        ClosestBefore {
            before_index,
            answer,
        } => ArrowReferent::Scan {
            qi: before_index as usize,
            dir: -1,
            oi: answer.idx(),
        },
        // Group C — this question's own answer.
        PrevSame => ArrowReferent::SameRun { dir: -1 },
        NextSame => ArrowReferent::SameRun { dir: 1 },
        SameAs => ArrowReferent::Candidates {
            qis: (0..fp.option_count)
                .filter_map(|oi| {
                    let ov = fp.options[qi][oi];
                    ov.is_num().then(|| ov.value() as usize)
                })
                .collect(),
        },
        // Group D — the whole grid (tally badge, not a line).
        MostCommon | LeastCommon | NoOtherHasAnswer => ArrowReferent::Tally,
        // Kinds L1 never uses — no arrow.
        _ => return None,
    })
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

    #[test]
    fn arrow_referent_maps_shapes() {
        use QuestionType::*;
        let (a, b, c) = (Answer::A, Answer::B, Answer::C);
        // One row per referent shape; qi 0 (SameAs) carries candidate options.
        let types = [
            SameAs,                    // 0
            CountAnswer { answer: a }, // 1
            CountAnswerBefore {
                answer: a,
                before_index: 5,
            }, // 2
            CountAnswerAfter {
                answer: b,
                after_index: 1,
            }, // 3
            FirstWith { answer: c },   // 4
            AnswerOf { question_index: 7 }, // 5
            ClosestAfter {
                after_index: 1,
                answer: a,
            }, // 6
            ClosestBefore {
                before_index: 9,
                answer: b,
            }, // 7
            PrevSame,                  // 8
            NextSame,                  // 9
            MostCommon,                // 10
            CountVowel,                // 11 (not an L1 kind)
        ];
        let n = types.len();
        let (affected_by, global_indices) = FlatPuzzle::build_deps(&types, n);
        let mut options = [[OptionValue::UNUSED; 5]; MAX_N];
        options[0][0] = OptionValue::num(0);
        options[0][1] = OptionValue::num(1);
        options[0][2] = OptionValue::num(2);
        let fp = FlatPuzzle {
            question_types: types,
            options,
            true_stmt_question_types: None,
            affected_by,
            global_indices,
            n,
            option_count: 3,
            initial_state: State::initial(3),
        };
        use ArrowReferent as R;
        let col = |oi, boundary| Some(R::Column { oi, boundary });
        assert_eq!(
            arrow_referent(&fp, 0),
            Some(R::Candidates { qis: vec![0, 1, 2] })
        );
        assert_eq!(arrow_referent(&fp, 1), col(0, None));
        assert_eq!(
            arrow_referent(&fp, 2),
            col(0, Some(Boundary { qi: 5, side: -1 }))
        );
        assert_eq!(
            arrow_referent(&fp, 3),
            col(1, Some(Boundary { qi: 1, side: 1 }))
        );
        assert_eq!(arrow_referent(&fp, 4), col(2, None));
        assert_eq!(arrow_referent(&fp, 5), Some(R::Question { qi: 7 }));
        assert_eq!(
            arrow_referent(&fp, 6),
            Some(R::Scan {
                qi: 1,
                dir: 1,
                oi: 0
            })
        );
        assert_eq!(
            arrow_referent(&fp, 7),
            Some(R::Scan {
                qi: 9,
                dir: -1,
                oi: 1
            })
        );
        assert_eq!(arrow_referent(&fp, 8), Some(R::SameRun { dir: -1 }));
        assert_eq!(arrow_referent(&fp, 9), Some(R::SameRun { dir: 1 }));
        assert_eq!(arrow_referent(&fp, 10), Some(R::Tally));
        assert_eq!(arrow_referent(&fp, 11), None);
    }

    #[test]
    fn arrow_referent_covers_l1_kinds() {
        use crate::construct::RECIPES;
        // Every kind L1's recipe can emit must resolve to an arrow. Probes the real
        // recipe, so coach coverage can't silently drift when the pool changes.
        let l1 = &RECIPES[0];
        let kinds = l1
            .required
            .iter()
            .map(|&(kind, _)| kind)
            .chain(l1.allowed.iter().copied());
        for kind in kinds {
            let fp = single_question_fp(sample_question(kind));
            assert!(
                arrow_referent(&fp, 0).is_some(),
                "arrow_referent has no arrow for L1 kind {kind:?}"
            );
        }
    }

    /// A representative `QuestionType` for `kind` — params don't affect whether an
    /// arrow exists. Panics for kinds outside L1's pool; extend if the recipe grows.
    fn sample_question(kind: QuestionTypeKind) -> QuestionType {
        use QuestionType as Q;
        use QuestionTypeKind as K;
        let a = Answer::A;
        match kind {
            K::CountAnswer => Q::CountAnswer { answer: a },
            K::CountAnswerBefore => Q::CountAnswerBefore {
                answer: a,
                before_index: 1,
            },
            K::CountAnswerAfter => Q::CountAnswerAfter {
                answer: a,
                after_index: 1,
            },
            K::AnswerOf => Q::AnswerOf { question_index: 1 },
            K::ClosestAfter => Q::ClosestAfter {
                after_index: 1,
                answer: a,
            },
            K::ClosestBefore => Q::ClosestBefore {
                before_index: 1,
                answer: a,
            },
            K::FirstWith => Q::FirstWith { answer: a },
            K::LastWith => Q::LastWith { answer: a },
            K::SameAs => Q::SameAs,
            K::PrevSame => Q::PrevSame,
            K::NextSame => Q::NextSame,
            K::MostCommon => Q::MostCommon,
            K::LeastCommon => Q::LeastCommon,
            K::NoOtherHasAnswer => Q::NoOtherHasAnswer,
            other => panic!("no sample QuestionType for {other:?} — extend for the L1 recipe"),
        }
    }

    fn single_question_fp(qt: QuestionType) -> FlatPuzzle {
        let (n, oc) = (3, 3);
        let mut question_types = [QuestionType::AnswerIsSelf; MAX_N];
        question_types[0] = qt;
        let (affected_by, global_indices) = FlatPuzzle::build_deps(&question_types, n);
        FlatPuzzle {
            question_types,
            options: [[OptionValue::UNUSED; 5]; MAX_N],
            true_stmt_question_types: None,
            affected_by,
            global_indices,
            n,
            option_count: oc,
            initial_state: State::initial(oc),
        }
    }
}
