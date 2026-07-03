use crate::build::count_letter;
use crate::check_answerable::answerable;
use crate::rng::Rng;
use crate::types::*;

pub fn format_claim_qt(qt: &QuestionType) -> serde_json::Value {
    let type_name = match qt {
        QuestionType::CountAnswer { .. } => "CountAnswer",
        QuestionType::CountConsonant => "CountConsonant",
        QuestionType::CountVowel => "CountVowel",
        QuestionType::CountAnswerAfter { .. } => "CountAnswerAfter",
        QuestionType::CountAnswerBefore { .. } => "CountAnswerBefore",
        QuestionType::AnswerOf { .. } => "AnswerOf",
        QuestionType::FirstWith { .. } => "FirstWith",
        QuestionType::LastWith { .. } => "LastWith",
        QuestionType::MostCommon => "MostCommon",
        QuestionType::LeastCommon => "LeastCommon",
        QuestionType::MostCommonCount => "MostCommonCount",
        QuestionType::NoOtherHasAnswer => "NoOtherHasAnswer",
        QuestionType::ConsecIdent => "ConsecIdent",
        QuestionType::OnlyOdd { .. } => "OnlyOdd",
        QuestionType::OnlyEven { .. } => "OnlyEven",
        QuestionType::EqualCount { .. } => "EqualCount",
        QuestionType::ClosestAfter { .. } => "ClosestAfter",
        QuestionType::ClosestBefore { .. } => "ClosestBefore",
        QuestionType::SameAsWhich { .. } => "SameAsWhich",
        _ => "Unknown",
    };
    let mut obj = serde_json::Map::new();
    obj.insert("type".into(), serde_json::json!(type_name));
    match *qt {
        QuestionType::CountAnswer { answer }
        | QuestionType::FirstWith { answer }
        | QuestionType::LastWith { answer }
        | QuestionType::OnlyOdd { answer }
        | QuestionType::OnlyEven { answer }
        | QuestionType::EqualCount { answer } => {
            obj.insert(
                "answer".into(),
                serde_json::json!(answer.as_char().to_string()),
            );
        }
        QuestionType::CountAnswerAfter {
            answer,
            after_index,
        } => {
            obj.insert(
                "answer".into(),
                serde_json::json!(answer.as_char().to_string()),
            );
            obj.insert("afterIndex".into(), serde_json::json!(after_index));
        }
        QuestionType::CountAnswerBefore {
            answer,
            before_index,
        } => {
            obj.insert(
                "answer".into(),
                serde_json::json!(answer.as_char().to_string()),
            );
            obj.insert("beforeIndex".into(), serde_json::json!(before_index));
        }
        QuestionType::ClosestAfter {
            answer,
            after_index,
        } => {
            obj.insert(
                "answer".into(),
                serde_json::json!(answer.as_char().to_string()),
            );
            obj.insert("afterIndex".into(), serde_json::json!(after_index));
        }
        QuestionType::ClosestBefore {
            answer,
            before_index,
        } => {
            obj.insert(
                "answer".into(),
                serde_json::json!(answer.as_char().to_string()),
            );
            obj.insert("beforeIndex".into(), serde_json::json!(before_index));
        }
        QuestionType::AnswerOf { question_index }
        | QuestionType::LetterDist { question_index }
        | QuestionType::SameAsWhich { question_index } => {
            obj.insert("questionIndex".into(), serde_json::json!(question_index));
        }
        _ => {}
    }
    serde_json::Value::Object(obj)
}

fn is_constrained_type(kind: QuestionTypeKind) -> bool {
    matches!(
        kind,
        QuestionTypeKind::ConsecIdent
            | QuestionTypeKind::NoOtherHasAnswer
            | QuestionTypeKind::OnlySame
            | QuestionTypeKind::OnlyOdd
            | QuestionTypeKind::OnlyEven
    )
}

