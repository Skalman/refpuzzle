//! Human-readable hint prose. Single source of truth (the TS `explain.ts` is a
//! faithful mirror being retired). Built on the engine's own primitives
//! (`check_answer` counts, `render` text) so the wording can't drift from what
//! the solver actually computes.

use crate::check_answer::{count_matching, count_pred, count_range};
use crate::render::q;
use crate::types::*;

/// Why question `qi`'s current answer is invalid, or `None` if it isn't (or is
/// unanswered). Mirrors the TS `explainInvalid`.
pub fn explain_invalid(fp: &FlatPuzzle, state: &State, qi: usize) -> Option<String> {
    state.answers[qi]?;
    explain_invalid_detail(fp, state, qi)
}

fn explain_invalid_detail(fp: &FlatPuzzle, state: &State, qi: usize) -> Option<String> {
    let a = state.answers[qi]?;
    let ai = a.idx();
    let qt = &fp.question_types[qi];
    // The value the chosen option asserts (a count, a letter index, or a
    // 1-based-in-prose question index depending on the kind); NONE = "no such".
    let value = fp.options[qi][ai];
    let n = fp.n;
    let answers = &state.answers;

    // Count kinds: the asserted number is already unreachable.
    if let Some(pred) = count_pred(qt) {
        let (from, to) = count_range(qt, n);
        let cr = count_matching(answers, &state.eliminated, pred, from, to);
        if value.is_num() {
            let v = value.value();
            if cr.count > v {
                return Some(format!(
                    "{} claims {v} {}, but there are already {}",
                    q(qi),
                    count_rule_label(qt, v),
                    cr.count
                ));
            }
            if cr.count + cr.remaining < v {
                return Some(format!(
                    "{} claims {v} {}, but at most {} are possible",
                    q(qi),
                    count_rule_label(qt, v),
                    cr.count + cr.remaining
                ));
            }
        }
    }

    match qt {
        QuestionType::AnswerOf { question_index } => {
            let k = *question_index as usize;
            if let Some(target) = answers[k]
                && value.is_num()
                && target.idx() as u8 != value.value()
            {
                return Some(format!(
                    "{} claims {}'s answer is {}, but {} is answered {target}",
                    q(qi),
                    q(k),
                    LETTERS[value.value() as usize],
                    q(k)
                ));
            }
        }
        QuestionType::LetterDist { question_index } => {
            let k = *question_index as usize;
            if let Some(other) = answers[k]
                && value.is_num()
            {
                let v = value.value();
                let dist = (ai as i32 - other.idx() as i32).unsigned_abs() as u8;
                if dist != v {
                    return Some(format!(
                        "{} claims letter distance {v}, but {a} is {dist} letters from {}'s answer {other}",
                        q(qi),
                        q(k)
                    ));
                }
            }
        }
        QuestionType::NoOtherHasAnswer => {
            for i in 0..n {
                if i != qi && answers[i] == Some(a) {
                    return Some(format!(
                        "{} claims {a} is unique, but {} already has answer {a}",
                        q(qi),
                        q(i)
                    ));
                }
            }
        }
        _ => {}
    }

    // Positional forward: "first"/"closest-after" points at a question that
    // doesn't hold the answer, or skips an earlier one that does.
    let forward = match qt {
        QuestionType::FirstWith { answer } => Some(("first", 0usize, *answer)),
        QuestionType::ClosestAfter {
            after_index,
            answer,
        } => Some(("closest", *after_index as usize + 1, *answer)),
        _ => None,
    };
    if let Some((label, scan_start, answer)) = forward {
        if value.is_num() {
            let v = value.value() as usize;
            if v < n
                && let Some(av) = answers[v]
                && av != answer
            {
                return Some(format!(
                    "{} claims {label} {answer} is {}, but {} is answered {av}",
                    q(qi),
                    q(v),
                    q(v)
                ));
            }
            for j in scan_start..v {
                if answers[j] == Some(answer) {
                    return Some(format!(
                        "{} claims {label} {answer} is {}, but {} has answer {answer} and comes before {}",
                        q(qi),
                        q(v),
                        q(j),
                        q(v)
                    ));
                }
            }
        } else {
            for j in scan_start..n {
                if answers[j] == Some(answer) {
                    return Some(format!(
                        "{} claims no question has answer {answer}, but {} does",
                        q(qi),
                        q(j)
                    ));
                }
            }
        }
    }

    // Positional backward: mirror of forward for "last"/"closest-before".
    let backward = match qt {
        QuestionType::LastWith { answer } => Some(("last", n, *answer)),
        QuestionType::ClosestBefore {
            before_index,
            answer,
        } => Some(("closest", *before_index as usize, *answer)),
        _ => None,
    };
    if let Some((label, before_idx, answer)) = backward {
        if value.is_num() {
            let v = value.value() as usize;
            if v < n
                && let Some(av) = answers[v]
                && av != answer
            {
                return Some(format!(
                    "{} claims {label} {answer} is {}, but {} is answered {av}",
                    q(qi),
                    q(v),
                    q(v)
                ));
            }
            for j in (v + 1..before_idx).rev() {
                if answers[j] == Some(answer) {
                    return Some(format!(
                        "{} claims {label} {answer} is {}, but {} has answer {answer} and comes after {}",
                        q(qi),
                        q(v),
                        q(j),
                        q(v)
                    ));
                }
            }
        } else {
            for j in 0..before_idx {
                if answers[j] == Some(answer) {
                    return Some(format!(
                        "{} claims no question has answer {answer}, but {} does",
                        q(qi),
                        q(j)
                    ));
                }
            }
        }
    }

    if matches!(qt, QuestionType::SameAs) && value.is_num() {
        let v = value.value() as usize;
        if v < n
            && let Some(av) = answers[v]
            && av != a
        {
            return Some(format!(
                "{} claims same answer as {}, but {} is {av} and {} is {a}",
                q(qi),
                q(v),
                q(v),
                q(qi)
            ));
        }
    }

    None
}

