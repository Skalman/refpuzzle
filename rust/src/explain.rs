//! Human-readable hint prose — the single source of truth (the frontend calls
//! this through the wasm boundary). Built on the engine's own primitives
//! (`check_answer` counts, `render` text) so the wording can't drift from what
//! the solver actually computes.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::check_answer::{InvalidReason, answered_claim, check_answer_with_reason};
use crate::counts::{count_matching, count_matching_mask, count_pred, count_range};
use crate::deduce::{DeduceAction, DeduceReason, DeduceResult, DeduceRule, reason_for};
use crate::lookahead::{Contradiction, LookaheadResult, hypothesis, replay_chain};
use crate::render::{claim_label, q};
use crate::types::*;

/// One hint step, serialized (via derive) to the wire shape the UI's `HintStep`
/// renders: `{ type: "simple", text }`, `{ type: "complex", header, lines }`, or
/// `{ type: "look", qis }`. For `Look`, only the 0-based question indices cross
/// the wire — the frontend builds the "Try looking at …" sentence (i18n
/// `hint.tryLooking`), keeping only navigation prose off the Rust side.
#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ExplainStep {
    Simple {
        text: String,
    },
    Complex {
        header: String,
        lines: Vec<String>,
    },
    /// A "look at these questions" pointer (0-based). The frontend renders the
    /// prose; `leading_questions` reads these for the coach's focus/arrows.
    Look {
        qis: Vec<usize>,
    },
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

/// Who is making a rejected claim. Fixes two things at once, because they are the same
/// decision: how the sentence opens, and how much of the board the "but …" clause may
/// state as fact.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ClaimSubject {
    /// The question's own committed answer is what's being rejected — "#3 claims …". Every
    /// question and tally the reason names really holds what it says.
    Answered,
    /// One of the question's options, rejected on a board that *assumes* it — "#3 option B
    /// claims …". #3's answer is that assumption, so anything downstream of it reads in the
    /// conditional: this sentence stands alone, with nothing before it to establish the
    /// assumption.
    Option,
    /// A refuted hypothesis, reported in the conditional — "#3 would say …". The clause
    /// stays indicative: a lookahead hint's earlier lines have already stated the questions it
    /// names ("#2 must be B."), so hedging them again would read as doubting facts the
    /// player was just handed.
    Hypothesis,
}

impl ClaimSubject {
    /// The sentence opener, through the verb: "#3 claims", "#3 option B claims", "#3 would
    /// say".
    fn opening(self, opt: OptionPos) -> String {
        match self {
            ClaimSubject::Answered => format!("{} claims", q(opt.qi)),
            ClaimSubject::Option => {
                format!("{} option {} claims", q(opt.qi), LETTERS[opt.oi])
            }
            ClaimSubject::Hypothesis => format!("{} would say", q(opt.qi)),
        }
    }

    /// Whether the clause may state the board as fact. False only for `Option`, whose
    /// board carries an assumption no preceding line has stated.
    fn indicative(self) -> bool {
        !matches!(self, ClaimSubject::Option)
    }
}

/// Why question `qi`'s current answer is invalid, or `None` if it isn't (or is
/// unanswered). Verdict and reason both come from `check_answer`, so this only renders
/// a verdict — it never re-decides one, and can't miss a kind `check_answer` can reject.
///
/// The only entry point onto `ClaimSubject::Answered`. Nothing renders a standalone
/// "why is this red" line yet, so today only the tests reach it — but that subject is
/// one of the three `InvalidReason` must phrase under (see its docs), so this stays
/// wired rather than becoming a test helper.
pub fn explain_invalid(fp: &FlatPuzzle, state: &State, qi: usize) -> Option<String> {
    rejected_answer_text(fp, state, qi, ClaimSubject::Answered)
}

/// [`explain_invalid`] under a chosen subject — `explain_lookahead` reports the same
/// verdict about a hypothesis, so it asks for the conditional opening.
fn rejected_answer_text(
    fp: &FlatPuzzle,
    state: &State,
    qi: usize,
    subject: ClaimSubject,
) -> Option<String> {
    let a = state.answers[qi]?;
    let reason = check_answer_with_reason(fp, *state, qi).reason()?;
    let claim = answered_claim(fp, state, qi)?;
    rejected_claim_text(
        subject,
        state,
        OptionPos { qi, oi: a.idx() },
        &claim,
        reason,
    )
}

/// A rejected claim as one sentence: "*subject* claims *what it asserts*, but *what breaks
/// it*". `None` only for the two reasons with nothing to say — a `Malformed` value
/// (which `check_form` rejects as an error, so no shipped puzzle carries one) and
/// `NoOptionsLeft` (which is about the question, not a claim).
///
/// `opt` is the option the claim came from — for a `TrueStmt` the statement it picked,
/// whose subject then reads as the question's own.
fn rejected_claim_text(
    subject: ClaimSubject,
    state: &State,
    opt: OptionPos,
    claim: &Claim,
    reason: InvalidReason,
) -> Option<String> {
    let assertion = claim_assertion(state, opt, claim)?;
    let clause = invalid_clause(state, opt, claim, reason, subject)?;
    Some(format!(
        "{} {assertion}, but {clause}",
        subject.opening(opt)
    ))
}

/// What a claim asserts, as the continuation of "#3 claims …". Exhaustive over
/// `QuestionType` like `render::question_text`, so a new kind has to supply one (or join
/// the arm for the two that can never be claimed).
fn claim_assertion(state: &State, opt: OptionPos, claim: &Claim) -> Option<String> {
    use QuestionType::*;
    let qt = claim.question_type;
    // The value the option asserts — a count, a letter index, or a 0-based question
    // index depending on the kind; `None` is the NONE option's "no such thing". A value
    // out of its kind's range comes back `Malformed` and so is never rendered, which is what
    // makes `LETTERS[…]` below safe.
    let value = claim.value.is_num().then(|| claim.value.value());
    // What this question's own answer is *for the claim*: the option it selected. The
    // same reading `check_answer` checks a self-referential kind against.
    let own = LETTERS[opt.oi];

    Some(match qt {
        CountAnswer { .. }
        | CountAnswerBefore { .. }
        | CountAnswerAfter { .. }
        | CountVowel
        | CountConsonant => {
            let v = value?;
            format!("{v} {}", count_rule_label(&qt, v))
        }

        MostCommonCount => format!("the most common answer appears {}", times(value?)),

        FirstWith { answer } => match value {
            Some(v) => format!("the first {answer} is {}", q(v)),
            None => format!("no question has answer {answer}"),
        },
        LastWith { answer } => match value {
            Some(v) => format!("the last {answer} is {}", q(v)),
            None => format!("no question has answer {answer}"),
        },
        ClosestAfter {
            after_index,
            answer,
        } => match value {
            Some(v) => format!("the closest {answer} after {} is {}", q(after_index), q(v)),
            None => format!("no question after {} has answer {answer}", q(after_index)),
        },
        ClosestBefore {
            before_index,
            answer,
        } => match value {
            Some(v) => format!(
                "the closest {answer} before {} is {}",
                q(before_index),
                q(v)
            ),
            None => format!("no question before {} has answer {answer}", q(before_index)),
        },
        PrevSame => match value {
            Some(v) => format!("the previous {own} is {}", q(v)),
            None => format!("no earlier question has answer {own}"),
        },
        NextSame => match value {
            Some(v) => format!("the next {own} is {}", q(v)),
            None => format!("no later question has answer {own}"),
        },
        OnlySame => match value {
            Some(v) => format!("{} is the only other question with answer {own}", q(v)),
            None => format!("no other question has answer {own}"),
        },
        SameAs => match value {
            Some(v) => format!(
                "{} is the only one of these questions with answer {own}",
                q(v)
            ),
            None => format!("none of these questions has answer {own}"),
        },
        SameAsWhich { question_index } => {
            let k = usize::from(question_index);
            // Nothing is decided until the reference is answered, so a rejected claim
            // always has it.
            let matched = state.answers[k]?;
            match value {
                Some(v) => format!(
                    "{} is the only one of these questions with the same answer as {} ({matched})",
                    q(v),
                    q(k)
                ),
                None => format!(
                    "none of these questions has the same answer as {} ({matched})",
                    q(k)
                ),
            }
        }
        OnlyOdd { answer } => match value {
            Some(v) => format!(
                "{} is the only odd-numbered question with answer {answer}",
                q(v)
            ),
            None => format!("no odd-numbered question has answer {answer}"),
        },
        OnlyEven { answer } => match value {
            Some(v) => format!(
                "{} is the only even-numbered question with answer {answer}",
                q(v)
            ),
            None => format!("no even-numbered question has answer {answer}"),
        },
        ConsecIdent => match value {
            Some(v) => format!(
                "{} and {} are the only identical consecutive pair",
                q(v),
                q(v as usize + 1)
            ),
            None => "no two consecutive questions have identical answers".to_string(),
        },

        AnswerOf { question_index } => format!(
            "{}'s answer is {}",
            q(question_index),
            LETTERS[value? as usize]
        ),
        LeastCommon => format!("{} is the least common answer", LETTERS[value? as usize]),
        MostCommon => format!("{} is the most common answer", LETTERS[value? as usize]),
        NoOtherHasAnswer => format!("no other question has answer {}", LETTERS[value? as usize]),
        EqualCount { answer } => match value {
            Some(v) => format!("{} appears as often as {answer}", LETTERS[v as usize]),
            None => format!("no answer appears as often as {answer}"),
        },
        LetterDist { question_index } => format!(
            "its answer is {} from {}'s",
            letters(value?),
            q(question_index)
        ),

        // Never claimed: `AnswerIsSelf` holds whatever value it asserts, and a statement
        // can't nest inside another (`check_form::check_stmt_kind`).
        AnswerIsSelf | TrueStmt => return None,
    })
}

