//! Human-readable hint prose. Single source of truth (the TS `explain.ts` is a
//! faithful mirror being retired). Built on the engine's own primitives
//! (`check_answer` counts, `render` text) so the wording can't drift from what
//! the solver actually computes.

use crate::check_answer::{count_matching, count_pred, count_range};
use crate::deduce::DeduceRule;
use crate::render::{claim_label, q};
use crate::types::*;

/// One rendered hint step: a single line, or a headed block of lines. Mirrors
/// the TS `ExplainStep`.
#[derive(Debug, PartialEq)]
pub enum ExplainStep {
    Simple { text: String },
    Complex { header: String, lines: Vec<String> },
}

fn simple(text: String) -> ExplainStep {
    ExplainStep::Simple { text }
}

#[allow(dead_code)] // used by explainElimination in a later increment
fn complex(header: String, lines: Vec<String>) -> ExplainStep {
    ExplainStep::Complex { header, lines }
}

/// "Try looking at #a and #b." with duplicate indices dropped (first-occurrence
/// order kept). Mirrors the TS `tryLooking`.
fn try_looking(qis: &[usize]) -> ExplainStep {
    let mut unique: Vec<usize> = Vec::new();
    for &qi in qis {
        if !unique.contains(&qi) {
            unique.push(qi);
        }
    }
    let joined = unique
        .iter()
        .map(|&i| q(i))
        .collect::<Vec<_>>()
        .join(" and ");
    simple(format!("Try looking at {joined}."))
}

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

/// Why an eliminated option is impossible, plus the "other" question the reason
/// leans on (for highlighting), or `None` if this kind has no specific reason.
pub struct ElimDetail {
    pub text: String,
    pub other_qi: Option<usize>,
}

fn detail(text: String, other_qi: Option<usize>) -> Option<ElimDetail> {
    Some(ElimDetail { text, other_qi })
}

fn is_elim(eliminated: &[u8; MAX_N], qi: usize, oi: usize) -> bool {
    (eliminated[qi] >> oi) & 1 == 1
}

/// The option value `answer` selects at question `qi`, if numeric.
fn option_value_at(fp: &FlatPuzzle, qi: usize, answer: Answer) -> Option<u8> {
    let v = fp.options[qi][answer.idx()];
    v.is_num().then(|| v.value())
}