/// The pluralized noun phrase for a count claim, e.g. "questions with answer A"
/// or "question before #3 with answer B". Mirrors the TS `countRuleLabel`.
fn count_rule_label(qt: &QuestionType, count: u8) -> String {
    let qs = if count == 1 { "question" } else { "questions" };
    match qt {
        QuestionType::CountAnswer { answer } => format!("{qs} with answer {answer}"),
        QuestionType::CountAnswerBefore {
            answer,
            before_index,
        } => format!("{qs} before {} with answer {answer}", q(*before_index)),
        QuestionType::CountAnswerAfter {
            answer,
            after_index,
        } => format!("{qs} after {} with answer {answer}", q(*after_index)),
        QuestionType::CountVowel => format!("{qs} with a vowel answer"),
        QuestionType::CountConsonant => format!("{qs} with a consonant answer"),
        _ => format!("matching {qs}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::parse_puzzle;
    use serde_json::json;

    fn state_with(fp: &FlatPuzzle, answers: &[Option<Answer>]) -> State {
        let mut a = [None; MAX_N];
        a[..answers.len()].copy_from_slice(answers);
        State {
            answers: a,
            eliminated: fp.initial_state.eliminated,
        }
    }

    #[test]
    fn answer_of_mismatch() {
        let fp = parse_puzzle(&json!({
            "q": [{"t": "AnswerOf", "q": 1}, {"t": "AnswerIsSelf"}],
            "o": [[0, 1, 2], [0, 1, 2]],
        }))
        .unwrap();
        // Q1 answered A asserts "Q2's answer is A", but Q2 is answered B.
        let state = state_with(&fp, &[Some(Answer::A), Some(Answer::B)]);
        assert_eq!(
            explain_invalid(&fp, &state, 0).as_deref(),
            Some("#1 claims #2's answer is A, but #2 is answered B")
        );
    }

    #[test]
    fn count_over_claims() {
        // Q1 = "how many have answer A?"; option 0 claims 0. Answering it while
        // Q2 and Q3 are A makes at least 2 — already too many.
        let fp = parse_puzzle(&json!({
            "q": [{"t": "CountAnswer", "a": 0}, {"t": "AnswerIsSelf"}, {"t": "AnswerIsSelf"}],
            "o": [[0, 1, 2], [0, 1, 2], [0, 1, 2]],
        }))
        .unwrap();
        let state = state_with(&fp, &[Some(Answer::A), Some(Answer::A), Some(Answer::A)]);
        assert_eq!(
            explain_invalid(&fp, &state, 0).as_deref(),
            Some("#1 claims 0 questions with answer A, but there are already 3")
        );
    }

    #[test]
    fn consistent_answer_is_not_invalid() {
        let fp = parse_puzzle(&json!({
            "q": [{"t": "AnswerOf", "q": 1}, {"t": "AnswerIsSelf"}],
            "o": [[0, 1, 2], [0, 1, 2]],
        }))
        .unwrap();
        // Q1 answered B asserts "Q2's answer is B", and Q2 is B — consistent.
        let state = state_with(&fp, &[Some(Answer::B), Some(Answer::B)]);
        assert_eq!(explain_invalid(&fp, &state, 0), None);
    }
}
