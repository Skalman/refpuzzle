//! Human-readable hint prose — the single source of truth (the frontend calls
//! this through the wasm boundary). Built on the engine's own primitives
//! (`check_answer` counts, `render` text) so the wording can't drift from what
//! the solver actually computes.

use std::collections::BTreeSet;

use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

use crate::check_answer::{count_matching, count_pred, count_range};
use crate::deduce::{DeduceAction, DeduceResult, DeduceRule};
use crate::lookahead::LookaheadResult;
use crate::render::{claim_label, q};
use crate::types::*;

/// One rendered hint step: a single line, a headed block, or a "look at these
/// questions" pointer. All serialize to the wire shape the UI's `HintStep`
/// renders — `{ type: "simple", text }` or `{ type: "complex", header, lines }`
/// (a `Look` serializes as `simple` with its rendered text).
#[derive(Debug, PartialEq)]
pub enum ExplainStep {
    Simple {
        text: String,
    },
    Complex {
        header: String,
        lines: Vec<String>,
    },
    /// "Try looking at #a, #b." — keeps the referenced questions (0-based) so
    /// consumers read them structurally instead of re-parsing the rendered text.
    Look {
        qis: Vec<usize>,
    },
}

impl Serialize for ExplainStep {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        match self {
            ExplainStep::Simple { text } => {
                map.serialize_entry("type", "simple")?;
                map.serialize_entry("text", text)?;
            }
            ExplainStep::Complex { header, lines } => {
                map.serialize_entry("type", "complex")?;
                map.serialize_entry("header", header)?;
                map.serialize_entry("lines", lines)?;
            }
            ExplainStep::Look { qis } => {
                map.serialize_entry("type", "simple")?;
                map.serialize_entry("text", &render_look(qis))?;
            }
        }
        map.end()
    }
}

fn simple(text: String) -> ExplainStep {
    ExplainStep::Simple { text }
}

fn complex(header: String, lines: Vec<String>) -> ExplainStep {
    ExplainStep::Complex { header, lines }
}

/// The questions an explanation sends the solver to look at (0-based, sorted) —
/// the full set from its last `Look` step: every question the deduction reads,
/// not just where the mark lands. Empty if it has no `Look`. The L1 coach names
/// and points its arrows at these.
pub fn leading_questions(steps: &[ExplainStep]) -> Vec<usize> {
    let mut refs = steps
        .iter()
        .rev()
        .find_map(|s| match s {
            ExplainStep::Look { qis } => Some(qis.clone()),
            _ => None,
        })
        .unwrap_or_default();
    refs.sort_unstable();
    refs.dedup();
    refs
}

/// A `Look` step's rendered prose: "Try looking at #a, #b and #c.".
fn render_look(qis: &[usize]) -> String {
    let labels: Vec<String> = qis.iter().map(|&i| q(i)).collect();
    let list = match labels.as_slice() {
        [] => String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    };
    format!("Try looking at {list}.")
}

/// A `Look` step over `qis`, duplicates dropped (first-occurrence order kept).
fn try_looking(qis: &[usize]) -> ExplainStep {
    let mut unique: Vec<usize> = Vec::new();
    for &qi in qis {
        if !unique.contains(&qi) {
            unique.push(qi);
        }
    }
    ExplainStep::Look { qis: unique }
}

/// Why question `qi`'s current answer is invalid, or `None` if it isn't (or is
/// unanswered). Mirrors the TS `explainInvalid`.
pub fn explain_invalid(fp: &FlatPuzzle, state: &State, qi: usize) -> Option<String> {
    state.answers[qi]?;
    explain_invalid_detail(fp, state, qi)
}

fn explain_invalid_detail(fp: &FlatPuzzle, state: &State, qi: usize) -> Option<String> {
    use QuestionType::*;
    let a = state.answers[qi]?;
    let ai = a.idx();
    let qt = fp.question_types[qi];
    // The value the chosen option asserts (a count, a letter index, or a
    // 1-based-in-prose question index depending on the kind); NONE = "no such".
    let ov = fp.options[qi][ai];
    let n = fp.n;
    let answers = &state.answers;

    match qt {
        // Count kinds: the asserted number is already unreachable.
        CountAnswer { .. }
        | CountAnswerBefore { .. }
        | CountAnswerAfter { .. }
        | CountVowel
        | CountConsonant => count_invalid_reason(fp, state, qi, &qt, ov),

        AnswerOf { question_index } => {
            let k = question_index as usize;
            let target = answers[k]?;
            (ov.is_num() && target.idx() as u8 != ov.value()).then(|| {
                format!(
                    "{} claims {}'s answer is {}, but {} is answered {target}",
                    q(qi),
                    q(k),
                    LETTERS[ov.value() as usize],
                    q(k)
                )
            })
        }

        LetterDist { question_index } => {
            let other = answers[question_index as usize]?;
            let ov = ov.is_num().then(|| ov.value())?;
            let dist = (ai as i32 - other.idx() as i32).unsigned_abs() as u8;
            (dist != ov).then(|| {
                format!(
                    "{} claims letter distance {ov}, but {a} is {dist} letters from {}'s answer {other}",
                    q(qi),
                    q(question_index as usize)
                )
            })
        }

        NoOtherHasAnswer => (0..n).find(|&i| i != qi && answers[i] == Some(a)).map(|i| {
            format!(
                "{} claims {a} is unique, but {} already has answer {a}",
                q(qi),
                q(i)
            )
        }),

        FirstWith { answer } => forward_invalid_reason(state, qi, "first", 0, answer, ov, n),
        ClosestAfter {
            after_index,
            answer,
        } => forward_invalid_reason(
            state,
            qi,
            "closest",
            after_index as usize + 1,
            answer,
            ov,
            n,
        ),

        LastWith { answer } => backward_invalid_reason(state, qi, "last", n, answer, ov, n),
        ClosestBefore {
            before_index,
            answer,
        } => backward_invalid_reason(state, qi, "closest", before_index as usize, answer, ov, n),

        SameAs => {
            let ov = ov.is_num().then(|| ov.value() as usize)?;
            if ov >= n {
                return None;
            }
            let av = answers[ov]?;
            (av != a).then(|| {
                format!(
                    "{} claims same answer as {}, but {} is {av} and {} is {a}",
                    q(qi),
                    q(ov),
                    q(ov),
                    q(qi)
                )
            })
        }

        // No own-answer contradiction to phrase for these kinds.
        MostCommonCount
        | SameAsWhich { .. }
        | OnlySame
        | PrevSame
        | NextSame
        | OnlyOdd { .. }
        | OnlyEven { .. }
        | ConsecIdent
        | LeastCommon
        | MostCommon
        | EqualCount { .. }
        | AnswerIsSelf
        | TrueStmt => None,
    }
}

/// Count-kind invalidity: the answered count is already out of reach.
fn count_invalid_reason(
    fp: &FlatPuzzle,
    state: &State,
    qi: usize,
    qt: &QuestionType,
    ov: OptionValue,
) -> Option<String> {
    let pred = count_pred(qt)?;
    let ov = ov.is_num().then(|| ov.value())?;
    let (from, to) = count_range(qt, fp.n);
    let cr = count_matching(&state.answers, &state.eliminated, pred, from, to);
    if cr.count > ov {
        return Some(format!(
            "{} claims {ov} {}, but there are already {}",
            q(qi),
            count_rule_label(qt, ov),
            cr.count
        ));
    }
    if cr.count + cr.remaining < ov {
        return Some(format!(
            "{} claims {ov} {}, but at most {} are possible",
            q(qi),
            count_rule_label(qt, ov),
            cr.count + cr.remaining
        ));
    }
    None
}

/// First/closest-after invalidity: the pointed-at question doesn't hold the
/// answer, or an earlier one does (or, for a "none" claim, some question does).
fn forward_invalid_reason(
    state: &State,
    qi: usize,
    label: &str,
    scan_start: usize,
    answer: Answer,
    ov: OptionValue,
    n: usize,
) -> Option<String> {
    let answers = &state.answers;
    let Some(ov) = ov.is_num().then(|| ov.value() as usize) else {
        return (scan_start..n)
            .find(|&j| answers[j] == Some(answer))
            .map(|j| {
                format!(
                    "{} claims no question has answer {answer}, but {} does",
                    q(qi),
                    q(j)
                )
            });
    };
    if ov < n
        && let Some(av) = answers[ov]
        && av != answer
    {
        return Some(format!(
            "{} claims {label} {answer} is {}, but {} is answered {av}",
            q(qi),
            q(ov),
            q(ov)
        ));
    }
    (scan_start..ov)
        .find(|&j| answers[j] == Some(answer))
        .map(|j| {
            format!(
                "{} claims {label} {answer} is {}, but {} has answer {answer} and comes before {}",
                q(qi),
                q(ov),
                q(j),
                q(ov)
            )
        })
}