/// What broke a claim, as the continuation of "…, but …" — see [`InvalidReason`], which
/// carries the questions and tallies these read.
///
/// A clause naming a question reads in the conditional when that question is `opt.qi` and
/// its answer is only assumed — "#1 is answered A" is a claim about the board a player can
/// check and find empty. Tallies hedge on the subject instead: they are counted on the
/// board that assumes the option, so under `Option` the number can include the assumed
/// question and may not be visible on the board ("there would already be 2"). Under
/// `Hypothesis` they stay indicative — the lookahead's earlier lines have already stated
/// the questions the count reads.
fn invalid_clause(
    state: &State,
    opt: OptionPos,
    claim: &Claim,
    reason: InvalidReason,
    subject: ClaimSubject,
) -> Option<String> {
    use InvalidReason::*;
    let answers = &state.answers;
    let value = claim.value.is_num().then(|| claim.value.value());
    let indicative = subject.indicative();
    // Is the board's account of this question an assumption rather than a fact?
    let assumed = |at: u8| !indicative && usize::from(at) == opt.qi;
    // "#2 has answer B", or its conditional form when that question is the assumption itself.
    let has = |at: u8| {
        if assumed(at) {
            "itself would have"
        } else {
            "has"
        }
    };

    Some(match reason {
        Malformed | NoOptionsLeft => return None,

        CountFloor { count, guaranteed } => {
            if guaranteed == 0 && indicative {
                format!("there are already {count}")
            } else if guaranteed == 0 {
                format!("there would already be {count}")
            } else {
                format!(
                    "{} {} certain",
                    count + guaranteed,
                    if indicative {
                        "are already"
                    } else {
                        "would be"
                    }
                )
            }
        }
        CountCeiling { max } | PeakCeiling { max } => format!(
            "at most {max} {} possible",
            if indicative { "are" } else { "would be" }
        ),
        PeakFloor { letter, floor } => format!(
            "{letter} {} certain to appear {}",
            if indicative { "is already" } else { "would be" },
            times(floor)
        ),

        TargetAnswered { at, answer } => {
            if assumed(at) {
                format!("{} itself would be {answer}", q(at))
            } else {
                format!("{} is answered {answer}", q(at))
            }
        }
        // Reads the rest of the board's eliminations. `opt.qi` can't be the target here —
        // under an assumption it is answered, which `target_broken` reports as answered.
        TargetCannot { at, letter } => format!("{letter} is ruled out for {}", q(at)),
        OtherHasLetter { at, letter } => {
            let joins = if assumed(at) { "" } else { also(claim) };
            format!("{} {} answer {letter}{joins}", q(at), has(at))
        }
        // Directional reasons only come from a claim that named a position, so `value?`
        // can't decline here.
        EarlierHasLetter { at, letter } | LaterHasLetter { at, letter } => {
            let comes = if matches!(reason, EarlierHasLetter { .. }) {
                "before"
            } else {
                "after"
            };
            format!(
                "{} {} answer {letter} and comes {comes} {}",
                q(at),
                has(at),
                q(value?)
            )
        }

        PairDiffers { at, first, second } => format!(
            "{} {} {first} and {} {} {second}",
            q(at),
            if assumed(at) { "would be" } else { "is" },
            q(at as usize + 1),
            if assumed(at + 1) { "would be" } else { "is" },
        ),
        // Leans on both questions' remaining options, so the assumption narrowing `opt.qi`'s can
        // be what closed the overlap.
        PairImpossible { at } => {
            let pair = format!("{} and {}", q(at), q(at as usize + 1));
            if assumed(at) || assumed(at + 1) {
                format!("{pair} would have no answer left in common")
            } else {
                format!("{pair} have no answer left in common")
            }
        }
        OtherPairMatches { at } => {
            let pair = format!("{} and {}", q(at), q(at as usize + 1));
            if assumed(at) || assumed(at + 1) {
                format!("{pair} would be identical")
            } else {
                format!("{pair} are identical{}", also(claim))
            }
        }

        CountsCantMeet {
            short,
            short_max,
            over,
            over_min,
        } => format!(
            "{over} {} at least {} and {short} {} reach at most {short_max}",
            if indicative {
                "already appears"
            } else {
                "would appear"
            },
            times(over_min),
            if indicative { "can" } else { "could" }
        ),
        OtherLetterTies { letter } => {
            format!("{letter} {}", if indicative { "does" } else { "would" })
        }

        NotExtremum {
            rival,
            rival_count,
            claimed_count,
        } => format!(
            "{rival} {} {} and {} {}",
            if indicative {
                "appears"
            } else {
                "would appear"
            },
            times(rival_count),
            LETTERS[value? as usize],
            times(claimed_count)
        ),
        ExtremumTied { rival, count } => format!(
            "{rival} {} {} too",
            if indicative {
                "appears"
            } else {
                "would appear"
            },
            times(count)
        ),
        // A floor against a ceiling, so it has to read as bounds: "appears 3 times" would
        // state a count the board hasn't settled.
        ExtremumOutOfReach {
            over,
            over_min,
            short,
            short_max,
        } => format!(
            "{over} {} {} and {short} {} reach at most {}",
            if indicative {
                "already appears"
            } else {
                "would already appear"
            },
            times(over_min),
            if indicative { "can" } else { "could" },
            times(short_max)
        ),
        // No rival to point at: the whole argument is the bound the board's shape puts on the
        // extreme letter, so the clause states that bound and then the claimed letter's own.
        ExtremumPigeonhole { threshold, reach } => {
            let claimed = LETTERS[value? as usize];
            match claim.question_type {
                QuestionType::LeastCommon => format!(
                    "the least common answer can appear at most {}, and {claimed} {} {}",
                    times(threshold),
                    if indicative {
                        "already appears"
                    } else {
                        "would already appear"
                    },
                    times(reach)
                ),
                QuestionType::MostCommon => format!(
                    "the most common answer must appear at least {}, and {claimed} {} reach at most {}",
                    times(threshold),
                    if indicative { "can" } else { "could" },
                    times(reach)
                ),
                _ => return None,
            }
        }

        // Pure letter arithmetic against the other question's answer — the assumption is the
        // subject of the claim, not evidence for the clause.
        WrongDistance { actual } => {
            let QuestionType::LetterDist { question_index } = claim.question_type else {
                return None;
            };
            // Undecided until the other question is answered, so it always is here.
            let other = answers[usize::from(question_index)]?;
            format!("{} is {} from {other}", LETTERS[opt.oi], letters(actual))
        }
        // Neither reads the assumption — one is pure letter arithmetic, the other the
        // target's remaining options — so both stay indicative under every subject.
        DistanceUnreachable { max } => format!(
            "{} can be at most {} from any answer",
            LETTERS[opt.oi],
            letters(max)
        ),
        NoLetterAtDistance => {
            let QuestionType::LetterDist { question_index } = claim.question_type else {
                return None;
            };
            format!(
                "no answer {} still has left is that far off",
                q(question_index)
            )
        }
    })
}

/// " too", or nothing when the claim denies the thing exists anywhere ("no question has
/// answer B", "no two consecutive questions have identical answers"). Every other claim
/// has already pointed at an instance — the position it named, or the question's own
/// answer — so a second one *joins* it; these have nothing for it to join.
fn also(claim: &Claim) -> &'static str {
    use QuestionType::*;
    let denies_any = !claim.value.is_num()
        && matches!(
            claim.question_type,
            FirstWith { .. }
                | LastWith { .. }
                | ClosestAfter { .. }
                | ClosestBefore { .. }
                | OnlyOdd { .. }
                | OnlyEven { .. }
                | ConsecIdent
        );
    if denies_any { "" } else { " too" }
}

/// A count with the matching plural, e.g. "1 time" / "3 times".
fn times(n: u8) -> String {
    if n == 1 {
        format!("{n} time")
    } else {
        format!("{n} times")
    }
}