/// Checks whether the solution has the properties needed for this type at this position.
pub(crate) fn solution_fits_kind(
    kind: QuestionTypeKind,
    qi: usize,
    sol: &[Answer; MAX_N],
    n: usize,
    oc: usize,
) -> bool {
    match kind {
        // MC/LC answerability (unique extreme) lives in `check_answerable` — the one
        // build-time source, shared with v2's parametrize and `refpuzzle check`.
        QuestionTypeKind::LeastCommon => answerable(&QuestionType::LeastCommon, sol, n, oc),
        QuestionTypeKind::MostCommon => answerable(&QuestionType::MostCommon, sol, n, oc),
        QuestionTypeKind::SameAs => {
            // Pool capacity for SameAs at qi depends on how many questions share qi's answer:
            //   same_count == 1 (qi is unique): correct = null, pool = n-1 other Qs, no null.
            //   same_count >= 2: correct = a same-answer Q, pool = (n - same_count) differing-Q + 1 null.
            // We need pool >= oc - 1 (one distractor per non-correct option).
            let same_count = count_letter(sol, sol[qi], n) as usize;
            let pool = if same_count == 1 {
                n - 1
            } else {
                n - same_count + 1
            };
            pool >= oc - 1
        }
        QuestionTypeKind::SameAsWhich => true,
        QuestionTypeKind::NoOtherHasAnswer => {
            count_letter(sol, sol[qi], n) == 1
                && LETTERS[..oc]
                    .iter()
                    .all(|&l| l == sol[qi] || count_letter(sol, l, n) >= 1)
        }
        QuestionTypeKind::EqualCount => true,
        _ if is_constrained_type(kind) => solution_satisfies_type_for_kind(kind, qi, sol, n),
        _ => true,
    }
}

fn solution_satisfies_type_for_kind(
    kind: QuestionTypeKind,
    qi: usize,
    sol: &[Answer; MAX_N],
    n: usize,
) -> bool {
    match kind {
        QuestionTypeKind::ConsecIdent => {
            let mut pairs = 0;
            for i in 0..n.saturating_sub(1) {
                if sol[i] == sol[i + 1] {
                    pairs += 1;
                }
            }
            pairs <= 1
        }
        QuestionTypeKind::NoOtherHasAnswer => count_letter(sol, sol[qi], n) == 1,
        QuestionTypeKind::OnlySame => {
            let mut m = 0;
            for i in 0..n {
                if i != qi && sol[i] == sol[qi] {
                    m += 1;
                }
            }
            m <= 1
        }
        QuestionTypeKind::OnlyOdd | QuestionTypeKind::OnlyEven => {
            let parity = if kind == QuestionTypeKind::OnlyOdd {
                1
            } else {
                0
            };
            LETTERS.iter().any(|&letter| {
                let mut m = 0;
                for i in 0..n {
                    if (i + 1) % 2 == parity && sol[i] == letter {
                        m += 1;
                    }
                }
                m <= 1
            })
        }
        _ => true,
    }
}