/// Last/closest-before invalidity: mirror of [`forward_invalid_reason`].
fn backward_invalid_reason(
    state: &State,
    qi: usize,
    label: &str,
    before_idx: usize,
    answer: Answer,
    ov: OptionValue,
    n: usize,
) -> Option<String> {
    let answers = &state.answers;
    let Some(ov) = ov.is_num().then(|| ov.value() as usize) else {
        return (0..before_idx)
            .find(|&j| answers[j] == Some(answer))
            .map(|j| {
                format!(
                    "{} claims no question has answer {answer}, but {} does",
                    q(qi),
                    q(j)
                )
            });
    };
    if ov < n
        && let Some(av) = answers[ov]
        && av != answer
    {
        return Some(format!(
            "{} claims {label} {answer} is {}, but {} is answered {av}",
            q(qi),
            q(ov),
            q(ov)
        ));
    }
    (ov + 1..before_idx)
        .rev()
        .find(|&j| answers[j] == Some(answer))
        .map(|j| {
            format!(
                "{} claims {label} {answer} is {}, but {} has answer {answer} and comes after {}",
                q(qi),
                q(ov),
                q(j),
                q(ov)
            )
        })
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

/// Why an eliminated option is impossible, plus the "other" question the reason
/// leans on (for highlighting), or `None` if this kind has no specific reason.
pub struct ElimDetail {
    pub text: String,
    pub other_qi: Option<usize>,
}

fn detail(text: String, other_qi: Option<usize>) -> Option<ElimDetail> {
    Some(ElimDetail { text, other_qi })
}

/// The option value `answer` selects at question `qi`, if numeric.
fn option_value_at(fp: &FlatPuzzle, qi: usize, answer: Answer) -> Option<u8> {
    let ov = fp.options[qi][answer.idx()];
    ov.is_num().then(|| ov.value())
}

/// Over/at-most count check shared by the count kinds (`CountAnswer`,
/// `CountAnswerBefore`, `CountAnswerAfter`, `CountVowel`, `CountConsonant`).
/// `MostCommonCount` is intentionally NOT here — it has its own max-known /
/// max-possible logic.
fn count_kind_elim_detail(
    qt: &QuestionType,
    qi: usize,
    letter: Answer,
    ov: Option<u8>,
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    n: usize,
) -> Option<ElimDetail> {
    let pred = count_pred(qt)?;
    let (from, to) = count_range(qt, n);
    let cr = count_matching(answers, eliminated, pred, from, to);
    if let Some(ov) = ov {
        if cr.count > ov {
            return detail(
                format!(
                    "{} option {letter} claims {ov} {}, but there are already {}.",
                    q(qi),
                    count_rule_label(qt, ov),
                    cr.count
                ),
                None,
            );
        }
        if cr.count + cr.remaining < ov {
            return detail(
                format!(
                    "{} option {letter} claims {ov} {}, but at most {} are possible.",
                    q(qi),
                    count_rule_label(qt, ov),
                    cr.count + cr.remaining
                ),
                None,
            );
        }
    }
    None
}

/// Positional forward reason shared by `FirstWith` (scan from 0) and
/// `ClosestAfter` (scan from just after the anchor). `label` is "first" or
/// "closest".
#[allow(clippy::too_many_arguments)]
fn forward_positional_elim_detail(
    label: &str,
    scan_start: usize,
    answer: Answer,
    qi: usize,
    letter: Answer,
    ov: Option<u8>,
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    n: usize,
) -> Option<ElimDetail> {
    match ov {
        Some(ov) => {
            let target = ov as usize;
            if target < scan_start || target >= n {
                return detail(
                    format!(
                        "{} option {letter} claims {label} {answer} is {}, but that's out of range.",
                        q(qi),
                        q(target)
                    ),
                    None,
                );
            }
            match answers[target] {
                Some(av) if av != answer => {
                    return detail(
                        format!(
                            "{} option {letter} claims {label} {answer} is {}, but {} is answered {av}.",
                            q(qi),
                            q(target),
                            q(target)
                        ),
                        Some(target),
                    );
                }
                None if is_eliminated(eliminated, target, answer.idx()) => {
                    return detail(
                        format!(
                            "{} option {letter} claims {label} {answer} is {}, but {answer} is ruled out for {}.",
                            q(qi),
                            q(target),
                            q(target)
                        ),
                        Some(target),
                    );
                }
                _ => {}
            }
            for j in scan_start..target {
                if answers[j] == Some(answer) {
                    return detail(
                        format!(
                            "{} option {letter} claims {label} {answer} is {}, but {} already has answer {answer} and comes before {}.",
                            q(qi),
                            q(target),
                            q(j),
                            q(target)
                        ),
                        Some(j),
                    );
                }
            }
            if letter == answer && qi >= scan_start && qi < target {
                return detail(
                    format!(
                        "{} option {letter} claims {label} {answer} is {}, but {} itself is before {} and would have answer {answer}. Contradiction.",
                        q(qi),
                        q(target),
                        q(qi),
                        q(target)
                    ),
                    None,
                );
            }
        }
        None => {
            for j in scan_start..n {
                if answers[j] == Some(answer) {
                    return detail(
                        format!(
                            "{} option {letter} claims no question has answer {answer}, but {} has answer {answer}.",
                            q(qi),
                            q(j)
                        ),
                        Some(j),
                    );
                }
            }
        }
    }
    None
}

/// Positional backward reason shared by `LastWith` (scan up to `n`) and
/// `ClosestBefore` (scan up to the anchor). `label` is "last" or "closest".
#[allow(clippy::too_many_arguments)]
fn backward_positional_elim_detail(
    label: &str,
    before_idx: usize,
    answer: Answer,
    qi: usize,
    letter: Answer,
    vnum: Option<u8>,
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
) -> Option<ElimDetail> {
    match vnum {
        Some(v) => {
            let target = v as usize;
            if target >= before_idx {
                return detail(
                    format!(
                        "{} option {letter} claims {label} {answer} is {}, but that's out of range.",
                        q(qi),
                        q(target)
                    ),
                    None,
                );
            }
            match answers[target] {
                Some(av) if av != answer => {
                    return detail(
                        format!(
                            "{} option {letter} claims {label} {answer} is {}, but {} is answered {av}.",
                            q(qi),
                            q(target),
                            q(target)
                        ),
                        Some(target),
                    );
                }
                None if is_eliminated(eliminated, target, answer.idx()) => {
                    return detail(
                        format!(
                            "{} option {letter} claims {label} {answer} is {}, but {answer} is ruled out for {}.",
                            q(qi),
                            q(target),
                            q(target)
                        ),
                        Some(target),
                    );
                }
                _ => {}
            }
            for j in (target + 1..before_idx).rev() {
                if answers[j] == Some(answer) {
                    return detail(
                        format!(
                            "{} option {letter} claims {label} {answer} is {}, but {} has answer {answer} and comes after {}.",
                            q(qi),
                            q(target),
                            q(j),
                            q(target)
                        ),
                        Some(j),
                    );
                }
            }
            if letter == answer && qi > target && qi < before_idx {
                return detail(
                    format!(
                        "{} option {letter} claims {label} {answer} is {}, but {} itself is after {} and would have answer {answer}. Contradiction.",
                        q(qi),
                        q(target),
                        q(qi),
                        q(target)
                    ),
                    None,
                );
            }
        }
        None => {
            for j in 0..before_idx {
                if answers[j] == Some(answer) {
                    return detail(
                        format!(
                            "{} option {letter} claims no question has answer {answer}, but {} has answer {answer}.",
                            q(qi),
                            q(j)
                        ),
                        Some(j),
                    );
                }
            }
        }
    }
    None
}

/// Parity reason shared by `OnlyOdd` (parity 1) and `OnlyEven` (parity 0).
/// `wrong_parity` labels a mispointed target's parity; `own_parity` labels this
/// question's parity for the no-match case.
#[allow(clippy::too_many_arguments)]
fn parity_elim_detail(
    parity: u8,
    wrong_parity: &str,
    own_parity: &str,
    answer: Answer,
    qi: usize,
    letter: Answer,
    vnum: Option<u8>,
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    n: usize,
) -> Option<ElimDetail> {
    match vnum {
        Some(v) => {
            let target = v as usize;
            if (target + 1) % 2 != parity as usize {
                return detail(
                    format!(
                        "{} option {letter} claims {}, but {} is {wrong_parity}-numbered.",
                        q(qi),
                        q(target),
                        q(target)
                    ),
                    None,
                );
            }
            if target < n {
                match answers[target] {
                    Some(av) if av != answer => {
                        return detail(
                            format!(
                                "{} option {letter} claims {} has answer {answer}, but {} is answered {av}.",
                                q(qi),
                                q(target),
                                q(target)
                            ),
                            Some(target),
                        );
                    }
                    None if is_eliminated(eliminated, target, answer.idx()) => {
                        return detail(
                            format!(
                                "{} option {letter} claims {} has answer {answer}, but {answer} is ruled out for {}.",
                                q(qi),
                                q(target),
                                q(target)
                            ),
                            Some(target),
                        );
                    }
                    _ => {}
                }
            }
        }
        None => {
            for i in 0..n {
                if (i + 1) % 2 == parity as usize && answers[i] == Some(answer) {
                    return detail(
                        format!(
                            "{} option {letter} claims no {own_parity}-numbered question has answer {answer}, but {} does.",
                            q(qi),
                            q(i)
                        ),
                        Some(i),
                    );
                }
            }
        }
    }
    None
}

/// Why option `oi` (value `ov`) of question `qi` is being eliminated. Mirrors
/// the TS `explainElimDetail`. Only called for options the engine has ruled out,
/// so a matching reason is expected; `None` means no phrasing for this kind.
///
/// Exhaustive over `QuestionType` on purpose: every kind can have options
/// eliminated, so a new variant must supply a reason (or join the explicit
/// no-reason arm). Mirrors the exhaustive `render::question_text`.
pub fn explain_elim_detail(
    qt: &QuestionType,
    qi: usize,
    oi: usize,
    ov: OptionValue,
    state: &State,
    n: usize,
) -> Option<ElimDetail> {
    let letter = LETTERS[oi];
    let answers = &state.answers;
    let eliminated = &state.eliminated;
    let ov = ov.is_num().then(|| ov.value());

    match qt {
        QuestionType::CountAnswer { .. }
        | QuestionType::CountAnswerBefore { .. }
        | QuestionType::CountAnswerAfter { .. }
        | QuestionType::CountVowel
        | QuestionType::CountConsonant => {
            count_kind_elim_detail(qt, qi, letter, ov, answers, eliminated, n)
        }

        QuestionType::MostCommonCount => {
            if let Some(ov) = ov {
                let mut counts = [0u8; 5];
                for i in 0..n {
                    if let Some(a) = answers[i] {
                        counts[a.idx()] += 1;
                    }
                }
                let max_known = counts.iter().copied().max().unwrap();
                if ov < max_known {
                    let s = if ov == 1 { "" } else { "s" };
                    return detail(
                        format!(
                            "{} option {letter} claims the most common answer appears {ov} time{s}, but one already appears {max_known} times.",
                            q(qi)
                        ),
                        None,
                    );
                }
                let mut max_possible = 0u8;
                for l in LETTERS {
                    let mut c = 0u8;
                    let mut r = 0u8;
                    for i in 0..n {
                        match answers[i] {
                            Some(a) if a == l => c += 1,
                            None if !is_eliminated(eliminated, i, l.idx()) => r += 1,
                            _ => {}
                        }
                    }
                    max_possible = max_possible.max(c + r);
                }
                if ov > max_possible {
                    return detail(
                        format!(
                            "{} option {letter} claims the most common answer appears {ov} times, but at most {max_possible} are possible.",
                            q(qi)
                        ),
                        None,
                    );
                }
            }
            None
        }

        QuestionType::ClosestAfter {
            after_index,
            answer,
        } => forward_positional_elim_detail(
            "closest",
            *after_index as usize + 1,
            *answer,
            qi,
            letter,
            ov,
            answers,
            eliminated,
            n,
        ),
        QuestionType::FirstWith { answer } => forward_positional_elim_detail(
            "first", 0, *answer, qi, letter, ov, answers, eliminated, n,
        ),

        QuestionType::ClosestBefore {
            before_index,
            answer,
        } => backward_positional_elim_detail(
            "closest",
            *before_index as usize,
            *answer,
            qi,
            letter,
            ov,
            answers,
            eliminated,
        ),
        QuestionType::LastWith { answer } => {
            backward_positional_elim_detail("last", n, *answer, qi, letter, ov, answers, eliminated)
        }

        QuestionType::OnlyOdd { answer } => parity_elim_detail(
            1, "even", "odd", *answer, qi, letter, ov, answers, eliminated, n,
        ),
        QuestionType::OnlyEven { answer } => parity_elim_detail(
            0, "odd", "even", *answer, qi, letter, ov, answers, eliminated, n,
        ),

        QuestionType::AnswerOf { question_index } => {
            if let Some(v) = ov
                && v < 5
                && is_eliminated(eliminated, *question_index as usize, v as usize)
            {
                let k = *question_index as usize;
                return detail(
                    format!(
                        "{} option {letter} claims {}'s answer is {}, but {} is ruled out for {}.",
                        q(qi),
                        q(k),
                        LETTERS[v as usize],
                        LETTERS[v as usize],
                        q(k)
                    ),
                    Some(k),
                );
            }
            None
        }

        QuestionType::SameAsWhich { question_index } => {
            if let Some(ref_ans) = answers[*question_index as usize]
                && let Some(v) = ov
                && (v as usize) < n
            {
                let k = *question_index as usize;
                let target = v as usize;
                match answers[target] {
                    Some(target_ans) if target_ans != ref_ans => {
                        return detail(
                            format!(
                                "{} option {letter} claims {} has the same answer as {} ({ref_ans}), but {} is answered {target_ans}.",
                                q(qi),
                                q(target),
                                q(k),
                                q(target)
                            ),
                            Some(target),
                        );
                    }
                    None if is_eliminated(eliminated, target, ref_ans.idx()) => {
                        return detail(
                            format!(
                                "{} option {letter} claims {} has the same answer as {} ({ref_ans}), but {ref_ans} is ruled out for {}.",
                                q(qi),
                                q(target),
                                q(k),
                                q(target)
                            ),
                            Some(target),
                        );
                    }
                    _ => {}
                }
            }
            None
        }

        QuestionType::LetterDist { question_index } => {
            let k = *question_index as usize;
            let max_dist = oi.max(4 - oi) as u8;
            if let Some(v) = ov
                && v > max_dist
            {
                return detail(
                    format!(
                        "{} option {letter} claims letter distance {v}, but {letter} can be at most {max_dist} letters from any answer.",
                        q(qi)
                    ),
                    None,
                );
            }
            match answers[k] {
                Some(other) => {
                    if let Some(v) = ov {
                        let dist = (oi as i32 - other.idx() as i32).unsigned_abs() as u8;
                        if dist != v {
                            return detail(
                                format!(
                                    "{} option {letter} claims letter distance {v}, but {letter} is {dist} letters from {}'s answer {other}.",
                                    q(qi),
                                    q(k)
                                ),
                                Some(k),
                            );
                        }
                    }
                }
                None => {
                    if let Some(v) = ov {
                        let any_possible = (0..5).any(|ti| {
                            !is_eliminated(eliminated, k, ti)
                                && (oi as i32 - ti as i32).unsigned_abs() as u8 == v
                        });
                        if !any_possible {
                            return detail(
                                format!(
                                    "{} option {letter} claims letter distance {v}, but no remaining answer for {} gives that distance from {letter}.",
                                    q(qi),
                                    q(k)
                                ),
                                Some(k),
                            );
                        }
                    }
                }
            }
            None
        }

        QuestionType::ConsecIdent => {
            match ov {
                Some(v) => {
                    let start = v as usize;
                    if start + 1 >= n {
                        return detail(
                            format!(
                                "{} option {letter} claims {} and {}, but that's out of range.",
                                q(qi),
                                q(start),
                                q(start + 1)
                            ),
                            None,
                        );
                    }
                    if start == qi || start + 1 == qi {
                        let partner = if start == qi { start + 1 } else { start };
                        if is_eliminated(eliminated, partner, oi) {
                            return detail(
                                format!(
                                    "{} option {letter} claims {} and {} are the consecutive pair, but {letter} is ruled out for {} so they can't match.",
                                    q(qi),
                                    q(start),
                                    q(start + 1),
                                    q(partner)
                                ),
                                Some(partner),
                            );
                        }
                    }
                    let poss_a = !eliminated[start] & ALL_OPTIONS_MASK;
                    let poss_b = !eliminated[start + 1] & ALL_OPTIONS_MASK;
                    if poss_a & poss_b == 0 {
                        return detail(
                            format!(
                                "{} option {letter} claims {} and {} are the consecutive pair, but they share no possible answer.",
                                q(qi),
                                q(start),
                                q(start + 1)
                            ),
                            Some(start),
                        );
                    }
                }
                None => {
                    for i in 0..n.saturating_sub(1) {
                        if let (Some(a), Some(b)) = (answers[i], answers[i + 1])
                            && a == b
                        {
                            return detail(
                                format!(
                                    "{} option {letter} claims no consecutive pair exists, but {} and {} both have answer {a}.",
                                    q(qi),
                                    q(i),
                                    q(i + 1)
                                ),
                                Some(i),
                            );
                        }
                    }
                }
            }
            None
        }

        QuestionType::PrevSame => {
            match ov {
                None => {
                    for j in 0..qi {
                        if answers[j] == Some(letter) {
                            return detail(
                                format!(
                                    "{} option {letter} claims no previous question has answer {letter}, but {} does.",
                                    q(qi),
                                    q(j)
                                ),
                                Some(j),
                            );
                        }
                    }
                }
                Some(v) => {
                    let target = v as usize;
                    if target >= qi {
                        return detail(
                            format!(
                                "{} option {letter} claims {}, but {} is not before {}.",
                                q(qi),
                                q(target),
                                q(target),
                                q(qi)
                            ),
                            None,
                        );
                    }
                    if is_eliminated(eliminated, target, oi) {
                        return detail(
                            format!(
                                "{} option {letter} claims {} has the same answer, but {letter} is ruled out for {}.",
                                q(qi),
                                q(target),
                                q(target)
                            ),
                            Some(target),
                        );
                    }
                    for j in (target + 1..qi).rev() {
                        if answers[j] == Some(letter) {
                            return detail(
                                format!(
                                    "{} option {letter} claims previous same answer is {}, but {} also has answer {letter} and is closer.",
                                    q(qi),
                                    q(target),
                                    q(j)
                                ),
                                Some(j),
                            );
                        }
                    }
                }
            }
            None
        }

        QuestionType::NextSame => {
            match ov {
                None => {
                    for j in (qi + 1)..n {
                        if answers[j] == Some(letter) {
                            return detail(
                                format!(
                                    "{} option {letter} claims no later question has answer {letter}, but {} does.",
                                    q(qi),
                                    q(j)
                                ),
                                Some(j),
                            );
                        }
                    }
                }
                Some(v) => {
                    let target = v as usize;
                    if target <= qi || target >= n {
                        return detail(
                            format!(
                                "{} option {letter} claims {}, but {} is not after {}.",
                                q(qi),
                                q(target),
                                q(target),
                                q(qi)
                            ),
                            None,
                        );
                    }
                    if is_eliminated(eliminated, target, oi) {
                        return detail(
                            format!(
                                "{} option {letter} claims {} has the same answer, but {letter} is ruled out for {}.",
                                q(qi),
                                q(target),
                                q(target)
                            ),
                            Some(target),
                        );
                    }
                    for j in (qi + 1)..target {
                        if answers[j] == Some(letter) {
                            return detail(
                                format!(
                                    "{} option {letter} claims next same answer is {}, but {} also has answer {letter} and is closer.",
                                    q(qi),
                                    q(target),
                                    q(j)
                                ),
                                Some(j),
                            );
                        }
                    }
                }
            }
            None
        }

        QuestionType::OnlySame => {
            match ov {
                None => {
                    for j in 0..n {
                        if j != qi && answers[j] == Some(letter) {
                            return detail(
                                format!(
                                    "{} option {letter} claims no other question has answer {letter}, but {} does.",
                                    q(qi),
                                    q(j)
                                ),
                                Some(j),
                            );
                        }
                    }
                }
                Some(v) => {
                    let target = v as usize;
                    if target == qi {
                        return detail(
                            format!(
                                "{} option {letter} points to {} itself, but a question can't share an answer with itself.",
                                q(qi),
                                q(qi)
                            ),
                            None,
                        );
                    }
                    if target < n && is_eliminated(eliminated, target, oi) {
                        return detail(
                            format!(
                                "{} option {letter} claims {} has the same answer, but {letter} is ruled out for {}.",
                                q(qi),
                                q(target),
                                q(target)
                            ),
                            Some(target),
                        );
                    }
                    if target < n && target != qi {
                        for j in 0..n {
                            if j != qi && j != target && answers[j] == Some(letter) {
                                return detail(
                                    format!(
                                        "{} option {letter} claims {} is the only other with answer {letter}, but {} already has answer {letter}.",
                                        q(qi),
                                        q(target),
                                        q(j)
                                    ),
                                    Some(j),
                                );
                            }
                        }
                    }
                }
            }
            None
        }

        QuestionType::SameAs => {
            if let Some(v) = ov {
                let target = v as usize;
                if target == qi {
                    return detail(
                        format!(
                            "{} option {letter} points to {} itself, but a question can't share an answer with itself.",
                            q(qi),
                            q(qi)
                        ),
                        None,
                    );
                }
                if target < n && is_eliminated(eliminated, target, oi) {
                    return detail(
                        format!(
                            "{} option {letter} claims {} has the same answer, but {letter} is ruled out for {}.",
                            q(qi),
                            q(target),
                            q(target)
                        ),
                        Some(target),
                    );
                }
            }
            None
        }

        QuestionType::NoOtherHasAnswer => {
            for i in 0..n {
                if answers[i] == Some(letter) {
                    return detail(
                        format!(
                            "{} option {letter} claims {letter} is unique, but {} already has answer {letter}.",
                            q(qi),
                            q(i)
                        ),
                        Some(i),
                    );
                }
            }
            None
        }

        QuestionType::EqualCount { answer } => {
            if let Some(v) = ov {
                if LETTERS[v as usize] == *answer {
                    return detail(
                        format!(
                            "{} option {letter} claims {}, but the question asks for a different letter with the same count as {answer}.",
                            q(qi),
                            LETTERS[v as usize]
                        ),
                        None,
                    );
                }
                if v < 5 {
                    let claimed = LETTERS[v as usize];
                    let (mut rc, mut rr, mut sc, mut sr) = (0u8, 0u8, 0u8, 0u8);
                    let ref_mask = 1u8 << answer.idx();
                    let claimed_mask = 1u8 << v;
                    for j in 0..n {
                        match answers[j] {
                            Some(aj) => {
                                if aj == *answer {
                                    rc += 1;
                                }
                                if aj == claimed {
                                    sc += 1;
                                }
                            }
                            None => {
                                if eliminated[j] & ref_mask == 0 {
                                    rr += 1;
                                }
                                if eliminated[j] & claimed_mask == 0 {
                                    sr += 1;
                                }
                            }
                        }
                    }
                    if rc + rr < sc {
                        return detail(
                            format!(
                                "{} option {letter} claims {claimed} has the same count as {answer}, but {answer} can have at most {} while {claimed} already has {sc}.",
                                q(qi),
                                rc + rr
                            ),
                            None,
                        );
                    }
                    if sc + sr < rc {
                        return detail(
                            format!(
                                "{} option {letter} claims {claimed} has the same count as {answer}, but {claimed} can have at most {} while {answer} already has {rc}.",
                                q(qi),
                                sc + sr
                            ),
                            None,
                        );
                    }
                }
            }
            None
        }

        QuestionType::LeastCommon => {
            if let Some(v) = ov
                && v < 5
            {
                let mut counts = [0u8; 5];
                for j in 0..n {
                    if let Some(aj) = answers[j] {
                        counts[aj.idx()] += 1;
                    }
                }
                let claimed = LETTERS[v as usize];
                let min_count = counts.iter().copied().min().unwrap();
                let min_letters: Vec<Answer> = (0..5)
                    .filter(|&i| counts[i] == min_count)
                    .map(|i| LETTERS[i])
                    .collect();
                if counts[v as usize] > min_count {
                    return detail(
                        format!(
                            "{} option {letter} claims {claimed} is the least common, but {claimed} appears {} time(s) while {} appears only {min_count}.",
                            q(qi),
                            counts[v as usize],
                            min_letters[0]
                        ),
                        None,
                    );
                }
                if min_letters.len() > 1 {
                    let joined = min_letters
                        .iter()
                        .map(|l| l.to_string())
                        .collect::<Vec<_>>()
                        .join(" and ");
                    return detail(
                        format!(
                            "{} option {letter} claims {claimed} is the least common, but {joined} are tied at {min_count} — no unique least.",
                            q(qi)
                        ),
                        None,
                    );
                }
                return detail(
                    format!(
                        "{} option {letter} claims {claimed} is the least common, but {claimed} can't be uniquely least.",
                        q(qi)
                    ),
                    None,
                );
            }
            None
        }

        QuestionType::MostCommon => {
            if let Some(v) = ov
                && v < 5
            {
                let mut counts = [0u8; 5];
                for j in 0..n {
                    if let Some(aj) = answers[j] {
                        counts[aj.idx()] += 1;
                    }
                }
                let claimed = LETTERS[v as usize];
                let max_count = counts.iter().copied().max().unwrap();
                let max_letters: Vec<Answer> = (0..5)
                    .filter(|&i| counts[i] == max_count)
                    .map(|i| LETTERS[i])
                    .collect();
                if counts[v as usize] < max_count {
                    return detail(
                        format!(
                            "{} option {letter} claims {claimed} is the most common, but {claimed} appears {} time(s) while {} appears {max_count}.",
                            q(qi),
                            counts[v as usize],
                            max_letters[0]
                        ),
                        None,
                    );
                }
                if max_letters.len() > 1 {
                    let joined = max_letters
                        .iter()
                        .map(|l| l.to_string())
                        .collect::<Vec<_>>()
                        .join(" and ");
                    return detail(
                        format!(
                            "{} option {letter} claims {claimed} is the most common, but {joined} are tied at {max_count} — no unique most.",
                            q(qi)
                        ),
                        None,
                    );
                }
                return detail(
                    format!(
                        "{} option {letter} claims {claimed} is the most common, but {claimed} can't be uniquely most.",
                        q(qi)
                    ),
                    None,
                );
            }
            None
        }

        // These kinds assert nothing that can rule an option out on its own.
        QuestionType::AnswerIsSelf | QuestionType::TrueStmt => None,
    }
}

/// A short "because …" clause for why question `qi` is forced to `letter`, or an
/// empty string if none fits. Mirrors the TS `briefForceReason`.
pub fn brief_force_reason(fp: &FlatPuzzle, state: &State, qi: usize, letter: Answer) -> String {
    let answers = &state.answers;
    let n = fp.n;

    if let QuestionType::AnswerOf { question_index } = fp.question_types[qi]
        && let Some(target) = answers[question_index as usize]
    {
        return format!("{} is {target}", q(question_index));
    }

    for other in 0..n {
        let Some(other_ans) = answers[other] else {
            continue;
        };
        let points_here = option_value_at(fp, other, other_ans) == Some(qi as u8);
        match fp.question_types[other] {
            QuestionType::AnswerOf { question_index } if question_index as usize == qi => {
                return format!("{} is {other_ans}, which implies {letter}", q(other));
            }
            QuestionType::SameAs if points_here => {
                return format!("same answer as {}", q(other));
            }
            QuestionType::PrevSame | QuestionType::NextSame | QuestionType::OnlySame
                if points_here =>
            {
                return format!("{} is {other_ans}, same answer as {}", q(other), q(qi));
            }
            _ => {}
        }
    }

    if (!state.eliminated[qi] & ALL_OPTIONS_MASK).count_ones() == 1 {
        return "only option left".to_string();
    }

    String::new()
}

/// The plain question that asks exactly what `claim` claims (same kind and
/// parameters), other than `exclude_qi`. Mirrors `findClaimMatchQuestion`.
/// (`QuestionType` equality already means "same proposition".)
fn find_claim_match_question(fp: &FlatPuzzle, exclude_qi: usize, claim: &Claim) -> Option<usize> {
    (0..fp.n).find(|&k| k != exclude_qi && fp.question_types[k] == claim.question_type)
}

/// An answered TrueStmt whose selected statement matches `qi`'s proposition.
/// Mirrors `findTrueStmtClaimMatching`.
fn find_true_stmt_claim_matching(
    fp: &FlatPuzzle,
    state: &State,
    qi: usize,
) -> Option<(usize, Claim)> {
    (0..fp.n).find_map(|t| {
        if !matches!(fp.question_types[t], QuestionType::TrueStmt) {
            return None;
        }
        let ans = state.answers[t]?;
        let claim = fp.claim_at(t, ans.idx())?;
        (fp.question_types[qi] == claim.question_type).then_some((t, claim))
    })
}

/// A sibling count question one short of its target, leaving `qi` as the only
/// slot that can still be `target_letter`. Mirrors `findCountSatSource`.
fn find_count_sat_source(fp: &FlatPuzzle, state: &State, target_letter: Answer) -> Option<usize> {
    let n = fp.n;
    for src in 0..n {
        let Some(ans) = state.answers[src] else {
            continue;
        };
        let qt = fp.question_types[src];
        let Some(pred) = count_pred(&qt) else {
            continue;
        };
        if !pred.matches(target_letter) {
            continue;
        }
        let Some(value) = option_value_at(fp, src, ans) else {
            continue;
        };
        let (from, to) = count_range(&qt, n);
        let cr = count_matching(&state.answers, &state.eliminated, pred, from, to);
        if cr.count + cr.remaining == value && cr.remaining > 0 {
            return Some(src);
        }
    }
    None
}

/// The narrated steps for a forced answer: `qi` must be `letter` (via `rule`).
/// Mirrors the TS `explainForce`.
pub fn explain_force(
    fp: &FlatPuzzle,
    state: &State,
    qi: usize,
    letter: Answer,
    rule: DeduceRule,
) -> Vec<ExplainStep> {
    let answers = &state.answers;
    let n = fp.n;
    let qt = fp.question_types[qi];
    let mut steps = vec![simple(format!("Try looking at {}.", q(qi)))];

    if (!state.eliminated[qi] & ALL_OPTIONS_MASK).count_ones() == 1 {
        steps.push(simple(format!(
            "{} has only one option left — it must be {letter}.",
            q(qi)
        )));
        return steps;
    }

    if let QuestionType::AnswerOf { question_index } = qt
        && let Some(target) = answers[question_index as usize]
    {
        let k = question_index as usize;
        steps.push(try_looking(&[qi, k]));
        steps.push(simple(format!(
            "{} asks for {}'s answer. {} is {target}, so {} must be {letter}.",
            q(qi),
            q(k),
            q(k),
            q(qi)
        )));
        return steps;
    }

    // Forward from an answered SameAs / Prev|Next|OnlySame that points at `qi`.
    for other in 0..n {
        let Some(other_ans) = answers[other] else {
            continue;
        };
        if option_value_at(fp, other, other_ans) != Some(qi as u8) {
            continue;
        }
        match fp.question_types[other] {
            QuestionType::SameAs => {
                steps.push(try_looking(&[qi, other]));
                steps.push(simple(format!(
                    "{} says it has the same answer as {}. {} is {other_ans}, so {} must be {other_ans}.",
                    q(other),
                    q(qi),
                    q(other),
                    q(qi)
                )));
                return steps;
            }
            QuestionType::PrevSame | QuestionType::NextSame | QuestionType::OnlySame => {
                steps.push(try_looking(&[qi, other]));
                steps.push(simple(format!(
                    "{} is {other_ans}, pointing to {} as having the same answer. So {} must be {other_ans}.",
                    q(other),
                    q(qi),
                    q(qi)
                )));
                return steps;
            }
            _ => {}
        }
    }

    // SameAsWhich reverse: an answered SameAsWhich propagates the equality.
    for other in 0..n {
        let Some(other_ans) = answers[other] else {
            continue;
        };
        if let QuestionType::SameAsWhich { question_index } = fp.question_types[other]
            && let Some(target_q) = option_value_at(fp, other, other_ans)
        {
            let ref_q = question_index as usize;
            let target_q = target_q as usize;
            if target_q < n {
                if target_q == qi
                    && let Some(ref_ans) = answers[ref_q]
                {
                    steps.push(try_looking(&[qi, other]));
                    steps.push(simple(format!(
                        "{} is {other_ans}, pointing to {} as having the same answer as {} ({ref_ans}). So {} must be {letter}.",
                        q(other), q(qi), q(ref_q), q(qi)
                    )));
                    return steps;
                }
                if ref_q == qi
                    && let Some(target_ans) = answers[target_q]
                {
                    steps.push(try_looking(&[qi, other]));
                    steps.push(simple(format!(
                        "{} is {other_ans}, pointing to {} as having the same answer as {}. {} is {target_ans}, so {} must be {letter}.",
                        q(other), q(target_q), q(qi), q(target_q), q(qi)
                    )));
                    return steps;
                }
            }
        }
    }

    // Reverse AnswerOf: another question asks for `qi`'s answer.
    for other in 0..n {
        let Some(other_ans) = answers[other] else {
            continue;
        };
        if let QuestionType::AnswerOf { question_index } = fp.question_types[other]
            && question_index as usize == qi
        {
            steps.push(try_looking(&[qi, other]));
            steps.push(simple(format!(
                "{} asks for {}'s answer. {} is {other_ans}, telling us {} must be {letter}.",
                q(other),
                q(qi),
                q(other),
                q(qi)
            )));
            return steps;
        }
    }

    if let QuestionType::LetterDist { question_index } = qt
        && let Some(target) = answers[question_index as usize]
    {
        steps.push(try_looking(&[qi, question_index as usize]));
        steps.push(simple(format!(
            "{} is answered {target}. Only option {letter} gives the right letter distance.",
            q(question_index as usize)
        )));
        return steps;
    }

    // Reverse LetterDist: another question's distance constrains `qi`.
    for src in 0..n {
        if src == qi {
            continue;
        }
        if let QuestionType::LetterDist { question_index } = fp.question_types[src]
            && question_index as usize == qi
            && let Some(src_ans) = answers[src]
            && let Some(dist) = option_value_at(fp, src, src_ans)
        {
            steps.push(try_looking(&[qi, src]));
            steps.push(simple(format!(
                "{} is answered {src_ans} with letter distance {dist}. Only {letter} is at distance {dist} from {src_ans}, so {} must be {letter}.",
                q(src), q(qi)
            )));
            return steps;
        }
    }

    // Counting: everything in range is answered, so the count is fixed.
    if let Some(pred) = count_pred(&qt) {
        let (from, to) = count_range(&qt, n);
        let cr = count_matching(answers, &state.eliminated, pred, from, to);
        if cr.remaining == 0 {
            steps.push(simple(format!(
                "There are {} {}, so {} must be {letter}.",
                cr.count,
                count_rule_label(&qt, cr.count),
                q(qi)
            )));
            return steps;
        }
    }

    if matches!(rule, DeduceRule::CountMustMatchForce)
        && let Some(src) = find_count_sat_source(fp, state, letter)
    {
        let src_qt = fp.question_types[src];
        let src_ans = answers[src].expect("find_count_sat_source only returns answered sources");
        let src_val = option_value_at(fp, src, src_ans).expect("answered count option is numeric");
        let (from, to) = count_range(&src_qt, n);
        let cr = count_matching(
            answers,
            &state.eliminated,
            count_pred(&src_qt).expect("count source has a predicate"),
            from,
            to,
        );
        steps.push(try_looking(&[qi, src]));
        steps.push(simple(format!(
            "{} says there are {src_val} {}. Only {} found so far, and {} is the only remaining question that could be {letter} — so {} must be {letter}.",
            q(src), count_rule_label(&src_qt, src_val), cr.count, q(qi), q(qi)
        )));
        return steps;
    }

    if matches!(rule, DeduceRule::LeastCommonForce) && matches!(qt, QuestionType::LeastCommon) {
        steps.push(simple(format!(
            "Only one answer can make its claimed letter the least common — {} must be {letter}.",
            q(qi)
        )));
        return steps;
    }

    if matches!(rule, DeduceRule::MostCommonForce) && matches!(qt, QuestionType::MostCommon) {
        steps.push(simple(format!(
            "Only one answer can make its claimed letter the most common — {} must be {letter}.",
            q(qi)
        )));
        return steps;
    }

    if matches!(
        rule,
        DeduceRule::ConsecIdentForwardForce | DeduceRule::ConsecIdentForwardBothForce
    ) {
        for src in 0..n {
            if !matches!(fp.question_types[src], QuestionType::ConsecIdent) {
                continue;
            }
            let Some(src_ans) = answers[src] else {
                continue;
            };
            let Some(start) = option_value_at(fp, src, src_ans) else {
                continue;
            };
            let p = start as usize;
            if p == qi || p + 1 == qi {
                let partner = if p == qi { p + 1 } else { p };
                steps.push(try_looking(&[qi, src]));
                if let Some(partner_ans) = answers[partner] {
                    steps.push(simple(format!(
                        "{} says {} and {} have the same answer. {} is {partner_ans}, so {} must be {letter}.",
                        q(src), q(p), q(p + 1), q(partner), q(qi)
                    )));
                } else {
                    steps.push(simple(format!(
                        "{} says {} and {} have the same answer. Only {letter} is possible for both, so {} must be {letter}.",
                        q(src), q(p), q(p + 1), q(qi)
                    )));
                }
                return steps;
            }
        }
    }

    if matches!(rule, DeduceRule::TrueStatementForward) {
        for src in 0..n {
            let Some(src_ans) = answers[src] else {
                continue;
            };
            if !matches!(fp.question_types[src], QuestionType::TrueStmt) {
                continue;
            }
            let Some(claim) = fp.claim_at(src, src_ans.idx()) else {
                continue;
            };
            match claim.question_type {
                QuestionType::AnswerOf { question_index } if question_index as usize == qi => {
                    steps.push(try_looking(&[qi, src]));
                    steps.push(simple(format!(
                        "{}'s true statement says {}'s answer is {letter}. So {} must be {letter}.",
                        q(src),
                        q(qi),
                        q(qi)
                    )));
                    return steps;
                }
                QuestionType::FirstWith { .. } | QuestionType::LastWith { .. }
                    if claim.value.is_num() && claim.value.value() as usize == qi =>
                {
                    steps.push(try_looking(&[qi, src]));
                    steps.push(simple(format!(
                        "{}'s true statement says {} has answer {letter}. So {} must be {letter}.",
                        q(src),
                        q(qi),
                        q(qi)
                    )));
                    return steps;
                }
                _ => {}
            }
        }
    }

    if matches!(rule, DeduceRule::TrueStatementClaimValid) {
        return vec![
            simple(format!("Try looking at {}.", q(qi))),
            simple(format!(
                "Only one of {}'s claims is still possible, so it must be the answer.",
                q(qi)
            )),
        ];
    }

    if matches!(rule, DeduceRule::TrueStatementClaimKnownTrue) {
        return vec![
            simple(format!("Try looking at {}.", q(qi))),
            simple(format!(
                "Option {letter}'s claim is already known to be true, so it must be the answer."
            )),
        ];
    }

    if matches!(rule, DeduceRule::TrueStatementMatchForce) {
        if let Some(self_claim) = fp.claim_at(qi, letter.idx()) {
            // `qi` is the TrueStmt: a matching question settled its statement true.
            let k = find_claim_match_question(fp, qi, &self_claim);
            return vec![
                simple(format!("Try looking at {}.", k.map_or_else(|| q(qi), q))),
                simple(match k {
                    Some(k) => format!(
                        "{} settles \"{}\", making that statement true — so {} must be {letter}.",
                        q(k),
                        claim_label(&self_claim),
                        q(qi)
                    ),
                    None => format!(
                        "Its statement is already settled as true, so {} must be {letter}.",
                        q(qi)
                    ),
                }),
            ];
        }
        // `qi` is a plain question that a chosen true statement points at.
        let matched = find_true_stmt_claim_matching(fp, state, qi);
        return vec![
            simple(format!(
                "Try looking at {}.",
                matched.as_ref().map_or_else(|| q(qi), |(t, _)| q(*t))
            )),
            simple(match &matched {
                Some((t, claim)) => format!(
                    "{}'s true statement is \"{}\", so {} must be {letter}.",
                    q(*t),
                    claim_label(claim),
                    q(qi)
                ),
                None => format!("The true statement forces {} to be {letter}.", q(qi)),
            }),
        ];
    }

    // No branch matched — a rule-wiring bug. Loud in debug, graceful in release.
    debug_assert!(
        false,
        "explain_force: no explanation for {} = {letter:?} (rule {rule:?})",
        qi + 1
    );
    steps.push(simple(format!("{} must be {letter}.", q(qi))));
    steps
}

/// Highest lower bound on the count of `letter_index` implied by a sibling
/// CountAnswer/Before/After question — `(src_qi, floor)`. Mirrors
/// `findCountFloorSource` (and `CountBounds::floor` in deduce.rs).
fn find_count_floor_source(
    fp: &FlatPuzzle,
    state: &State,
    letter_index: usize,
) -> Option<(usize, u8)> {
    let target = LETTERS[letter_index];
    let mut best: Option<(usize, u8)> = None;
    for src in 0..fp.n {
        if !matches!(
            fp.question_types[src],
            QuestionType::CountAnswer { answer }
                | QuestionType::CountAnswerBefore { answer, .. }
                | QuestionType::CountAnswerAfter { answer, .. }
            if answer == target
        ) {
            continue;
        }
        // Smallest still-possible option value = lower bound on this count.
        let lo = match state.answers[src] {
            Some(ans) => option_value_at(fp, src, ans),
            None => (0..fp.option_count)
                .filter(|&oi| !state.is_eliminated(src, oi) && fp.options[src][oi].is_num())
                .map(|oi| fp.options[src][oi].value())
                .min(),
        };
        if let Some(lo) = lo
            && best.is_none_or(|(_, floor)| lo > floor)
        {
            best = Some((src, lo));
        }
    }
    best
}

/// Lowest upper bound on the count of `letter_index` from a sibling full-range
/// CountAnswer — `(src_qi, ceil)`. Mirrors `findCountCeilSource` (Before/After
/// bound only a sub-range, so they can't cap the total).
fn find_count_ceil_source(
    fp: &FlatPuzzle,
    state: &State,
    letter_index: usize,
) -> Option<(usize, u8)> {
    let target = LETTERS[letter_index];
    let mut best: Option<(usize, u8)> = None;
    for src in 0..fp.n {
        if !matches!(fp.question_types[src], QuestionType::CountAnswer { answer } if answer == target)
        {
            continue;
        }
        let hi = match state.answers[src] {
            Some(ans) => option_value_at(fp, src, ans),
            None => (0..fp.option_count)
                .filter(|&oi| !state.is_eliminated(src, oi) && fp.options[src][oi].is_num())
                .map(|oi| fp.options[src][oi].value())
                .max(),
        };
        if let Some(hi) = hi
            && best.is_none_or(|(_, ceil)| hi < ceil)
        {
            best = Some((src, hi));
        }
    }
    best
}

/// A positional question (first/last/closest/prev-next-same) whose range rules
/// `letter` out at `qi` — `(src_qi, prose)`. Mirrors `findPositionalRangeSource`.
fn find_positional_range_source(
    fp: &FlatPuzzle,
    state: &State,
    qi: usize,
    oi: usize,
) -> Option<(usize, String)> {
    let n = fp.n;
    let letter = LETTERS[oi];
    let answers = &state.answers;
    for src in 0..n {
        if src == qi {
            continue;
        }
        let src_qt = fp.question_types[src];
        // Forward positional: first/closest-after `letter` sits at or past `qi`.
        if matches!(src_qt, QuestionType::FirstWith { answer } | QuestionType::ClosestAfter { answer, .. } if answer == letter)
        {
            let label = if matches!(src_qt, QuestionType::FirstWith { .. }) {
                "first"
            } else {
                "closest"
            };
            match answers[src] {
                Some(src_ans) => {
                    if let Some(v) = option_value_at(fp, src, src_ans)
                        && qi < v as usize
                    {
                        return Some((
                            src,
                            format!(
                                "{} says {label} {letter} is {}, so {} can't be {letter}.",
                                q(src),
                                q(v as usize),
                                q(qi)
                            ),
                        ));
                    }
                }
                None => {
                    let mut min_pos = n;
                    for si in 0..5 {
                        if state.is_eliminated(src, si) {
                            continue;
                        }
                        let ov = fp.options[src][si];
                        if ov.is_num() && (ov.value() as usize) < min_pos {
                            min_pos = ov.value() as usize;
                        }
                    }
                    return Some((
                        src,
                        format!(
                            "{}'s remaining options for {label} {letter} are all at {} or later, so earlier questions can't be {letter}.",
                            q(src),
                            q(min_pos)
                        ),
                    ));
                }
            }
        }
        // Backward positional: last/closest-before `letter` sits at or before `qi`.
        if matches!(src_qt, QuestionType::LastWith { answer } | QuestionType::ClosestBefore { answer, .. } if answer == letter)
        {
            let label = if matches!(src_qt, QuestionType::LastWith { .. }) {
                "last"
            } else {
                "closest"
            };
            match answers[src] {
                Some(src_ans) => {
                    if let Some(v) = option_value_at(fp, src, src_ans)
                        && qi > v as usize
                    {
                        return Some((
                            src,
                            format!(
                                "{} says {label} {letter} is {}, so {} can't be {letter}.",
                                q(src),
                                q(v as usize),
                                q(qi)
                            ),
                        ));
                    }
                }
                None => {
                    let mut max_pos: i32 = -1;
                    for si in 0..5 {
                        if state.is_eliminated(src, si) {
                            continue;
                        }
                        let ov = fp.options[src][si];
                        if ov.is_num() && (ov.value() as i32) > max_pos {
                            max_pos = ov.value() as i32;
                        }
                    }
                    return Some((
                        src,
                        format!(
                            "{}'s remaining options for {label} {letter} are all at {} or earlier, so later questions can't be {letter}.",
                            q(src),
                            q(max_pos.max(0) as usize)
                        ),
                    ));
                }
            }
        }
        if matches!(src_qt, QuestionType::NextSame)
            && answers[src] == Some(letter)
            && let Some(v) = option_value_at(fp, src, letter)
            && qi > src
            && qi < v as usize
        {
            return Some((
                src,
                format!(
                    "{} is {letter} and says next same answer is {}, so {} can't be {letter}.",
                    q(src),
                    q(v as usize),
                    q(qi)
                ),
            ));
        }
        if matches!(src_qt, QuestionType::PrevSame)
            && answers[src] == Some(letter)
            && let Some(v) = option_value_at(fp, src, letter)
            && qi > v as usize
            && qi < src
        {
            return Some((
                src,
                format!(
                    "{} is {letter} and says previous same answer is {}, so {} can't be {letter}.",
                    q(src),
                    q(v as usize),
                    q(qi)
                ),
            ));
        }
    }
    None
}

/// A sibling count question already at its stated count (so `qi` can't add
/// another match), or one short (so `qi` must match) — `(src_qi, prose)`.
/// Mirrors `explainCountSaturation`.
fn explain_count_saturation(
    fp: &FlatPuzzle,
    state: &State,
    qi: usize,
    oi: usize,
) -> Option<(usize, String)> {
    let n = fp.n;
    let letter = LETTERS[oi];
    for src in 0..n {
        let Some(ans) = state.answers[src] else {
            continue;
        };
        let src_qt = fp.question_types[src];
        let Some(pred) = count_pred(&src_qt) else {
            continue;
        };
        let Some(value) = option_value_at(fp, src, ans) else {
            continue;
        };
        let (from, to) = count_range(&src_qt, n);
        let cr = count_matching(&state.answers, &state.eliminated, pred, from, to);
        if cr.count == value && pred.matches(letter) {
            return Some((
                src,
                format!(
                    "{} says there are {value} {}. There are already {value}, so {} can't also be {letter}.",
                    q(src),
                    count_rule_label(&src_qt, value),
                    q(qi)
                ),
            ));
        }
        if cr.count + cr.remaining == value && cr.remaining > 0 && !pred.matches(letter) {
            return Some((
                src,
                format!(
                    "{} says there are {value} {}. Only {} found so far, and all remaining unknowns must match — so {} can't be {letter}.",
                    q(src),
                    count_rule_label(&src_qt, value),
                    cr.count,
                    q(qi)
                ),
            ));
        }
    }
    None
}

/// The narrated steps for eliminating option `oi` of question `qi` (via `rule`).
/// Mirrors the TS `explainElimination`.
pub fn explain_elimination(
    fp: &FlatPuzzle,
    state: &State,
    qi: usize,
    oi: usize,
    rule: DeduceRule,
) -> Vec<ExplainStep> {
    let letter = LETTERS[oi];
    let qt = fp.question_types[qi];
    let ov = fp.options[qi][oi];
    let n = fp.n;
    let answers = &state.answers;
    let mut steps = vec![simple(format!("Try looking at {}.", q(qi)))];
    let what_if = || simple(format!("What if {} is {letter}?", q(qi)));

    if matches!(
        rule,
        DeduceRule::CountSaturated | DeduceRule::CountMustMatchElim
    ) {
        if let Some((src_qi, text)) = explain_count_saturation(fp, state, qi, oi) {
            steps.push(try_looking(&[qi, src_qi]));
            steps.push(what_if());
            steps.push(simple(text));
        } else {
            steps.push(what_if());
            steps.push(simple(format!("{} can't be {letter}.", q(qi))));
        }
        return steps;
    }

    if matches!(
        rule,
        DeduceRule::PositionalRangeAnswered | DeduceRule::PositionalRangeUnanswered
    ) {
        if let Some((src_qi, text)) = find_positional_range_source(fp, state, qi, oi) {
            steps.push(try_looking(&[src_qi, qi]));
            steps.push(simple(text));
        } else {
            steps.push(simple(format!("{} can't be {letter}.", q(qi))));
        }
        return steps;
    }

    if matches!(rule, DeduceRule::TrueStatementClaimInvalid) {
        if let Some(claim) = fp.claim_at(qi, oi) {
            match claim.question_type {
                QuestionType::FirstWith { answer } | QuestionType::LastWith { answer }
                    if claim.value.is_num()
                        && (claim.value.value() as usize) < n
                        && answers[claim.value.value() as usize].is_some() =>
                {
                    let target = claim.value.value() as usize;
                    let target_ans = answers[target].unwrap();
                    steps.push(try_looking(&[qi, target]));
                    steps.push(what_if());
                    steps.push(simple(format!(
                        "{} option {letter}'s statement says {} has answer {answer}, but {} is {target_ans}.",
                        q(qi), q(target), q(target)
                    )));
                    return steps;
                }
                QuestionType::AnswerOf { question_index }
                    if (question_index as usize) < n
                        && answers[question_index as usize].is_some() =>
                {
                    let k = question_index as usize;
                    let k_ans = answers[k].unwrap();
                    steps.push(try_looking(&[qi, k]));
                    steps.push(what_if());
                    steps.push(simple(format!(
                        "{} option {letter}'s statement says {}'s answer is {}, but {} is {k_ans}.",
                        q(qi),
                        q(k),
                        LETTERS[claim.value.value() as usize],
                        q(k)
                    )));
                    return steps;
                }
                _ => {}
            }
        }
        steps.push(what_if());
        steps.push(simple(format!(
            "{} option {letter}'s statement is contradicted by the current answers.",
            q(qi)
        )));
        return steps;
    }

    if matches!(rule, DeduceRule::TrueStatementSelfRef) {
        if let Some(claim) = fp.claim_at(qi, oi) {
            match claim.question_type {
                QuestionType::FirstWith { answer } | QuestionType::LastWith { answer }
                    if claim.value.is_num() && claim.value.value() as usize == qi =>
                {
                    steps.push(what_if());
                    steps.push(simple(format!(
                        "{} option {letter}'s statement says {} has answer {answer}, but that contradicts {} being {letter}.",
                        q(qi), q(qi), q(qi)
                    )));
                    return steps;
                }
                QuestionType::AnswerOf { question_index } if question_index as usize == qi => {
                    steps.push(what_if());
                    steps.push(simple(format!(
                        "{} option {letter}'s statement says {}'s answer is {}, but that contradicts {} being {letter}.",
                        q(qi), q(qi), LETTERS[claim.value.value() as usize], q(qi)
                    )));
                    return steps;
                }
                _ => {}
            }
        }
        steps.push(what_if());
        steps.push(simple(format!(
            "{} option {letter}'s statement contradicts itself.",
            q(qi)
        )));
        return steps;
    }

    if matches!(rule, DeduceRule::OnlySameNoneForward) {
        for src in 0..n {
            if !matches!(fp.question_types[src], QuestionType::OnlySame) {
                continue;
            }
            let Some(src_ans) = answers[src] else {
                continue;
            };
            // Only the "none" option (no numeric value) claims uniqueness.
            if option_value_at(fp, src, src_ans).is_some() {
                continue;
            }
            if src_ans == letter {
                steps.push(try_looking(&[qi, src]));
                steps.push(what_if());
                steps.push(simple(format!(
                    "{} is {letter} and claims no other question shares that answer, so {} can't be {letter}.",
                    q(src), q(qi)
                )));
                return steps;
            }
        }
    }

    if matches!(rule, DeduceRule::ConsecIdentForwardElim) {
        for src in 0..n {
            if !matches!(fp.question_types[src], QuestionType::ConsecIdent) {
                continue;
            }
            let Some(src_ans) = answers[src] else {
                continue;
            };
            let Some(start) = option_value_at(fp, src, src_ans) else {
                continue;
            };
            let p = start as usize;
            if p == qi || p + 1 == qi {
                let partner = if p == qi { p + 1 } else { p };
                steps.push(try_looking(&[qi, partner, src]));
                steps.push(what_if());
                steps.push(simple(format!(
                    "{} says {} and {} must have the same answer, but {letter} is ruled out for {}.",
                    q(src), q(p), q(p + 1), q(partner)
                )));
                return steps;
            }
        }
    }

    if matches!(rule, DeduceRule::ConsecIdentReverse) {
        for src in 0..n {
            if !matches!(fp.question_types[src], QuestionType::ConsecIdent) {
                continue;
            }
            let neighbor = if qi > 0 && answers[qi - 1] == Some(letter) {
                Some(qi - 1)
            } else if qi + 1 < n && answers[qi + 1] == Some(letter) {
                Some(qi + 1)
            } else {
                None
            };
            if let Some(neighbor) = neighbor {
                steps.push(try_looking(&[qi, neighbor, src]));
                steps.push(what_if());
                steps.push(simple(format!(
                    "{} and {} would both be {letter}, creating a consecutive pair — but {}'s remaining options don't allow that pair.",
                    q(qi), q(neighbor), q(src)
                )));
            } else {
                steps.push(what_if());
                steps.push(simple(format!(
                    "That would create a consecutive pair not allowed by {}'s remaining options.",
                    q(src)
                )));
            }
            return steps;
        }
    }

    if matches!(
        rule,
        DeduceRule::VowelCrossElim | DeduceRule::ConsonantCrossElim
    ) {
        steps.push(what_if());
        steps.push(simple(format!(
            "{} option {letter}: no compatible option exists on the other counting rule.",
            q(qi)
        )));
        return steps;
    }

    if matches!(rule, DeduceRule::LeastCommonCountFloor) && ov.is_num() && ov.value() < 5 {
        let claimed = LETTERS[ov.value() as usize];
        if let Some((src_qi, floor)) = find_count_floor_source(fp, state, ov.value() as usize) {
            let src_qt = fp.question_types[src_qi];
            steps.push(try_looking(&[qi, src_qi]));
            steps.push(what_if());
            steps.push(simple(format!(
                "{} means there are at least {floor} {}, so {claimed} appears too often to be the least common.",
                q(src_qi), count_rule_label(&src_qt, floor)
            )));
            return steps;
        }
        // No count-question floor: fall through to the cell-based explanation.
    }

    if matches!(rule, DeduceRule::MostCommonCountCeil) && ov.is_num() && ov.value() < 5 {
        let claimed = LETTERS[ov.value() as usize];
        if let Some((src_qi, ceil)) = find_count_ceil_source(fp, state, ov.value() as usize) {
            let src_qt = fp.question_types[src_qi];
            steps.push(try_looking(&[qi, src_qi]));
            steps.push(what_if());
            steps.push(simple(format!(
                "{} means there are at most {ceil} {}, so {claimed} appears too rarely to be the most common.",
                q(src_qi), count_rule_label(&src_qt, ceil)
            )));
            return steps;
        }
        // No count-question ceiling: fall through to the cell-based explanation.
    }

    if matches!(rule, DeduceRule::TrueStatementMatchElim) {
        let claim = fp.claim_at(qi, oi);
        let k = claim.and_then(|c| find_claim_match_question(fp, qi, &c));
        if let Some(k) = k {
            steps.push(try_looking(&[qi, k]));
        }
        steps.push(what_if());
        steps.push(simple(match (claim, k) {
            (Some(c), Some(k)) => format!(
                "{}'s option {letter} is the statement \"{}\", but {} rules that out — so it can't be the true statement.",
                q(qi), claim_label(&c), q(k)
            ),
            _ => format!(
                "{} option {letter}'s statement can't be true, so it's not the answer.",
                q(qi)
            ),
        }));
        return steps;
    }

    // Generic fallback: the per-type elimination reason.
    let detail = explain_elim_detail(&qt, qi, oi, ov, state, n);
    if let Some(other) = detail.as_ref().and_then(|d| d.other_qi) {
        steps.push(try_looking(&[qi, other]));
    }
    steps.push(what_if());
    match detail {
        Some(d) => steps.push(simple(d.text)),
        None => {
            debug_assert!(
                false,
                "no explain_elim_detail for {} option {letter:?} (rule {rule:?})",
                qi + 1
            );
            steps.push(simple(format!("{} can't be {letter}.", q(qi))));
        }
    }
    steps
}

/// Why a set of questions can all drop the masked options — `(prose, other_qi)`
/// for a highlight. Mirrors the TS `explainMultiElim`.
fn explain_multi_elim(
    fp: &FlatPuzzle,
    state: &State,
    qi: usize,
    option_mask: u8,
    rule: DeduceRule,
) -> (String, Option<usize>) {
    let n = fp.n;
    let answers = &state.answers;

    if matches!(rule, DeduceRule::SameAsNegative) {
        for src in 0..n {
            if src == qi || !matches!(fp.question_types[src], QuestionType::SameAs) {
                continue;
            }
            let Some(src_ans) = answers[src] else {
                continue;
            };
            // Mirror the deduce guard: a "none" answer triggers no negative inference.
            if option_value_at(fp, src, src_ans).is_none() {
                continue;
            }
            return (
                format!(
                    "{} identifies which question shares its answer, so the other listed questions cannot have the same answer.",
                    q(src)
                ),
                Some(src),
            );
        }
    }

    if matches!(rule, DeduceRule::LetterDistReverseElim) {
        for src in 0..n {
            if src == qi {
                continue;
            }
            if let QuestionType::LetterDist { question_index } = fp.question_types[src]
                && question_index as usize == qi
            {
                if let Some(src_ans) = answers[src] {
                    let dist = option_value_at(fp, src, src_ans).unwrap_or(0);
                    return (
                        format!(
                            "{} is answered {src_ans} with letter distance {dist}, so only answers at distance {dist} from {src_ans} are possible.",
                            q(src)
                        ),
                        Some(src),
                    );
                }
                return (
                    format!(
                        "{}'s remaining options limit which answers are possible for {}.",
                        q(src),
                        q(qi)
                    ),
                    Some(src),
                );
            }
        }
    }

    if matches!(rule, DeduceRule::OnlyOddEvenRangeElim) {
        for src in 0..n {
            if src == qi {
                continue;
            }
            let (parity, answer) = match fp.question_types[src] {
                QuestionType::OnlyOdd { answer } => ("odd", answer),
                QuestionType::OnlyEven { answer } => ("even", answer),
                _ => continue,
            };
            return (
                format!(
                    "{} asks for the only {parity}-numbered question with answer {answer}, limiting which {parity} questions can have that answer.",
                    q(src)
                ),
                Some(src),
            );
        }
    }

    if matches!(
        rule,
        DeduceRule::CountSaturated | DeduceRule::CountMustMatchElim
    ) {
        let sample_oi = (0..5).find(|&b| (option_mask >> b) & 1 == 1).unwrap_or(0);
        if let Some((src_qi, text)) = explain_count_saturation(fp, state, qi, sample_oi) {
            return (text, Some(src_qi));
        }
    }

    if matches!(
        rule,
        DeduceRule::CountExceeded | DeduceRule::CountImpossible
    ) {
        let qt = fp.question_types[qi];
        if let Some(pred) = count_pred(&qt) {
            let (from, to) = count_range(&qt, n);
            let cr = count_matching(answers, &state.eliminated, pred, from, to);
            if matches!(rule, DeduceRule::CountExceeded) {
                return (
                    format!(
                        "{} claims a count below what's already found ({} {}).",
                        q(qi),
                        cr.count,
                        count_rule_label(&qt, cr.count)
                    ),
                    None,
                );
            }
            return (
                format!(
                    "{} claims a count above what's possible (at most {} {}).",
                    q(qi),
                    cr.count + cr.remaining,
                    count_rule_label(&qt, cr.count + cr.remaining)
                ),
                None,
            );
        }
    }

    debug_assert!(
        false,
        "no explain_multi_elim handler for {rule:?} at {}",
        qi + 1
    );
    (format!("{} can't be those options.", q(qi)), None)
}

/// Turn one `DeduceResult` into narrated hint steps. Mirrors `explainDeduce`.
pub fn explain_deduce(fp: &FlatPuzzle, state: &State, result: &DeduceResult) -> Vec<ExplainStep> {
    let n = fp.n;
    match result.action {
        DeduceAction::Force { qi, answer } => explain_force(fp, state, qi, answer, result.rule),
        DeduceAction::Eliminate { qi, oi } => explain_elimination(fp, state, qi, oi, result.rule),
        DeduceAction::EliminateMulti {
            question_mask,
            option_mask,
        } => {
            let qis: Vec<usize> = (0..n).filter(|&i| (question_mask >> i) & 1 == 1).collect();
            let opt_str = (0..5)
                .filter(|&b| (option_mask >> b) & 1 == 1)
                .map(|b| LETTERS[b].to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let q_list = qis.iter().map(|&i| q(i)).collect::<Vec<_>>().join(", ");

            if matches!(
                result.rule,
                DeduceRule::PositionalRangeAnswered | DeduceRule::PositionalRangeUnanswered
            ) {
                let oi = if (option_mask.count_ones()) == 1 {
                    option_mask.trailing_zeros() as usize
                } else {
                    0
                };
                if let Some((src_qi, text)) = find_positional_range_source(fp, state, qis[0], oi) {
                    vec![
                        try_looking(&[src_qi]),
                        try_looking(&sorted_qs(src_qi, &qis)),
                        simple(format!("{q_list} can't be {opt_str}: {text}")),
                    ]
                } else {
                    vec![simple(format!("{q_list} can't be {opt_str}."))]
                }
            } else {
                let (text, other_qi) =
                    explain_multi_elim(fp, state, qis[0], option_mask, result.rule);
                let mut steps = Vec::new();
                if let Some(other) = other_qi {
                    steps.push(try_looking(&[other]));
                    steps.push(try_looking(&sorted_qs(other, &qis)));
                } else {
                    steps.push(try_looking(&qis));
                }
                steps.push(simple(format!("{q_list} can't be {opt_str}: {text}")));
                steps
            }
        }
    }
}

/// `{extra} ∪ qis`, sorted and deduplicated (0-based).
fn sorted_qs(extra: usize, qis: &[usize]) -> Vec<usize> {
    let mut all = vec![extra];
    all.extend_from_slice(qis);
    all.sort_unstable();
    all.dedup();
    all
}

/// Narrate a refuted lookahead assumption: replay the chain, then the surfaced
/// contradiction. Mirrors the TS `explainLookahead`.
pub fn explain_lookahead(
    fp: &FlatPuzzle,
    state: &State,
    result: &LookaheadResult,
) -> Vec<ExplainStep> {
    let qi = result.assumption_qi;
    let letter = result.assumption_answer;
    let n = fp.n;

    let mut hyp = *state;
    hyp.answers[qi] = Some(letter);
    hyp.eliminated[qi] = ALL_OPTIONS_MASK ^ (1 << letter.idx());

    let mut involved: BTreeSet<usize> = BTreeSet::from([qi]);
    let mut lines: Vec<String> = Vec::new();

    for dr in &result.chain {
        match dr.action {
            DeduceAction::Force { qi: fqi, answer } => {
                involved.insert(fqi);
                let reason = brief_force_reason(fp, &hyp, fqi, answer);
                lines.push(if reason.is_empty() {
                    format!("{} must be {answer}.", q(fqi))
                } else {
                    format!("{} must be {answer} ({reason}).", q(fqi))
                });
                hyp.eliminated[fqi] = ALL_OPTIONS_MASK ^ (1 << answer.idx());
                hyp.answers[fqi] = Some(answer);
            }
            DeduceAction::EliminateMulti {
                question_mask,
                option_mask,
            } => {
                let qis: Vec<usize> = (0..n).filter(|&i| (question_mask >> i) & 1 == 1).collect();
                involved.extend(qis.iter().copied());
                let opt_str = (0..5)
                    .filter(|&b| (option_mask >> b) & 1 == 1)
                    .map(|b| LETTERS[b].to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                let q_list = qis.iter().map(|&i| q(i)).collect::<Vec<_>>().join(", ");
                lines.push(format!("Eliminate {opt_str} from {q_list}."));
                for &i in &qis {
                    hyp.eliminated[i] |= option_mask;
                }
            }
            DeduceAction::Eliminate { qi: eqi, oi } => {
                involved.insert(eqi);
                let mut handled = false;
                if matches!(
                    dr.rule,
                    DeduceRule::CountSaturated | DeduceRule::CountMustMatchElim
                ) && let Some((src_qi, text)) = explain_count_saturation(fp, &hyp, eqi, oi)
                {
                    involved.insert(src_qi);
                    lines.push(format!(
                        "Eliminate {} option {}: {text}",
                        q(eqi),
                        LETTERS[oi]
                    ));
                    handled = true;
                }
                if !handled {
                    let ov = fp.options[eqi][oi];
                    match explain_elim_detail(&fp.question_types[eqi], eqi, oi, ov, &hyp, n) {
                        Some(d) => {
                            lines.push(format!(
                                "Eliminate {} option {}: {}",
                                q(eqi),
                                LETTERS[oi],
                                d.text
                            ));
                        }
                        None => {
                            debug_assert!(
                                false,
                                "no explain for ELIM {} option {} (rule {:?})",
                                eqi + 1,
                                LETTERS[oi],
                                dr.rule
                            );
                            lines.push(format!("Eliminate {} option {}.", q(eqi), LETTERS[oi]));
                        }
                    }
                }
                hyp.eliminated[eqi] |= 1 << oi;
            }
        }
    }

    let contradiction_qi = result.contradiction_qi;
    involved.insert(contradiction_qi);
    let detail = match explain_invalid_detail(fp, &hyp, contradiction_qi) {
        Some(reason) => reason.replace(" claims ", " would say "),
        None => format!("{} would be invalid", q(contradiction_qi)),
    };
    lines.push(format!("But {detail}. Contradiction."));
    lines.push(format!("So {} can't be {letter}.", q(qi)));

    let mut steps = vec![simple(format!("Try looking at {}.", q(qi)))];
    if involved.len() > 1 {
        let q_list = involved
            .iter()
            .map(|&i| q(i))
            .collect::<Vec<_>>()
            .join(", ");
        steps.push(simple(format!("Try looking at {q_list}.")));
    }
    steps.push(simple(format!("What if {} is {letter}?", q(qi))));
    steps.push(complex(format!("What if {} is {letter}?", q(qi)), lines));
    steps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::parse_puzzle;
    use arrayvec::ArrayVec;
    use serde_json::json;

    #[test]
    fn leading_questions_reads_last_look_step() {
        // The last Look step wins (names the full set), sorted; non-Look ignored.
        assert_eq!(
            leading_questions(&[
                try_looking(&[1]),
                try_looking(&[1, 0]),
                simple("#1 can't be A.".into()),
            ]),
            vec![0, 1]
        );
        // Single Look step.
        assert_eq!(leading_questions(&[try_looking(&[2])]), vec![2]);
        // No Look step → empty (the coach falls back to the action's target).
        assert_eq!(
            leading_questions(&[simple("#3 must be A.".into())]),
            Vec::<usize>::new()
        );
        assert_eq!(leading_questions(&[]), Vec::<usize>::new());
    }

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

    #[test]
    fn brief_force_reason_answer_of() {
        let fp = parse_puzzle(&json!({
            "q": [{"t": "AnswerOf", "q": 1}, {"t": "AnswerIsSelf"}],
            "o": [[0, 1, 2], [0, 1, 2]],
        }))
        .unwrap();
        let state = state_with(&fp, &[None, Some(Answer::B)]);
        assert_eq!(brief_force_reason(&fp, &state, 0, Answer::B), "#2 is B");
    }

    #[test]
    fn elim_first_with_earlier_match() {
        let fp = parse_puzzle(&json!({
            "q": [{"t": "FirstWith", "a": 0}, {"t": "AnswerIsSelf"}, {"t": "AnswerIsSelf"}],
            "o": [[0, 1, 2], [0, 1, 2], [0, 1, 2]],
        }))
        .unwrap();
        // Option C claims first A is #3, but #2 already has A and comes before it.
        let state = state_with(&fp, &[None, Some(Answer::A), None]);
        let d = explain_elim_detail(&fp.question_types[0], 0, 2, OptionValue::num(2), &state, 3)
            .unwrap();
        assert_eq!(
            d.text,
            "#1 option C claims first A is #3, but #2 already has answer A and comes before #3."
        );
        assert_eq!(d.other_qi, Some(1));
    }

    #[test]
    fn elim_least_common_not_least() {
        let fp = parse_puzzle(&json!({
            "q": [{"t": "LeastCommon"}, {"t": "AnswerIsSelf"}, {"t": "AnswerIsSelf"}],
            "o": [[0, 1, 2], [0, 1, 2], [0, 1, 2]],
        }))
        .unwrap();
        // Q2 and Q3 answered A ⇒ A appears twice; option A can't be least common.
        let state = state_with(&fp, &[None, Some(Answer::A), Some(Answer::A)]);
        let d = explain_elim_detail(&fp.question_types[0], 0, 0, OptionValue::num(0), &state, 3)
            .unwrap();
        assert_eq!(
            d.text,
            "#1 option A claims A is the least common, but A appears 2 time(s) while B appears only 0."
        );
        assert_eq!(d.other_qi, None);
    }

    #[test]
    fn force_only_option_left() {
        let fp = parse_puzzle(&json!({
            "q": [{"t": "AnswerIsSelf"}],
            "o": [[0, 1, 2]],
        }))
        .unwrap();
        // Strike B and C, leaving only A. (Rule is irrelevant — this branch is
        // structural, checked before any rule-specific reasoning.)
        let state = State {
            answers: [None; MAX_N],
            eliminated: [0b11110; MAX_N],
        };
        let steps = explain_force(&fp, &state, 0, Answer::A, DeduceRule::CountMustMatchForce);
        assert_eq!(
            steps,
            vec![
                simple("Try looking at #1.".into()),
                simple("#1 has only one option left — it must be A.".into()),
            ]
        );
    }

    #[test]
    fn force_answer_of_target_answered() {
        let fp = parse_puzzle(&json!({
            "q": [{"t": "AnswerOf", "q": 1}, {"t": "AnswerIsSelf"}],
            "o": [[0, 1, 2], [0, 1, 2]],
        }))
        .unwrap();
        // Q2 is B, so the AnswerOf question #1 must be B (rule irrelevant here too).
        let state = state_with(&fp, &[None, Some(Answer::B)]);
        let steps = explain_force(&fp, &state, 0, Answer::B, DeduceRule::CountMustMatchForce);
        assert_eq!(
            steps,
            vec![
                simple("Try looking at #1.".into()),
                try_looking(&[0, 1]),
                simple("#1 asks for #2's answer. #2 is B, so #1 must be B.".into()),
            ]
        );
    }

    #[test]
    fn elimination_vowel_cross() {
        let fp = parse_puzzle(&json!({
            "q": [{"t": "CountVowel"}, {"t": "AnswerIsSelf"}],
            "o": [[0, 1, 2], [0, 1, 2]],
        }))
        .unwrap();
        let state = state_with(&fp, &[None, None]);
        let steps = explain_elimination(&fp, &state, 0, 1, DeduceRule::VowelCrossElim);
        assert_eq!(
            steps,
            vec![
                simple("Try looking at #1.".into()),
                simple("What if #1 is B?".into()),
                simple(
                    "#1 option B: no compatible option exists on the other counting rule.".into()
                ),
            ]
        );
    }

    #[test]
    fn elimination_generic_falls_through_to_elim_detail() {
        let fp = parse_puzzle(&json!({
            "q": [{"t": "FirstWith", "a": 0}, {"t": "AnswerIsSelf"}, {"t": "AnswerIsSelf"}],
            "o": [[0, 1, 2], [0, 1, 2], [0, 1, 2]],
        }))
        .unwrap();
        // Option C (claims first A is #3) with #2 already A ⇒ earlier match. A
        // non-special elim rule routes through explain_elim_detail; other_qi = 1
        // adds the second "Try looking" step.
        let state = state_with(&fp, &[None, Some(Answer::A), None]);
        let steps =
            explain_elimination(&fp, &state, 0, 2, DeduceRule::FirstClosestAfterEarlierMatch);
        assert_eq!(
            steps,
            vec![
                simple("Try looking at #1.".into()),
                try_looking(&[0, 1]),
                simple("What if #1 is C?".into()),
                simple(
                    "#1 option C claims first A is #3, but #2 already has answer A and comes before #3."
                        .into()
                ),
            ]
        );
    }

    #[test]
    fn lookahead_narrates_the_contradiction() {
        let fp = parse_puzzle(&json!({
            "q": [{"t": "CountAnswer", "a": 0}, {"t": "AnswerIsSelf"}, {"t": "AnswerIsSelf"}],
            "o": [[0, 1, 2], [0, 1, 2], [0, 1, 2]],
        }))
        .unwrap();
        // Q2 and Q3 are A. Assuming Q1 = A (option 0, "0 answers are A") is
        // self-refuting: there'd be three A's. Empty chain — the contradiction is
        // immediate at Q1.
        let state = state_with(&fp, &[None, Some(Answer::A), Some(Answer::A)]);
        let result = LookaheadResult {
            eliminate_qi: 0,
            eliminate_oi: 0,
            assumption_qi: 0,
            assumption_answer: Answer::A,
            chain: ArrayVec::new(),
            contradiction_qi: 0,
        };
        let steps = explain_lookahead(&fp, &state, &result);
        assert_eq!(
            steps,
            vec![
                simple("Try looking at #1.".into()),
                simple("What if #1 is A?".into()),
                complex(
                    "What if #1 is A?".into(),
                    vec![
                        "But #1 would say 0 questions with answer A, but there are already 3. Contradiction."
                            .into(),
                        "So #1 can't be A.".into(),
                    ]
                ),
            ]
        );
    }
}