/// A letter distance with the matching plural, e.g. "1 letter" / "2 letters".
fn letters(n: u8) -> String {
    if n == 1 {
        format!("{n} letter")
    } else {
        format!("{n} letters")
    }
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

/// The questions a sub-range count question doesn't cover, as the subject of "even if …
/// were B" — "the question outside that range", "both questions outside that range",
/// "all 3 questions outside that range". Only called with a non-zero count.
fn outside_range_phrase(outside: u8) -> String {
    match outside {
        1 => "the question outside that range".to_string(),
        2 => "both questions outside that range".to_string(),
        _ => format!("all {outside} questions outside that range"),
    }
}

/// Why an eliminated option is impossible, plus the "other" question the reason
/// leans on (for highlighting), or `None` if this kind has no specific reason.
struct ElimDetail {
    pub text: String,
    pub other_qi: Option<usize>,
}

fn detail(text: String, other_qi: Option<usize>) -> Option<ElimDetail> {
    Some(ElimDetail { text, other_qi })
}

/// Why option `oi` of question `qi` is impossible, as one sentence — "#1 option C claims
/// the first A is #3, but #2 has answer A and comes before #3." Answering `oi` would commit
/// `qi` to a claim, so this asks `check_answer` about *that* claim on the state that assumes it,
/// and renders the verdict through the same `rejected_claim_text` the answered case uses,
/// under a hypothetical subject. `deduce_reason` covers the one rule `check_answer` can't
/// reach a verdict on (see [`elim_clause_beyond_check_answer`]).
///
/// `None` when there is nothing to say: `check_answer` doesn't reject the claim and the
/// carried reason has no clause either. That is not the same as "no phrasing for this
/// kind" — an elimination whose argument is *another* question's is
/// `explain_elimination`'s to phrase, from the rule, before it reaches here.
fn explain_elim_detail(
    fp: &FlatPuzzle,
    state: &State,
    qi: usize,
    oi: usize,
    deduce_reason: DeduceReason,
) -> Option<ElimDetail> {
    let letter = LETTERS[oi];
    let hyp = hypothesis(state, qi, letter);
    let opt = OptionPos { qi, oi };
    let claim = answered_claim(fp, &hyp, qi)?;
    let subject = ClaimSubject::Option;

    if let Some(reason) = check_answer_with_reason(fp, hyp, qi).reason()
        && let Some(text) = rejected_claim_text(subject, &hyp, opt, &claim, reason)
    {
        return detail(format!("{text}."), reason_other_qi(qi, &claim, reason));
    }
    let (clause, other_qi) = elim_clause_beyond_check_answer(&hyp, opt, &claim, deduce_reason)?;
    let assertion = claim_assertion(&hyp, opt, &claim)?;
    detail(
        format!("{} {assertion}, but {clause}.", subject.opening(opt)),
        other_qi,
    )
}

/// The question an [`InvalidReason`] points at, for the elimination's "Try looking at …"
/// highlight — the question that refutes the claim, or none when the reason is about tallies
/// rather than a place, or when that question is `qi` itself (which the hint already points at,
/// and which would collapse the two-question `Look` to one). Exhaustive so a new reason has
/// to say which it is.
fn reason_other_qi(qi: usize, claim: &Claim, reason: InvalidReason) -> Option<usize> {
    use InvalidReason::*;
    match reason {
        TargetAnswered { at, .. }
        | TargetCannot { at, .. }
        | OtherHasLetter { at, .. }
        | EarlierHasLetter { at, .. }
        | LaterHasLetter { at, .. } => Some(usize::from(at)).filter(|&at| at != qi),
        // A pair reason names two questions; when the first is `qi` itself the partner still
        // carries the argument, so the highlight moves there instead of vanishing.
        PairDiffers { at, .. } | PairImpossible { at } | OtherPairMatches { at } => {
            let at = usize::from(at);
            Some(if at == qi { at + 1 } else { at })
        }
        // The distance is measured against the question the kind names; the reason carries
        // only the distance itself. `NoLetterAtDistance` too — the target's surviving
        // options are the evidence.
        WrongDistance { .. } | NoLetterAtDistance => match claim.question_type {
            QuestionType::LetterDist { question_index } => Some(usize::from(question_index)),
            _ => None,
        },
        // None of these blames a question: most are tallies, `DistanceUnreachable` blames the
        // alphabet, and `ExtremumPigeonhole` the shape of the board. Nowhere to point.
        Malformed
        | NoOptionsLeft
        | CountFloor { .. }
        | CountCeiling { .. }
        | PeakFloor { .. }
        | PeakCeiling { .. }
        | CountsCantMeet { .. }
        | OtherLetterTies { .. }
        | NotExtremum { .. }
        | ExtremumTied { .. }
        | ExtremumOutOfReach { .. }
        | ExtremumPigeonhole { .. }
        | DistanceUnreachable { .. } => None,
    }
}

/// The option value `answer` selects at question `qi`, if numeric.
fn option_value_at(fp: &FlatPuzzle, qi: usize, answer: Answer) -> Option<u8> {
    let ov = fp.options[qi][answer.idx()];
    ov.is_num().then(|| ov.value())
}

/// The "…, but *what breaks it*" clause for the eliminations `check_answer` can't reach a
/// verdict on, plus the question to highlight — rendered from the reason the rule carried
/// out of `deduce`, never re-derived here.
///
/// `EqualCountRangeElim` only. It folds in a sibling *count* question, which is
/// cross-question reasoning and outside `check_claim`'s scope by design.
///
/// The extremum kinds need nothing here. `check_claim` settles every extremum case arguable from
/// the marks — pairwise and by whole-board pigeonhole, both off cells alone — and where a sibling
/// count question set the bound instead, `explain_elimination` quotes that question and returns
/// before reaching here. Those two cases are complementary, so an extremum arm would have nothing
/// to say. Should a rule eliminate an extremum cell with no arm of its own, the caller panics
/// naming that rule, as it does for any kind it cannot phrase.
///
/// It carries the reason but not the wording: `check_claim` has a variant of the same
/// shape, so the clause is built by handing that variant to [`invalid_clause`] — the two
/// paths can't drift into two phrasings of one argument. The assertion half always comes
/// from [`claim_assertion`].
fn elim_clause_beyond_check_answer(
    state: &State,
    opt: OptionPos,
    claim: &Claim,
    deduce_reason: DeduceReason,
) -> Option<(String, Option<usize>)> {
    let DeduceReason::CountsCantMeet {
        short,
        short_max,
        over,
        over_min,
    } = deduce_reason
    else {
        return None;
    };
    // Always under an assumption: this is only reached from `explain_elim_detail`.
    let clause = invalid_clause(
        state,
        opt,
        claim,
        InvalidReason::CountsCantMeet {
            short,
            short_max,
            over,
            over_min,
        },
        ClaimSubject::Option,
    )?;
    Some((clause, None))
}

/// A short "because …" clause for why question `qi` is forced to `letter`, or an
/// empty string if the rule has no brief phrasing. Mirrors the TS `briefForceReason`.
fn brief_force_reason(
    fp: &FlatPuzzle,
    state: &State,
    qi: usize,
    letter: Answer,
    rule: DeduceRule,
    reason: DeduceReason,
) -> String {
    let answers = &state.answers;
    let source = reason.source();

    match rule {
        DeduceRule::AnswerOfForward => {
            if let QuestionType::AnswerOf { question_index } = fp.question_types[qi]
                && let Some(target) = answers[question_index as usize]
            {
                return format!("{} is {target}", q(question_index));
            }
        }
        DeduceRule::AnswerOfReverse => {
            if let Some(other) = source
                && let Some(other_ans) = answers[other]
            {
                return format!("{} is {other_ans}, which implies {letter}", q(other));
            }
        }
        DeduceRule::SameAsReverse => {
            if let Some(other) = source {
                return format!("same answer as {}", q(other));
            }
        }
        DeduceRule::PrevNextOnlySameReverse => {
            if let Some(other) = source
                && let Some(other_ans) = answers[other]
            {
                return format!("{} is {other_ans}, same answer as {}", q(other), q(qi));
            }
        }
        _ => {}
    }

    if (!state.eliminated[qi] & ALL_OPTIONS_MASK).count_ones() == 1 {
        return "only option left".to_string();
    }

    String::new()
}

/// The narrated steps for a forced answer: `qi` must be `letter` (via `rule`,
/// justified by `reason`). Mirrors the TS `explainForce`.
///
/// Keyed on the rule; the reason supplies the question the rule leaned on, and the
/// board is read only to *describe* it (its answer, its type's wording). Each arm
/// falls through to the closing panic if the board doesn't show what the reason
/// names — that would be a rule emitting an unrenderable reason, the bug the panic
/// exists to surface.
fn explain_force(
    fp: &FlatPuzzle,
    state: &State,
    qi: usize,
    letter: Answer,
    rule: DeduceRule,
    reason: DeduceReason,
) -> Vec<ExplainStep> {
    let answers = &state.answers;
    let n = fp.n;
    let qt = fp.question_types[qi];
    let mut steps = vec![try_looking(&[qi])];
    let source = reason.source();

    // Presentation policy, not attribution: whatever rule fired, a question down to
    // one option is simplest explained by that.
    if (!state.eliminated[qi] & ALL_OPTIONS_MASK).count_ones() == 1 {
        steps.push(simple(format!(
            "{} has only one option left — it must be {letter}.",
            q(qi)
        )));
        return steps;
    }

    match rule {
        DeduceRule::AnswerOfForward => {
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
        }

        DeduceRule::SameAsReverse | DeduceRule::PrevNextOnlySameReverse => {
            if let Some(other) = source
                && let Some(other_ans) = answers[other]
            {
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
        }

        DeduceRule::SameAsWhichReverse => {
            if let Some(other) = source
                && let Some(other_ans) = answers[other]
                && let QuestionType::SameAsWhich { question_index } = fp.question_types[other]
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

        DeduceRule::AnswerOfReverse => {
            if let Some(other) = source
                && let Some(other_ans) = answers[other]
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

        DeduceRule::LetterDistForward => {
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
        }

        DeduceRule::LetterDistReverseForce => {
            if let Some(src) = source
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

        // Counting: every question in range is now decided one way or the other
        // (no open possibilities), so the count is pinned to a single value. Uses
        // the deduce-side tally — a question forced-but-not-yet-answered still counts.
        DeduceRule::CountAllAnswered => {
            if let Some(pred) = count_pred(&qt) {
                let (from, to) = count_range(&qt, n);
                let tally =
                    count_matching_mask(&state.answers, &state.eliminated, pred.mask(), from, to);
                if tally.possible == 0 {
                    let total = tally.min();
                    steps.push(simple(format!(
                        "There are {total} {}, so {} must be {letter}.",
                        count_rule_label(&qt, total),
                        q(qi)
                    )));
                    return steps;
                }
            }
        }

        DeduceRule::CountMustMatchForce => {
            if let Some(src) = source
                && let Some(src_ans) = answers[src]
                && let Some(src_val) = option_value_at(fp, src, src_ans)
                && let Some(pred) = count_pred(&fp.question_types[src])
            {
                let src_qt = fp.question_types[src];
                let (from, to) = count_range(&src_qt, n);
                let cr = count_matching(answers, &state.eliminated, pred, from, to);
                steps.push(try_looking(&[qi, src]));
                steps.push(simple(format!(
                    "{} says there are {src_val} {}. Only {} found so far, and {} is the only remaining question that could be {letter} — so {} must be {letter}.",
                    q(src), count_rule_label(&src_qt, src_val), cr.count, q(qi), q(qi)
                )));
                return steps;
            }
        }

        DeduceRule::LeastCommonForce => {
            if matches!(qt, QuestionType::LeastCommon) {
                steps.push(simple(format!(
                    "Only one answer can make its claimed letter the least common — {} must be {letter}.",
                    q(qi)
                )));
                return steps;
            }
        }

        DeduceRule::MostCommonForce => {
            if matches!(qt, QuestionType::MostCommon) {
                steps.push(simple(format!(
                    "Only one answer can make its claimed letter the most common — {} must be {letter}.",
                    q(qi)
                )));
                return steps;
            }
        }

        DeduceRule::ConsecIdentForwardForce | DeduceRule::ConsecIdentForwardBothForce => {
            if let Some(src) = source
                && let Some(src_ans) = answers[src]
                && let Some(start) = option_value_at(fp, src, src_ans)
            {
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

        DeduceRule::TrueStatementForward => {
            if let Some(src) = source
                && let Some(src_ans) = answers[src]
                && let Some(claim) = fp.claim_at(src, src_ans.idx())
            {
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

        DeduceRule::TrueStatementClaimValid => {
            return vec![
                try_looking(&[qi]),
                simple(format!(
                    "Only one of {}'s claims is still possible, so it must be the answer.",
                    q(qi)
                )),
            ];
        }

        DeduceRule::TrueStatementClaimKnownTrue => {
            return vec![
                try_looking(&[qi]),
                simple(format!(
                    "Option {letter}'s claim is already known to be true, so it must be the answer."
                )),
            ];
        }

        DeduceRule::TrueStatementMatchForce => {
            if let Some(k) = source {
                if let Some(self_claim) = fp.claim_at(qi, letter.idx()) {
                    // `qi` is the TrueStmt: `k` settled its statement true.
                    return vec![
                        try_looking(&[k]),
                        simple(format!(
                            "{} settles \"{}\", making that statement true — so {} must be {letter}.",
                            q(k),
                            claim_label(&self_claim),
                            q(qi)
                        )),
                    ];
                }
                // `qi` is a plain question that the chosen true statement `k` points at.
                if let Some(k_ans) = answers[k]
                    && let Some(claim) = fp.claim_at(k, k_ans.idx())
                {
                    return vec![
                        try_looking(&[k]),
                        simple(format!(
                            "{}'s true statement is \"{}\", so {} must be {letter}.",
                            q(k),
                            claim_label(&claim),
                            q(qi)
                        )),
                    ];
                }
            }
        }

        _ => {}
    }

    // No arm rendered — a rule-wiring bug. Crash (with analytics) rather than
    // show a reason-less "must be {letter}."
    panic!(
        "explain_force: no explanation for {} = {letter:?} (rule {rule:?}, {reason:?})",
        qi + 1
    )
}

/// The prose for a positional-range elimination: how `src` — the positional
/// question (first/last/closest/prev-next-same) the rule carried as its reason —
/// rules `letter` out at `qi`. Reads the board only to describe `src`: its
/// answered position, or the span its remaining options still allow.
fn positional_range_text(
    fp: &FlatPuzzle,
    state: &State,
    src: usize,
    qi: usize,
    oi: usize,
) -> Option<String> {
    let n = fp.n;
    let letter = LETTERS[oi];
    let answers = &state.answers;
    let src_qt = fp.question_types[src];

    // Forward positional: first/closest-after `letter` sits at or past `qi` — or,
    // answered "none", nowhere at all.
    if matches!(src_qt, QuestionType::FirstWith { answer } | QuestionType::ClosestAfter { answer, .. } if answer == letter)
    {
        let label = if matches!(src_qt, QuestionType::FirstWith { .. }) {
            "first"
        } else {
            "closest"
        };
        return Some(match answers[src] {
            Some(src_ans) => match option_value_at(fp, src, src_ans) {
                Some(v) => format!(
                    "{} says {label} {letter} is {}, so {} can't be {letter}.",
                    q(src),
                    q(v as usize),
                    q(qi)
                ),
                None => format!(
                    "{} says there is no {label} {letter} at all, so {} can't be {letter}.",
                    q(src),
                    q(qi)
                ),
            },
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
                format!(
                    "{}'s remaining options for {label} {letter} are all at {} or later, so earlier questions can't be {letter}.",
                    q(src),
                    q(min_pos)
                )
            }
        });
    }
    // Backward positional: last/closest-before `letter` sits at or before `qi` — or,
    // answered "none", nowhere at all.
    if matches!(src_qt, QuestionType::LastWith { answer } | QuestionType::ClosestBefore { answer, .. } if answer == letter)
    {
        let label = if matches!(src_qt, QuestionType::LastWith { .. }) {
            "last"
        } else {
            "closest"
        };
        return Some(match answers[src] {
            Some(src_ans) => match option_value_at(fp, src, src_ans) {
                Some(v) => format!(
                    "{} says {label} {letter} is {}, so {} can't be {letter}.",
                    q(src),
                    q(v as usize),
                    q(qi)
                ),
                None => format!(
                    "{} says there is no {label} {letter} at all, so {} can't be {letter}.",
                    q(src),
                    q(qi)
                ),
            },
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
                format!(
                    "{}'s remaining options for {label} {letter} are all at {} or earlier, so later questions can't be {letter}.",
                    q(src),
                    q(max_pos.max(0) as usize)
                )
            }
        });
    }
    if matches!(src_qt, QuestionType::NextSame)
        && answers[src] == Some(letter)
        && let Some(v) = option_value_at(fp, src, letter)
    {
        return Some(format!(
            "{} is {letter} and says next same answer is {}, so {} can't be {letter}.",
            q(src),
            q(v as usize),
            q(qi)
        ));
    }
    if matches!(src_qt, QuestionType::PrevSame)
        && answers[src] == Some(letter)
        && let Some(v) = option_value_at(fp, src, letter)
    {
        return Some(format!(
            "{} is {letter} and says previous same answer is {}, so {} can't be {letter}.",
            q(src),
            q(v as usize),
            q(qi)
        ));
    }
    None
}

/// The prose for a count-saturation elimination: `src` — the count question the
/// rule carried as its reason — is already at its stated count (`CountSaturated`,
/// so `qi` can't add another match) or one short of it (`CountMustMatchElim`, so
/// `qi` must match). The rule picks the case; the tally is read back only to
/// quote its numbers. Mirrors `explainCountSaturation`.
fn count_saturation_text(
    fp: &FlatPuzzle,
    state: &State,
    src: usize,
    qi: usize,
    oi: usize,
    rule: DeduceRule,
) -> Option<String> {
    let n = fp.n;
    let letter = LETTERS[oi];
    let ans = state.answers[src]?;
    let src_qt = fp.question_types[src];
    let pred = count_pred(&src_qt)?;
    let value = option_value_at(fp, src, ans)?;
    let (from, to) = count_range(&src_qt, n);
    // The same tally `apply_count` fires on, so the quoted numbers match its
    // `min`/`max` exactly. (The pred-only `count_matching` folds locked-in questions
    // into `remaining`, understating the fixed count whenever one is already forced.)
    let tally = count_matching_mask(&state.answers, &state.eliminated, pred.mask(), from, to);
    let min = tally.min();
    Some(match rule {
        // CountSaturated: `value` matches are already locked in (answered or forced),
        // so no other question can take a matching option.
        DeduceRule::CountSaturated => format!(
            "{} says there are {value} {}, and {value} are already fixed — so {} can't also be {letter}.",
            q(src),
            count_rule_label(&src_qt, value),
            q(qi)
        ),
        // CountMustMatchElim: the count can only reach `value` if every remaining
        // unknown matches, so a non-matching option is impossible.
        DeduceRule::CountMustMatchElim => format!(
            "{} says there are {value} {}. Only {min} are fixed so far and every remaining unknown must match — so {} can't be {letter}.",
            q(src),
            count_rule_label(&src_qt, value),
            q(qi)
        ),
        _ => return None,
    })
}

/// The narrated steps for eliminating option `oi` of question `qi` (via `rule`,
/// justified by `reason`). Mirrors the TS `explainElimination`.
fn explain_elimination(
    fp: &FlatPuzzle,
    state: &State,
    qi: usize,
    oi: usize,
    rule: DeduceRule,
    reason: DeduceReason,
) -> Vec<ExplainStep> {
    let letter = LETTERS[oi];
    let ov = fp.options[qi][oi];
    let n = fp.n;
    let answers = &state.answers;
    let mut steps = vec![try_looking(&[qi])];
    let what_if = || simple(format!("What if {} is {letter}?", q(qi)));
    let source = reason.source();

    if matches!(
        rule,
        DeduceRule::CountSaturated | DeduceRule::CountMustMatchElim
    ) {
        if let Some(src_qi) = source
            && let Some(text) = count_saturation_text(fp, state, src_qi, qi, oi, rule)
        {
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
        if let Some(src_qi) = source
            && let Some(text) = positional_range_text(fp, state, src_qi, qi, oi)
        {
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
        // Fires for both OnlySame and SameAs (the shared "none = unique" arm);
        // `source` is the question answered "none".
        if let Some(src) = source
            && answers[src] == Some(letter)
        {
            steps.push(try_looking(&[qi, src]));
            steps.push(what_if());
            steps.push(simple(
                if matches!(fp.question_types[src], QuestionType::SameAs) {
                    format!(
                        "{} is {letter} and claims none of its listed questions shares that answer, so {} can't be {letter}.",
                        q(src),
                        q(qi)
                    )
                } else {
                    format!(
                        "{} is {letter} and claims no other question shares that answer, so {} can't be {letter}.",
                        q(src),
                        q(qi)
                    )
                },
            ));
            return steps;
        }
    }

    if matches!(rule, DeduceRule::SameAsWhichNoneForward)
        && let Some(src) = source
        && let QuestionType::SameAsWhich { question_index } = fp.question_types[src]
        && answers[usize::from(question_index)] == Some(letter)
    {
        let k = usize::from(question_index);
        steps.push(try_looking(&[qi, src, k]));
        steps.push(what_if());
        steps.push(simple(format!(
            "{} claims none of its listed questions is answered {letter} like {}, so {} can't be {letter}.",
            q(src),
            q(k),
            q(qi)
        )));
        return steps;
    }

    if matches!(rule, DeduceRule::ConsecIdentForwardElim)
        && let Some(src) = source
        && let Some(src_ans) = answers[src]
        && let Some(start) = option_value_at(fp, src, src_ans)
    {
        let p = start as usize;
        if p == qi || p + 1 == qi {
            let partner = if p == qi { p + 1 } else { p };
            steps.push(try_looking(&[qi, partner, src]));
            steps.push(what_if());
            steps.push(simple(format!(
                "{} says {} and {} must have the same answer, but {letter} is ruled out for {}.",
                q(src),
                q(p),
                q(p + 1),
                q(partner)
            )));
            return steps;
        }
    }

    if matches!(rule, DeduceRule::ConsecIdentReverse)
        && let Some(src) = source
    {
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
                q(qi),
                q(neighbor),
                q(src)
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

    // The bound these two rules argue from, named by the question that set it — carried
    // out of `deduce` as the rule's reason. A `Board` reason means placed cells drove the
    // bound with no count question to quote — the cell-based explanation below takes over.
    if matches!(
        rule,
        DeduceRule::LeastCommonCountFloor | DeduceRule::MostCommonCountCeil
    ) && ov.is_num()
        && (ov.value() as usize) < fp.option_count
        && let DeduceReason::LetterBound {
            source: src_qi,
            bound,
            own_range,
        } = reason
    {
        let claimed = LETTERS[ov.value() as usize];
        let src_qi = usize::from(src_qi);
        let src_qt = fp.question_types[src_qi];
        if matches!(rule, DeduceRule::LeastCommonCountFloor) {
            // A sub-range floor bounds the whole board as it stands, so label and number
            // describe the same set.
            steps.push(try_looking(&[qi, src_qi]));
            steps.push(what_if());
            steps.push(simple(format!(
                "{} means there are at least {bound} {}, so {claimed} appears too often to be the least common.",
                q(src_qi),
                count_rule_label(&src_qt, bound),
            )));
            return steps;
        }
        let outside = bound - own_range;
        steps.push(try_looking(&[qi, src_qi]));
        steps.push(what_if());
        // The sentence has to walk the widening rather than quote the whole-board total
        // under the source's own label, which would state a cap the source never set.
        steps.push(simple(if outside == 0 {
            format!(
                "{} means there are at most {bound} {}, so {claimed} appears too rarely to be the most common.",
                q(src_qi),
                count_rule_label(&src_qt, bound),
            )
        } else {
            format!(
                "{} means there are at most {own_range} {}, and even if {} were {claimed}, that's at most {bound} in all — so {claimed} appears too rarely to be the most common.",
                q(src_qi),
                count_rule_label(&src_qt, own_range),
                outside_range_phrase(outside),
            )
        }));
        return steps;
    }

    if matches!(rule, DeduceRule::TrueStatementMatchElim) {
        let claim = fp.claim_at(qi, oi);
        if let Some(k) = source {
            steps.push(try_looking(&[qi, k]));
        }
        steps.push(what_if());
        steps.push(simple(match (claim, source) {
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

    // Generic fallback: the claim that answering this option would commit `qi` to, rejected.
    let detail = explain_elim_detail(fp, state, qi, oi, reason);
    if let Some(other) = detail.as_ref().and_then(|d| d.other_qi) {
        steps.push(try_looking(&[qi, other]));
    }
    steps.push(what_if());
    match detail {
        Some(d) => steps.push(simple(d.text)),
        // No per-type reason and no rule-specific branch above — crash (with
        // analytics) rather than show a reason-less "can't be {letter}."
        None => panic!(
            "no explain_elim_detail for {} option {letter:?} (rule {rule:?})",
            qi + 1
        ),
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
    reason: DeduceReason,
) -> (String, Option<usize>) {
    let answers = &state.answers;
    let source = reason.source();

    if matches!(rule, DeduceRule::SameAsNegative)
        && let Some(src) = source
    {
        return (
            format!(
                "{} identifies which question shares its answer, so the other listed questions cannot have the same answer.",
                q(src)
            ),
            Some(src),
        );
    }

    if matches!(rule, DeduceRule::SameAsWhichNegative)
        && let Some(src) = source
        && let QuestionType::SameAsWhich { question_index } = fp.question_types[src]
        && let Some(src_ans) = answers[src]
        && let Some(target) = option_value_at(fp, src, src_ans)
    {
        let k = usize::from(question_index);
        return (
            match answers[k] {
                Some(ref_ans) => format!(
                    "{} says {} is the only one of its listed questions answered {ref_ans} (the answer to {}), so the others cannot be {ref_ans}.",
                    q(src),
                    q(target as usize),
                    q(k)
                ),
                None => format!(
                    "{} says {} is the only one of its listed questions matching {}, so the others cannot match it.",
                    q(src),
                    q(target as usize),
                    q(k)
                ),
            },
            Some(src),
        );
    }

    if matches!(rule, DeduceRule::LetterDistReverseElim)
        && let Some(src) = source
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

    if matches!(rule, DeduceRule::OnlyOddEvenRangeElim)
        && let Some(src) = source
    {
        let (parity, answer) = match fp.question_types[src] {
            QuestionType::OnlyOdd { answer } => ("odd", answer),
            QuestionType::OnlyEven { answer } => ("even", answer),
            _ => panic!("OnlyOddEvenRangeElim sourced from a non-parity question"),
        };
        return (
            format!(
                "{} asks for the only {parity}-numbered question with answer {answer}, limiting which {parity} questions can have that answer.",
                q(src)
            ),
            Some(src),
        );
    }

    if matches!(
        rule,
        DeduceRule::CountSaturated | DeduceRule::CountMustMatchElim
    ) {
        let sample_oi = (0..5).find(|&b| (option_mask >> b) & 1 == 1).unwrap_or(0);
        if let Some(src_qi) = source
            && let Some(text) = count_saturation_text(fp, state, src_qi, qi, sample_oi, rule)
        {
            return (text, Some(src_qi));
        }
    }

    if matches!(
        rule,
        DeduceRule::CountExceeded | DeduceRule::CountImpossible
    ) {
        let qt = fp.question_types[qi];
        if let Some(pred) = count_pred(&qt) {
            let (from, to) = count_range(&qt, fp.n);
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

    // No handler for this multi-elim rule, or its saturating source couldn't be
    // reconstructed — a hint we can't justify. Crash (with analytics) rather than
    // show a reason-less "can't be those options."
    panic!("no explain_multi_elim handler for {rule:?} at {}", qi + 1)
}

/// Turn one `DeduceResult` into narrated hint steps, justified by the reason
/// `deduce` carried for it. Mirrors `explainDeduce`.
pub fn explain_deduce(
    fp: &FlatPuzzle,
    state: &State,
    result: &DeduceResult,
    reason: DeduceReason,
) -> Vec<ExplainStep> {
    let n = fp.n;
    match result.action {
        DeduceAction::Force { qi, answer } => {
            explain_force(fp, state, qi, answer, result.rule, reason)
        }
        DeduceAction::Eliminate { qi, oi } => {
            explain_elimination(fp, state, qi, oi, result.rule, reason)
        }
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
                let source = reason.source();
                if let Some(src_qi) = source
                    && let Some(text) = positional_range_text(fp, state, src_qi, qis[0], oi)
                {
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
                    explain_multi_elim(fp, state, qis[0], option_mask, result.rule, reason);
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

/// One elimination step of a lookahead chain, rendered through the full
/// `explain_elimination` logic and flattened to a single reason line, plus the
/// questions it highlights. The redundant "What if …?" step is dropped — the whole
/// chain already sits under one.
fn elim_chain_line(
    fp: &FlatPuzzle,
    state: &State,
    qi: usize,
    oi: usize,
    rule: DeduceRule,
    reason: DeduceReason,
) -> (String, Vec<usize>) {
    let steps = explain_elimination(fp, state, qi, oi, rule, reason);
    let skip = format!("What if {} is {}?", q(qi), LETTERS[oi]);
    let reason = steps
        .iter()
        .filter_map(|s| match s {
            ExplainStep::Simple { text } if *text != skip => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ");
    (reason, leading_questions(&steps))
}

/// The closing detail when nothing can be said about *why* `qi` broke — a committed answer
/// `check_answer` rejects for a reason `explain_invalid_detail` has no arm for.
pub fn no_reason_detail(qi: usize) -> String {
    format!("{} would be invalid", q(qi))
}

/// The closing detail when an elimination would take `qi`'s last option. Lives here rather
/// than inline because `reference`'s hint audit identifies that route by this exact
/// sentence.
pub fn optionless_detail(qi: usize) -> String {
    format!("{} would have no options left", q(qi))
}

/// How the hypothesis broke at `qi`, for the hint's closing line when
/// `explain_invalid_detail` declines — phrased against `hyp`, the state the chain ends in.
/// `AnswerInvalid` reaches here only when that function has no arm for the kind, leaving
/// nothing better to say than `no_reason_detail`.
fn refutation_detail(
    fp: &FlatPuzzle,
    hyp: &State,
    qi: usize,
    contradiction: &Contradiction,
) -> String {
    match contradiction {
        Contradiction::AnswerInvalid => no_reason_detail(qi),
        Contradiction::Conflict {
            result,
            derived_from,
        } => match result.action {
            DeduceAction::Force { answer, .. } => {
                // The chain carries no reasons (see `replay_chain`), so recover this
                // conflict's by deducing its recorded pre-state again.
                let deduce_reason = reason_for(fp, derived_from, result);
                let reason =
                    brief_force_reason(fp, derived_from, qi, answer, result.rule, deduce_reason);
                let forced = if reason.is_empty() {
                    format!("{} would have to be {answer}", q(qi))
                } else {
                    format!("{} would have to be {answer} ({reason})", q(qi))
                };
                match hyp.answers[qi] {
                    Some(a) => format!("{forced}, but it would already be {a}"),
                    None => format!("{forced}, which is already ruled out for it"),
                }
            }
            // An elimination conflicts either by removing a committed answer — the letter
            // removed is that same answer, already stated by a preceding line, so this one
            // doesn't repeat it — or by taking an unanswered question's last option, the
            // only way a hint now reports one with nowhere left to go. No reason attached:
            // the elimination explainers return a whole sentence built to follow a colon,
            // which doesn't fit this frame.
            DeduceAction::Eliminate { .. } | DeduceAction::EliminateMulti { .. } => {
                match hyp.answers[qi] {
                    Some(a) => format!("{a} would be ruled out for {}", q(qi)),
                    None => optionless_detail(qi),
                }
            }
        },
    }
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

    let mut hyp = hypothesis(state, qi, letter);

    let mut involved: BTreeSet<usize> = BTreeSet::from([qi]);
    let mut lines: Vec<String> = Vec::new();

    // One line per chain entry, its reason rendered against the round pre-state
    // `replay_chain` hands over — rendering against the running state instead would show a
    // step a state its same-round siblings have already advanced, collapsing (say) a count
    // bound the reason relies on.
    let replayed = replay_chain(
        fp,
        &mut hyp,
        &result.chain,
        &mut 0,
        |round_pre, dr, reason| {
            match dr.action {
                DeduceAction::Force { qi: fqi, answer } => {
                    involved.insert(fqi);
                    let brief = brief_force_reason(fp, round_pre, fqi, answer, dr.rule, reason);
                    lines.push(if brief.is_empty() {
                        format!("{} must be {answer}.", q(fqi))
                    } else {
                        format!("{} must be {answer} ({brief}).", q(fqi))
                    });
                }
                DeduceAction::EliminateMulti {
                    question_mask,
                    option_mask,
                } => {
                    let qis: Vec<usize> =
                        (0..n).filter(|&i| (question_mask >> i) & 1 == 1).collect();
                    involved.extend(qis.iter().copied());
                    let opt_str = (0..5)
                        .filter(|&b| (option_mask >> b) & 1 == 1)
                        .map(|b| LETTERS[b].to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let q_list = qis.iter().map(|&i| q(i)).collect::<Vec<_>>().join(", ");
                    lines.push(format!("Eliminate {opt_str} from {q_list}."));
                }
                DeduceAction::Eliminate { qi: eqi, oi } => {
                    involved.insert(eqi);
                    // Reuse the single-elimination explainer so every per-rule reason
                    // (ConsecIdent, count, positional, true-statement, …) reads the same
                    // inside a chain as on its own — no thinner second path to drift.
                    let (line, extra) = elim_chain_line(fp, round_pre, eqi, oi, dr.rule, reason);
                    involved.extend(extra);
                    lines.push(format!(
                        "Eliminate {} option {}: {line}",
                        q(eqi),
                        LETTERS[oi]
                    ));
                }
            }
        },
    );
    debug_assert!(replayed, "lookahead chain failed to replay");

    let contradiction_qi = result.contradiction_qi;
    involved.insert(contradiction_qi);
    let detail = match rejected_answer_text(fp, &hyp, contradiction_qi, ClaimSubject::Hypothesis) {
        Some(reason) => reason,
        // It only speaks about a committed answer being wrong, so it has nothing for the
        // routes that leave a question with no legal answer instead.
        None => refutation_detail(fp, &hyp, contradiction_qi, &result.contradiction),
    };
    lines.push(format!("But {detail}. Contradiction."));
    lines.push(format!("So {} can't be {letter}.", q(qi)));

    let mut steps = vec![try_looking(&[qi])];
    if involved.len() > 1 {
        steps.push(try_looking(&involved.iter().copied().collect::<Vec<_>>()));
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

    /// An elimination is explained on a board that *assumes* the option, so whatever rests on
    /// that assumption has to read as hypothetical. Both sentences here would otherwise
    /// state an unanswered #1's answer as fact, and the highlight would point at #1 —
    /// which the hint already points at, collapsing the two-question `Look` to one.
    #[test]
    fn elim_under_assumption_reads_as_hypothetical() {
        let fp = parse_puzzle(&json!({
            "q": [{"t": "FirstWith", "a": 1}, {"t": "AnswerIsSelf"}, {"t": "AnswerIsSelf"}],
            "o": [[0, 1, 2], [0, 1, 2], [0, 1, 2]],
        }))
        .unwrap();
        let state = state_with(&fp, &[None, None, None]);
        // Option A names #1 itself: assuming it makes #1 an A, not the B it claims to find.
        let d = explain_elim_detail(&fp, &state, 0, 0, DeduceReason::Board).unwrap();
        assert_eq!(
            d.text,
            "#1 option A claims the first B is #1, but #1 itself would be A."
        );
        assert_eq!(d.other_qi, None);
        // Option B names #2, but assuming it puts a B at #1, which comes first.
        let d = explain_elim_detail(&fp, &state, 0, 1, DeduceReason::Board).unwrap();
        assert_eq!(
            d.text,
            "#1 option B claims the first B is #2, but #1 itself would have answer B and comes \
             before #2."
        );
        assert_eq!(d.other_qi, None);
    }

    /// A puzzle shell of `n` questions and `oc` options. `check_claim_with_reason` reads nothing else
    /// off a puzzle, and neither does the prose — a claim carries its own value.
    fn claim_shell(n: usize, oc: usize) -> FlatPuzzle {
        let question_types = [QuestionType::AnswerIsSelf; MAX_N];
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

    /// The name of an `InvalidReason` variant, for the coverage tally. Exhaustive on
    /// purpose: a new variant won't compile until it is listed here, which is the
    /// prompt to give it prose in `invalid_clause`.
    fn reason_name(reason: InvalidReason) -> &'static str {
        use InvalidReason::*;
        match reason {
            Malformed => "Malformed",
            NoOptionsLeft => "NoOptionsLeft",
            CountFloor { .. } => "CountFloor",
            CountCeiling { .. } => "CountCeiling",
            PeakFloor { .. } => "PeakFloor",
            PeakCeiling { .. } => "PeakCeiling",
            TargetAnswered { .. } => "TargetAnswered",
            TargetCannot { .. } => "TargetCannot",
            OtherHasLetter { .. } => "OtherHasLetter",
            EarlierHasLetter { .. } => "EarlierHasLetter",
            LaterHasLetter { .. } => "LaterHasLetter",
            PairDiffers { .. } => "PairDiffers",
            PairImpossible { .. } => "PairImpossible",
            OtherPairMatches { .. } => "OtherPairMatches",
            CountsCantMeet { .. } => "CountsCantMeet",
            OtherLetterTies { .. } => "OtherLetterTies",
            NotExtremum { .. } => "NotExtremum",
            ExtremumTied { .. } => "ExtremumTied",
            ExtremumOutOfReach { .. } => "ExtremumOutOfReach",
            ExtremumPigeonhole { .. } => "ExtremumPigeonhole",
            WrongDistance { .. } => "WrongDistance",
            DistanceUnreachable { .. } => "DistanceUnreachable",
            NoLetterAtDistance => "NoLetterAtDistance",
        }
    }

    /// Anything `check_answer` can invalidate can be explained — the property that
    /// replaced the per-kind reason arms this file used to carry. Sweeps random partial
    /// boards against every claimable kind and every value a claim could hold: each
    /// `Invalid` verdict must render a sentence, unless its reason is one of the two
    /// with nothing to say (`Malformed`, which `check_form` rejects outright, and
    /// `NoOptionsLeft`, which `check_claim_with_reason` never returns).
    ///
    /// The per-reason floor is what keeps this honest: agreement is worthless if the
    /// sweep stopped producing a reason, since an unrendered one would then pass.
    #[test]
    fn every_invalid_reason_renders() {
        use crate::check_answer::check_claim_with_reason;
        use crate::rng::Rng;
        use std::collections::BTreeMap;

        // Same value set as `check_claim_fast_matches_check_claim`: every in-range
        // position/count, every letter, and NONE.
        let values: Vec<OptionValue> = (0..MAX_N as u8)
            .map(OptionValue::num)
            .chain(std::iter::once(OptionValue::NONE))
            .collect();
        let mut seen: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut unrendered: Vec<String> = Vec::new();
        let mut malformed = 0usize;
        // Some values assert (see below), so silence the default hook for the sweep and
        // report from the collected lists instead. Restored before the assertions, which
        // would otherwise print no message.
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));

        for seed in 0..300u32 {
            let mut rng = Rng::new(seed.wrapping_mul(2654435761).wrapping_add(17));
            let n = rng.int(2, MAX_N as i32) as usize;
            let oc = if rng.int(0, 1) == 0 { 3 } else { 5 };
            // A per-seed fill rate so the sweep spans barely-started boards (where a
            // pair can still be made impossible) and fully answered ones (which the
            // whole-board kinds need before they settle anything).
            let fill_rate = rng.int(2, 10);

            let mut state = State::initial(oc);
            for i in 0..n {
                if rng.int(1, 10) <= fill_rate {
                    let a = rng.pick_letter(oc);
                    state.answers[i] = Some(a);
                    state.eliminated[i] = ALL_OPTIONS_MASK ^ (1 << a.idx());
                } else {
                    for oi in 0..oc {
                        if rng.int(0, 1) == 0 {
                            state.eliminated[i] |= 1 << oi;
                        }
                    }
                }
            }

            let fp = claim_shell(n, oc);
            for qi in 0..n {
                let other = (qi as u8 + 1) % n as u8;
                let answer = rng.pick_letter(oc);
                let before_index = rng.int(0, n as i32) as u8;
                let after_index = rng.int(0, (n as i32 - 2).max(0)) as u8;
                // Every kind a claim can carry. `SameAs`/`SameAsWhich` are checked as
                // questions, not claims (`check_claim_impl` says so with an
                // `unreachable!`), and their reasons are shared with the kinds here.
                let kinds = [
                    QuestionType::CountAnswer { answer },
                    QuestionType::CountAnswerBefore {
                        answer,
                        before_index,
                    },
                    QuestionType::CountAnswerAfter {
                        answer,
                        after_index,
                    },
                    QuestionType::CountVowel,
                    QuestionType::CountConsonant,
                    QuestionType::MostCommonCount,
                    QuestionType::ClosestAfter {
                        after_index,
                        answer,
                    },
                    QuestionType::ClosestBefore {
                        before_index,
                        answer,
                    },
                    QuestionType::FirstWith { answer },
                    QuestionType::LastWith { answer },
                    QuestionType::PrevSame,
                    QuestionType::NextSame,
                    QuestionType::OnlySame,
                    QuestionType::OnlyOdd { answer },
                    QuestionType::OnlyEven { answer },
                    QuestionType::ConsecIdent,
                    QuestionType::AnswerOf {
                        question_index: other,
                    },
                    QuestionType::LeastCommon,
                    QuestionType::MostCommon,
                    QuestionType::NoOtherHasAnswer,
                    QuestionType::EqualCount { answer },
                    QuestionType::LetterDist {
                        question_index: other,
                    },
                ];
                for question_type in kinds {
                    for &value in &values {
                        let claim = Claim {
                            question_type,
                            value,
                        };
                        let opt = OptionPos {
                            qi,
                            oi: rng.int(0, oc as i32 - 1) as usize,
                        };
                        // A structurally impossible value asserts on some kinds (see the
                        // `check_answer` module doc); those aren't this test's target.
                        let verdict =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                check_claim_with_reason(&fp, state, opt, claim)
                            }));
                        let Some(reason) = verdict.ok().and_then(|j| j.reason()) else {
                            continue;
                        };
                        if matches!(
                            reason,
                            InvalidReason::Malformed | InvalidReason::NoOptionsLeft
                        ) {
                            malformed += 1;
                            continue;
                        }
                        *seen.entry(reason_name(reason)).or_insert(0) += 1;
                        // Every subject, since `ClaimSubject::Option` selects different clause
                        // wording — a reason left unrendered under any of the three is a gap.
                        for subject in [
                            ClaimSubject::Answered,
                            ClaimSubject::Option,
                            ClaimSubject::Hypothesis,
                        ] {
                            if rejected_claim_text(subject, &state, opt, &claim, reason).is_none()
                                && unrendered.len() < 10
                            {
                                unrendered.push(format!(
                                    "seed {seed} qi={qi} {question_type:?} value={value:?} \
                                     {reason:?} under {:?}: no prose",
                                    subject.opening(opt)
                                ));
                            }
                        }
                    }
                }
            }
        }

        std::panic::set_hook(hook);
        assert!(
            unrendered.is_empty(),
            "{} unexplained invalid claim(s):\n  {}",
            unrendered.len(),
            unrendered.join("\n  ")
        );
        // A denylist against the enum itself, so a new variant joins this floor by existing
        // rather than by someone remembering to add it: name the two that carry no sentence and
        // demand every other declared variant show up.
        const NO_SENTENCE: [&str; 2] = ["Malformed", "NoOptionsLeft"];
        for name in crate::check_answer::ALL_INVALID_REASON_NAMES
            .iter()
            .filter(|name| !NO_SENTENCE.contains(name))
        {
            assert!(
                seen.contains_key(*name),
                "the sweep never produced {name} — it no longer covers that reason"
            );
        }
        eprintln!(
            "every_invalid_reason_renders: {malformed} malformed, {}",
            seen.iter()
                .map(|(name, count)| format!("{name} {count}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
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

    /// "too" needs a first holder for the second one to join. A claim that reserves the
    /// letter for one place has one (here the question's own answer); a claim that denies
    /// the letter anywhere has none, and the question that has it simply refutes the claim.
    #[test]
    fn a_second_holder_joins_only_a_claim_that_had_a_first() {
        let denies_any = parse_puzzle(&json!({
            "q": [{"t": "FirstWith", "a": 0}, {"t": "AnswerIsSelf"}, {"t": "AnswerIsSelf"}],
            "o": [[1, 2, null], [0, 1, 2], [0, 1, 2]],
        }))
        .unwrap();
        // #1 = C is the "no question has answer A" option; #2 is A.
        let state = state_with(&denies_any, &[Some(Answer::C), Some(Answer::A), None]);
        assert_eq!(
            explain_invalid(&denies_any, &state, 0).as_deref(),
            Some("#1 claims no question has answer A, but #2 has answer A")
        );

        let names_one = parse_puzzle(&json!({
            "q": [{"t": "OnlySame"}, {"t": "AnswerIsSelf"}, {"t": "AnswerIsSelf"}],
            "o": [[1, 2, null], [0, 1, 2], [0, 1, 2]],
        }))
        .unwrap();
        // #1 = C claims no *other* question shares C — #1 itself is the first holder.
        let state = state_with(&names_one, &[Some(Answer::C), Some(Answer::C), None]);
        assert_eq!(
            explain_invalid(&names_one, &state, 0).as_deref(),
            Some("#1 claims no other question has answer C, but #2 has answer C too")
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
        assert_eq!(
            brief_force_reason(
                &fp,
                &state,
                0,
                Answer::B,
                DeduceRule::AnswerOfForward,
                DeduceReason::Source { source: 1 }
            ),
            "#2 is B"
        );
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
        let d = explain_elim_detail(&fp, &state, 0, 2, DeduceReason::Board).unwrap();
        assert_eq!(
            d.text,
            "#1 option C claims the first A is #3, but #2 has answer A and comes before #3."
        );
        assert_eq!(d.other_qi, Some(1));
    }

    /// The extremum kinds on a partial board: #4 is still open, and `check_answer` settles it off
    /// cell bounds anyway — `ExtremumOutOfReach` through the shared `invalid_clause`, not
    /// `elim_clause_beyond_check_answer`. The bound wording is the point: #4 could still take
    /// B, so B's count is not 0 but *at most 1*, and A's 3 is a floor, not a total.
    #[test]
    fn elim_least_common_not_least() {
        let fp = parse_puzzle(&json!({
            "q": [{"t": "LeastCommon"}, {"t": "AnswerIsSelf"}, {"t": "AnswerIsSelf"}, {"t": "AnswerIsSelf"}],
            "o": [[0, 1, 2], [0, 1, 2], [0, 1, 2], [0, 1, 2]],
        }))
        .unwrap();
        // Q2 and Q3 answered A ⇒ assuming #1 = A makes three; option A can't be least common.
        let state = state_with(&fp, &[None, Some(Answer::A), Some(Answer::A), None]);
        let d = explain_elim_detail(&fp, &state, 0, 0, DeduceReason::Board).unwrap();
        assert_eq!(
            d.text,
            "#1 option A claims A is the least common answer, but A would already appear 3 times and B could reach at most 1 time."
        );
        assert_eq!(d.other_qi, None);
    }

    /// No rival is out of reach here — every other letter still has four open questions — so the
    /// refutation is the board's own arithmetic: seven answers over three letters hold the least
    /// common one to at most 1, and the assumption puts A at 3.
    #[test]
    fn elim_least_common_past_the_pigeonhole_cap() {
        let fp = parse_puzzle(&json!({
            "q": [{"t": "LeastCommon"}, {"t": "AnswerIsSelf"}, {"t": "AnswerIsSelf"},
                  {"t": "AnswerIsSelf"}, {"t": "AnswerIsSelf"}, {"t": "AnswerIsSelf"},
                  {"t": "AnswerIsSelf"}],
            "o": [[0, 1, 2], [0, 1, 2], [0, 1, 2], [0, 1, 2], [0, 1, 2], [0, 1, 2], [0, 1, 2]],
        }))
        .unwrap();
        let state = state_with(
            &fp,
            &[
                None,
                Some(Answer::A),
                Some(Answer::A),
                None,
                None,
                None,
                None,
            ],
        );
        let d = explain_elim_detail(&fp, &state, 0, 0, DeduceReason::Board).unwrap();
        assert_eq!(
            d.text,
            "#1 option A claims A is the least common answer, but the least common answer can appear at most 1 time, and A would already appear 3 times."
        );
        assert_eq!(d.other_qi, None);
    }

    /// The mirror, and the reason the bound has to be stated rather than a tally: C is not placed
    /// anywhere, so a tally would put it at 0 — what actually rules it out is that only #4 is
    /// left to take it, while four answers over five letters need the most common one at 2 or
    /// more.
    #[test]
    fn elim_most_common_short_of_the_pigeonhole_floor() {
        let fp = parse_puzzle(&json!({
            "q": [{"t": "MostCommon"}, {"t": "AnswerIsSelf"}, {"t": "AnswerIsSelf"},
                  {"t": "AnswerIsSelf"}],
            "o": [[2, 0, 1, 3, 4], [0, 1, 2, 3, 4], [0, 1, 2, 3, 4], [0, 1, 2, 3, 4]],
        }))
        .unwrap();
        let state = state_with(&fp, &[None, Some(Answer::B), Some(Answer::D), None]);
        let d = explain_elim_detail(&fp, &state, 0, 0, DeduceReason::Board).unwrap();
        assert_eq!(
            d.text,
            "#1 option A claims C is the most common answer, but the most common answer must appear at least 2 times, and C could reach at most 1 time."
        );
        assert_eq!(d.other_qi, None);
    }

    /// Q1 = `SameAs` listing Q2/Q3 plus a "none" option, on a 4-question board.
    fn same_as_board() -> FlatPuzzle {
        parse_puzzle(&json!({
            "q": [{"t": "SameAs"}, {"t": "AnswerIsSelf"}, {"t": "AnswerIsSelf"}, {"t": "AnswerIsSelf"}],
            "o": [[1, 2, null], [0, 1, 2], [0, 1, 2], [0, 1, 2]],
        }))
        .unwrap()
    }

    /// Q1 = `SameAsWhich` referencing Q4, listing Q2/Q3 plus a "none" option.
    fn same_as_which_board() -> FlatPuzzle {
        parse_puzzle(&json!({
            "q": [{"t": "SameAsWhich", "q": 3}, {"t": "AnswerIsSelf"}, {"t": "AnswerIsSelf"}, {"t": "AnswerIsSelf"}],
            "o": [[1, 2, null], [0, 1, 2], [0, 1, 2], [0, 1, 2]],
        }))
        .unwrap()
    }

    fn elim_text(fp: &FlatPuzzle, state: &State, oi: usize) -> ElimDetail {
        explain_elim_detail(fp, state, 0, oi, DeduceReason::Board)
            .expect("every scoped-sameness elimination carries a reason")
    }

    /// §3.6: the target being *answered otherwise* needs its own wording — "ruled
    /// out for #2" is wrong for a question that already has an answer.
    #[test]
    fn elim_same_as_target_answered_otherwise() {
        let fp = same_as_board();
        let state = state_with(&fp, &[None, Some(Answer::B), None, None]);
        let d = elim_text(&fp, &state, 0);
        assert_eq!(
            d.text,
            "#1 option A claims #2 is the only one of these questions with answer A, but #2 is answered B."
        );
        assert_eq!(d.other_qi, Some(1));
    }

    #[test]
    fn elim_same_as_another_listed_candidate_matches() {
        let fp = same_as_board();
        // Option A claims #2 is the only listed question with answer A — but #3 has it.
        let state = state_with(&fp, &[None, None, Some(Answer::A), None]);
        let d = elim_text(&fp, &state, 0);
        assert_eq!(
            d.text,
            "#1 option A claims #2 is the only one of these questions with answer A, but #3 has answer A too."
        );
        assert_eq!(d.other_qi, Some(2));
    }

    #[test]
    fn elim_same_as_which_another_listed_candidate_matches() {
        let fp = same_as_which_board();
        // #4 = C is the letter to match; both listed #2 and #3 hold it.
        let state = state_with(
            &fp,
            &[None, Some(Answer::C), Some(Answer::C), Some(Answer::C)],
        );
        let d = elim_text(&fp, &state, 0);
        assert_eq!(
            d.text,
            "#1 option A claims #2 is the only one of these questions with the same answer as #4 (C), but #3 has answer C too."
        );
        assert_eq!(d.other_qi, Some(2));
    }

    #[test]
    fn elim_same_as_which_none_option_refuted() {
        let fp = same_as_which_board();
        let state = state_with(&fp, &[None, None, Some(Answer::C), Some(Answer::C)]);
        let d = elim_text(&fp, &state, 2);
        assert_eq!(
            d.text,
            "#1 option C claims none of these questions has the same answer as #4 (C), but #3 has answer C too."
        );
        assert_eq!(d.other_qi, Some(2));
    }

    /// The answered-question summary, for the verdict §2 newly turns `Invalid`.
    #[test]
    fn invalid_same_as_only_clause_broken() {
        let fp = same_as_board();
        // #1 = A points at #2 (also A), but listed #3 is A too.
        let state = state_with(
            &fp,
            &[Some(Answer::A), Some(Answer::A), Some(Answer::A), None],
        );
        assert_eq!(
            explain_invalid(&fp, &state, 0).as_deref(),
            Some(
                "#1 claims #2 is the only one of these questions with answer A, but #3 has answer A too"
            )
        );
    }

    #[test]
    fn invalid_same_as_which_only_clause_broken() {
        let fp = same_as_which_board();
        // #1 = A points at #2, matching #4's C; but listed #3 matches too.
        let state = state_with(
            &fp,
            &[
                Some(Answer::A),
                Some(Answer::C),
                Some(Answer::C),
                Some(Answer::C),
            ],
        );
        assert_eq!(
            explain_invalid(&fp, &state, 0).as_deref(),
            Some(
                "#1 claims #2 is the only one of these questions with the same answer as #4 (C), \
                 but #3 has answer C too"
            )
        );
    }

    /// The two new whole-list rules, whose prose lives in `explain_elimination` /
    /// `explain_multi_elim` rather than the per-type fallback.
    #[test]
    fn same_as_which_none_forward_and_negative_prose() {
        let fp = same_as_which_board();
        // #1 answered "none" (option C) while #4 = C: nothing listed may be C.
        let state = state_with(&fp, &[Some(Answer::C), None, None, Some(Answer::C)]);
        let steps = explain_elimination(
            &fp,
            &state,
            1,
            2,
            DeduceRule::SameAsWhichNoneForward,
            DeduceReason::Source { source: 0 },
        );
        assert!(
            render_text(&steps).contains(
                "#1 claims none of its listed questions is answered C like #4, so #2 can't be C."
            ),
            "got {:?}",
            render_text(&steps)
        );

        // #1 answered A (option A → #2): the other listed questions can't be C.
        let state = state_with(&fp, &[Some(Answer::A), None, None, Some(Answer::C)]);
        let (text, src) = explain_multi_elim(
            &fp,
            &state,
            2,
            1 << 2,
            DeduceRule::SameAsWhichNegative,
            DeduceReason::Source { source: 0 },
        );
        assert_eq!(
            text,
            "#1 says #2 is the only one of its listed questions answered C (the answer to #4), so the others cannot be C."
        );
        assert_eq!(src, Some(0));
    }

    /// A sub-range ceiling caps only the range it names, so the sentence walks the widening
    /// rather than quoting the whole-board total under the source's own label.
    #[test]
    fn elim_most_common_ceiling_widens_a_sub_range_source() {
        // Three questions, three options. The unique most-common letter needs 2 of the 3
        // answers, and #2 caps C at 0 after #1 — leaving only #1 itself, so at most 1 C.
        let fp = parse_puzzle(&json!({
            "q": [{"t": "MostCommon"}, {"t": "CountAnswerAfter", "a": 2, "q": 0}, {"t": "AnswerIsSelf"}],
            "o": [[0, 1, 2], [0, 1, 2], [0, 1, 2]],
        }))
        .unwrap();
        let state = state_with(&fp, &[None, Some(Answer::A), None]);
        let reason = assert_ceiling_rules_out_c(&fp, &state);
        let steps = explain_elimination(&fp, &state, 0, 2, DeduceRule::MostCommonCountCeil, reason);
        assert_eq!(
            render_text(&steps),
            "What if #1 is C? #2 means there are at most 0 questions after #1 with answer C, \
             and even if the question outside that range were C, that's at most 1 in all — \
             so C appears too rarely to be the most common."
        );
        assert_eq!(leading_questions(&steps), vec![0, 1]);

        // Same shape with two questions outside the range, for the "both" phrasing.
        let fp = parse_puzzle(&json!({
            "q": [{"t": "MostCommon"}, {"t": "CountAnswerBefore", "a": 2, "q": 3},
                  {"t": "AnswerIsSelf"}, {"t": "AnswerIsSelf"}, {"t": "AnswerIsSelf"}],
            "o": [[0, 1, 2], [0, 1, 2], [0, 1, 2], [0, 1, 2], [0, 1, 2]],
        }))
        .unwrap();
        let state = state_with(&fp, &[None, Some(Answer::A), None, None, None]);
        let reason = assert_ceiling_rules_out_c(&fp, &state);
        let steps = explain_elimination(&fp, &state, 0, 2, DeduceRule::MostCommonCountCeil, reason);
        assert_eq!(
            render_text(&steps),
            "What if #1 is C? #2 means there are at most 0 questions before #4 with answer C, \
             and even if both questions outside that range were C, that's at most 2 in all — \
             so C appears too rarely to be the most common."
        );
    }

    /// A full-range source needs no widening: its own cap *is* the whole-board bound, so the
    /// sentence stays the direct one.
    #[test]
    fn elim_most_common_ceiling_from_a_full_range_source() {
        let fp = parse_puzzle(&json!({
            "q": [{"t": "MostCommon"}, {"t": "CountAnswer", "a": 2}, {"t": "AnswerIsSelf"}],
            "o": [[0, 1, 2], [0, 1, 2], [0, 1, 2]],
        }))
        .unwrap();
        let state = state_with(&fp, &[None, Some(Answer::B), None]);
        let reason = assert_ceiling_rules_out_c(&fp, &state);
        let steps = explain_elimination(&fp, &state, 0, 2, DeduceRule::MostCommonCountCeil, reason);
        assert_eq!(
            render_text(&steps),
            "What if #1 is C? #2 means there are at most 1 question with answer C, \
             so C appears too rarely to be the most common."
        );
    }

    /// The sentences above describe a firing, so each board has to be one: `#1` option C
    /// really is ruled out by `MostCommonCountCeil` and not by some other rule. Returns
    /// the reason the firing carried, so the prose is rendered from the real thing.
    fn assert_ceiling_rules_out_c(fp: &FlatPuzzle, state: &State) -> DeduceReason {
        let mut reasons = crate::deduce::DeduceReasons::new();
        let results = crate::deduce::deduce_with_reasons(fp, state, &mut reasons);
        let fired = results.iter().position(|dr| {
            dr.rule == DeduceRule::MostCommonCountCeil
                && dr.action == DeduceAction::Eliminate { qi: 0, oi: 2 }
        });
        let Some(i) = fired else {
            panic!("MostCommonCountCeil should rule out #1 option C");
        };
        reasons[i]
    }

    fn render_text(steps: &[ExplainStep]) -> String {
        steps
            .iter()
            .filter_map(|s| match s {
                ExplainStep::Simple { text } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn force_only_option_left() {
        let fp = parse_puzzle(&json!({
            "q": [{"t": "AnswerIsSelf"}],
            "o": [[0, 1, 2]],
        }))
        .unwrap();
        // Eliminate B and C, leaving only A. (Rule and reason are irrelevant — this
        // branch is structural, checked before any rule-specific reasoning.)
        let state = State {
            answers: [None; MAX_N],
            eliminated: [0b11110; MAX_N],
        };
        let steps = explain_force(
            &fp,
            &state,
            0,
            Answer::A,
            DeduceRule::CountMustMatchForce,
            DeduceReason::Board,
        );
        assert_eq!(
            steps,
            vec![
                try_looking(&[0]),
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
        // Q2 is B, so the AnswerOf question #1 must be B.
        let state = state_with(&fp, &[None, Some(Answer::B)]);
        let steps = explain_force(
            &fp,
            &state,
            0,
            Answer::B,
            DeduceRule::AnswerOfForward,
            DeduceReason::Source { source: 1 },
        );
        assert_eq!(
            steps,
            vec![
                try_looking(&[0]),
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
        let steps = explain_elimination(
            &fp,
            &state,
            0,
            1,
            DeduceRule::VowelCrossElim,
            DeduceReason::Source { source: 1 },
        );
        assert_eq!(
            steps,
            vec![
                try_looking(&[0]),
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
        let steps = explain_elimination(
            &fp,
            &state,
            0,
            2,
            DeduceRule::FirstClosestAfterEarlierMatch,
            DeduceReason::Source { source: 1 },
        );
        assert_eq!(
            steps,
            vec![
                try_looking(&[0]),
                try_looking(&[0, 1]),
                simple("What if #1 is C?".into()),
                simple(
                    "#1 option C claims the first A is #3, but #2 has answer A and comes before #3."
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
            contradiction: Contradiction::AnswerInvalid,
        };
        let steps = explain_lookahead(&fp, &state, &result);
        assert_eq!(
            steps,
            vec![
                try_looking(&[0]),
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