/// Why option `oi` (value `value`) of question `qi` is being eliminated. Mirrors
/// the TS `explainElimDetail`. Only called for options the engine has ruled out,
/// so a matching reason is expected; `None` means no phrasing for this kind.
pub fn explain_elim_detail(
    qt: &QuestionType,
    qi: usize,
    oi: usize,
    value: OptionValue,
    state: &State,
    n: usize,
) -> Option<ElimDetail> {
    let letter = LETTERS[oi];
    let answers = &state.answers;
    let eliminated = &state.eliminated;
    let vnum = value.is_num().then(|| value.value());

    // Count kinds (count_pred is None for MostCommonCount, handled next).
    if let Some(pred) = count_pred(qt) {
        let (from, to) = count_range(qt, n);
        let cr = count_matching(answers, eliminated, pred, from, to);
        if let Some(v) = vnum {
            if cr.count > v {
                return detail(
                    format!(
                        "{} option {letter} claims {v} {}, but there are already {}.",
                        q(qi),
                        count_rule_label(qt, v),
                        cr.count
                    ),
                    None,
                );
            }
            if cr.count + cr.remaining < v {
                return detail(
                    format!(
                        "{} option {letter} claims {v} {}, but at most {} are possible.",
                        q(qi),
                        count_rule_label(qt, v),
                        cr.count + cr.remaining
                    ),
                    None,
                );
            }
        }
    }

    if matches!(qt, QuestionType::MostCommonCount)
        && let Some(v) = vnum
    {
        let mut counts = [0u8; 5];
        for i in 0..n {
            if let Some(a) = answers[i] {
                counts[a.idx()] += 1;
            }
        }
        let max_known = counts.iter().copied().max().unwrap();
        if v < max_known {
            let s = if v == 1 { "" } else { "s" };
            return detail(
                format!(
                    "{} option {letter} claims the most common answer appears {v} time{s}, but one already appears {max_known} times.",
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
                    None if !is_elim(eliminated, i, l.idx()) => r += 1,
                    _ => {}
                }
            }
            max_possible = max_possible.max(c + r);
        }
        if v > max_possible {
            return detail(
                format!(
                    "{} option {letter} claims the most common answer appears {v} times, but at most {max_possible} are possible.",
                    q(qi)
                ),
                None,
            );
        }
    }

    if let QuestionType::AnswerOf { question_index } = qt
        && let Some(v) = vnum
        && v < 5
        && is_elim(eliminated, *question_index as usize, v as usize)
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

    if let QuestionType::SameAsWhich { question_index } = qt
        && let Some(ref_ans) = answers[*question_index as usize]
        && let Some(v) = vnum
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
            None if is_elim(eliminated, target, ref_ans.idx()) => {
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

    if let QuestionType::LetterDist { question_index } = qt {
        let k = *question_index as usize;
        let max_dist = oi.max(4 - oi) as u8;
        if let Some(v) = vnum
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
                if let Some(v) = vnum {
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
                if let Some(v) = vnum {
                    let any_possible = (0..5).any(|ti| {
                        !is_elim(eliminated, k, ti)
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
    }

    // Positional forward: "first" / "closest-after".
    let forward = match qt {
        QuestionType::FirstWith { answer } => Some(("first", 0usize, *answer)),
        QuestionType::ClosestAfter {
            after_index,
            answer,
        } => Some(("closest", *after_index as usize + 1, *answer)),
        _ => None,
    };
    if let Some((label, scan_start, answer)) = forward {
        match vnum {
            Some(v) => {
                let target = v as usize;
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
                    None if is_elim(eliminated, target, answer.idx()) => {
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
    }

    // Positional backward: "last" / "closest-before".
    let backward = match qt {
        QuestionType::LastWith { answer } => Some(("last", n, *answer)),
        QuestionType::ClosestBefore {
            before_index,
            answer,
        } => Some(("closest", *before_index as usize, *answer)),
        _ => None,
    };
    if let Some((label, before_idx, answer)) = backward {
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
                    None if is_elim(eliminated, target, answer.idx()) => {
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
    }

    // Parity: OnlyOdd / OnlyEven. `wrong_parity` names a mispointed target's
    // parity; `own_parity` names this question's parity for the no-match case.
    let odd_even = match qt {
        QuestionType::OnlyOdd { answer } => Some((1u8, "even", "odd", *answer)),
        QuestionType::OnlyEven { answer } => Some((0u8, "odd", "even", *answer)),
        _ => None,
    };
    if let Some((parity, wrong_parity, own_parity, answer)) = odd_even {
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
                        None if is_elim(eliminated, target, answer.idx()) => {
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
    }

    if matches!(qt, QuestionType::ConsecIdent) {
        match vnum {
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
                    if is_elim(eliminated, partner, oi) {
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
                let poss_a = !eliminated[start] & 0b11111;
                let poss_b = !eliminated[start + 1] & 0b11111;
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
    }

    if matches!(qt, QuestionType::PrevSame) {
        match vnum {
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
                if is_elim(eliminated, target, oi) {
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
    }

    if matches!(qt, QuestionType::NextSame) {
        match vnum {
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
                if is_elim(eliminated, target, oi) {
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
    }

    if matches!(qt, QuestionType::OnlySame) && vnum.is_none() {
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

    if matches!(qt, QuestionType::OnlySame | QuestionType::SameAs)
        && let Some(v) = vnum
    {
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
        if target < n && is_elim(eliminated, target, oi) {
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
        if matches!(qt, QuestionType::OnlySame) && target < n && target != qi {
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

    if matches!(qt, QuestionType::NoOtherHasAnswer) {
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
    }

    if let QuestionType::EqualCount { answer } = qt
        && let Some(v) = vnum
    {
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

    if matches!(qt, QuestionType::LeastCommon)
        && let Some(v) = vnum
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

    if matches!(qt, QuestionType::MostCommon)
        && let Some(v) = vnum
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

    if (!state.eliminated[qi] & 0b11111).count_ones() == 1 {
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

    if (!state.eliminated[qi] & 0b11111).count_ones() == 1 {
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
                simple("Try looking at #1 and #2.".into()),
                simple("#1 asks for #2's answer. #2 is B, so #1 must be B.".into()),
            ]
        );
    }
}