pub(crate) fn random_type_params(
    kind: QuestionTypeKind,
    qi: usize,
    n: usize,
    option_count: usize,
    solution: &[Answer; MAX_N],
    assigned: u16,
    rng: &mut Rng,
) -> Option<QuestionType> {
    match kind {
        QuestionTypeKind::CountAnswer => Some(QuestionType::CountAnswer {
            answer: rng.pick_letter(option_count),
        }),
        QuestionTypeKind::CountAnswerBefore => {
            // Need before_index with at least oc distinct count values (0..=before_index).
            if n < option_count {
                return None;
            }
            Some(QuestionType::CountAnswerBefore {
                answer: rng.pick_letter(option_count),
                before_index: rng.int(option_count as i32 - 1, n as i32 - 1) as u8,
            })
        }
        QuestionTypeKind::CountAnswerAfter => {
            // Need after_index with at least oc distinct count values (0..=n-1-after_index).
            if n < option_count {
                return None;
            }
            Some(QuestionType::CountAnswerAfter {
                answer: rng.pick_letter(option_count),
                after_index: rng.int(0, n as i32 - option_count as i32) as u8,
            })
        }
        QuestionTypeKind::CountVowel => Some(QuestionType::CountVowel),
        QuestionTypeKind::CountConsonant => Some(QuestionType::CountConsonant),
        QuestionTypeKind::MostCommonCount => Some(QuestionType::MostCommonCount),
        QuestionTypeKind::AnswerOf => {
            let mut pool = [0u8; MAX_N];
            let mut plen = 0;
            for j in 0..n {
                if j != qi && (assigned & (1 << j)) != 0 {
                    pool[plen] = j as u8;
                    plen += 1;
                }
            }
            if plen == 0 {
                return None;
            }
            Some(QuestionType::AnswerOf {
                question_index: rng.pick(&pool[..plen]),
            })
        }
        QuestionTypeKind::LetterDist => {
            let mut pool = [0u8; MAX_N];
            let mut plen = 0;
            for j in 0..n {
                if j != qi && (assigned & (1 << j)) != 0 {
                    pool[plen] = j as u8;
                    plen += 1;
                }
            }
            if plen == 0 {
                for j in 0..n {
                    if j != qi {
                        pool[plen] = j as u8;
                        plen += 1;
                    }
                }
            }
            Some(QuestionType::LetterDist {
                question_index: rng.pick(&pool[..plen]),
            })
        }
        QuestionTypeKind::ClosestAfter => {
            // Need after_index with at least oc distinct option values
            // (positions after_index+1..n, plus null).
            if n < option_count {
                return None;
            }
            Some(QuestionType::ClosestAfter {
                after_index: rng.int(0, n as i32 - option_count as i32) as u8,
                answer: rng.pick_letter(option_count),
            })
        }
        QuestionTypeKind::ClosestBefore => {
            // Need before_index with at least oc distinct option values
            // (positions 0..before_index, plus null).
            if n < option_count {
                return None;
            }
            Some(QuestionType::ClosestBefore {
                before_index: rng.int(option_count as i32 - 1, n as i32 - 1) as u8,
                answer: rng.pick_letter(option_count),
            })
        }
        QuestionTypeKind::FirstWith => Some(QuestionType::FirstWith {
            answer: rng.pick_letter(option_count),
        }),
        QuestionTypeKind::LastWith => Some(QuestionType::LastWith {
            answer: rng.pick_letter(option_count),
        }),
        QuestionTypeKind::PrevSame => {
            // Need oc distinct option values; pool size is qi + 1 (positions [0, qi) + null).
            if qi + 1 < option_count {
                return None;
            }
            Some(QuestionType::PrevSame)
        }
        QuestionTypeKind::NextSame => {
            // Need oc distinct option values; pool size is n - qi (positions (qi, n) + null).
            if n - qi < option_count {
                return None;
            }
            Some(QuestionType::NextSame)
        }
        QuestionTypeKind::OnlySame => Some(QuestionType::OnlySame),
        QuestionTypeKind::SameAs => {
            // "none" (no other question shares this answer) is a valid answer, so there is no
            // structural requirement. Capacity: with "none" as a value, oc distinct options
            // need n >= oc (none case: oc-1 index distractors from the n-1 other questions).
            if n < option_count {
                return None;
            }
            Some(QuestionType::SameAs)
        }
        QuestionTypeKind::ConsecIdent => Some(QuestionType::ConsecIdent),
        QuestionTypeKind::OnlyOdd | QuestionTypeKind::OnlyEven => {
            let answer = rng.pick_letter(option_count);
            Some(if kind == QuestionTypeKind::OnlyOdd {
                QuestionType::OnlyOdd { answer }
            } else {
                QuestionType::OnlyEven { answer }
            })
        }
        QuestionTypeKind::LeastCommon => Some(QuestionType::LeastCommon),
        QuestionTypeKind::MostCommon => Some(QuestionType::MostCommon),
        QuestionTypeKind::NoOtherHasAnswer => Some(QuestionType::NoOtherHasAnswer),
        QuestionTypeKind::EqualCount => {
            let ref_letter = rng.pick_letter(option_count);
            let ref_count = count_letter(solution, ref_letter, n);
            let has_match = LETTERS
                .iter()
                .any(|&l| l != ref_letter && count_letter(solution, l, n) == ref_count);
            if !has_match && rng.int(0, 4) > 1 {
                return None;
            }
            Some(QuestionType::EqualCount { answer: ref_letter })
        }
        QuestionTypeKind::AnswerIsSelf => Some(QuestionType::AnswerIsSelf),
        QuestionTypeKind::TrueStmt => {
            if option_count < 5 {
                return None;
            }
            Some(QuestionType::TrueStmt)
        }
        QuestionTypeKind::SameAsWhich => {
            let mut pool = [0u8; MAX_N];
            let mut plen = 0;
            for j in 0..n {
                if j != qi && (assigned & (1 << j)) != 0 {
                    pool[plen] = j as u8;
                    plen += 1;
                }
            }
            if plen == 0 {
                return None;
            }
            let ref_qi = rng.pick(&pool[..plen]) as usize;
            if solution[ref_qi] == solution[qi] {
                return None;
            }
            // Structural: another question must share ref's answer.
            // Capacity: need at least oc-1 questions whose answer differs from ref (distractors).
            let mut has_match = false;
            let mut distractor_count = 0usize;
            for j in 0..n {
                if j == qi {
                    continue;
                }
                if solution[j] == solution[ref_qi] {
                    if j != ref_qi {
                        has_match = true;
                    }
                } else {
                    distractor_count += 1;
                }
            }
            if !has_match || distractor_count < option_count - 1 {
                return None;
            }
            Some(QuestionType::SameAsWhich {
                question_index: ref_qi as u8,
            })
        }
    }
}
