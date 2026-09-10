//! Play-time validity: is an answer to question `qi` valid / pending / invalid on
//! the current (possibly partial) board? `check_claim` is the single authority the
//! solver, generator, and UI all share.
//!
//! Scope — *one question against the raw marks*. This checks a single question's
//! own constraint against the board's answers + eliminations. It reads every cell's
//! mark (CountVowel tallies the whole board), but it never interprets another
//! *question's* meaning: combining two questions — e.g. bounding a letter from a
//! sibling `CountAnswer` — is cross-question reasoning, which is deduce's job.
//! check_answer sees such a consequence only once deduce has propagated it into the
//! marks (forced/eliminated cells), never by reading the other question's type.
//!
//! Within that scope it aims to be *complete* — unlike `deduce` (deliberately
//! incomplete, see its module doc). The scope is bounded, so a definite verdict is
//! always reachable: return the strongest one the marks already force — `Invalid`
//! once no completion of the open questions can satisfy the question,
//! `Valid`/`Consistent` once none can break it, `Pending`/`Neutral` only while the
//! outcome is genuinely open. Deduce prunes and propagates; it does not own
//! validity — a deduce rule's validity-style check is *applying* this authority,
//! and its self-elimination of a question's own options must never outrun this
//! verdict.
//!
//! Precondition — the puzzle is **well-formed**. Structural nonsense is
//! `check_form`'s to reject, e.g. an option naming a question that doesn't
//! exist or itself, or a NONE where the kind forbids one. Guards in this code
//! may panic on poorly formed puzzles; those sites are tagged
//! `Fatal check_form error.`

use arrayvec::ArrayVec;

use crate::counts::{
    CountResult, MaskTally, Pred, compute_letter_cells, count_matching, count_matching_mask,
    count_range,
};
use crate::types::*;

/// Play-time verdict for a single question. This is a **wasm wire contract**: the
/// u8 encoding in `lib.rs::validity_to_u8` and its inverse `wasm.ts::validityFromU8`
/// must stay in sync with these variants (and their order is not the wire order —
/// the mapping is explicit on both sides). The two subtle pairs:
/// - `Neutral` vs `Pending`: `Neutral` = unanswered with options still open (nothing
///   to say); `Pending` = answered (or forced) but the truth can't be decided until
///   more answers land.
/// - `Valid` vs `Consistent`: `Valid` = provably correct independent of this
///   question's own answer; `Consistent` = correct only once its own answer is
///   assumed (self-referential, see `maybe_consistent`). `is_valid()` accepts both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Validity {
    Neutral,
    Valid,
    Consistent,
    Invalid,
    Pending,
}

impl Validity {
    pub fn is_valid(self) -> bool {
        matches!(self, Validity::Valid | Validity::Consistent)
    }
}

/// Why a claim is `Invalid`, in the terms prose needs — decided here, by the code that
/// decided the verdict, so `explain` never re-derives a verdict it only has to
/// describe. Carries just what a renderer can't read back off the puzzle and the state:
/// which question or letter is at fault, plus any tally this module computed on the way.
///
/// `explain::rejected_claim_text` renders these — for an answered question, for one of its
/// options, and for a refuted hypothesis. Every variant except `Malformed` and
/// `NoOptionsLeft` must yield a sentence under all three, which `explain`'s
/// `every_invalid_reason_renders` pins.
///
/// Declared through a macro for one reason: it emits `ALL_INVALID_REASON_NAMES` alongside the
/// enum, so that test can require prose from every variant but the two named above without a
/// hand-kept list to fall out of date. Same trick as `deduce`'s rule list.
macro_rules! invalid_reasons {
    (
        $(
            $(#[$meta:meta])*
            $variant:ident $({ $($field:ident : $ty:ty),* $(,)? })?
        ),+ $(,)?
    ) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) enum InvalidReason {
            $(
                $(#[$meta])*
                $variant $({ $($field : $ty),* })?,
            )+
        }

        /// Every `InvalidReason` variant's name, in declaration order.
        #[cfg(test)]
        pub(crate) const ALL_INVALID_REASON_NAMES: &[&str] = &[$(stringify!($variant)),+];
    };
}

invalid_reasons! {
    /// The option value can't mean anything for this kind: a NONE where a number is
    /// required, a letter index past the option count, a position outside the window the
    /// kind scans, a self-reference. `check_form` rejects every one of these as an
    /// `Error`, so a shipped puzzle never carries one and there is no prose for it.
    Malformed,
    /// Not about a claim at all — the question has no answer *and* no options left. The
    /// lookahead hint says this from the elimination that took the last option instead,
    /// via `explain::optionless_detail`.
    NoOptionsLeft,

    /// More questions already match than the claimed count allows: `count` answered, plus
    /// `guaranteed` unanswered ones with no non-matching option left.
    CountFloor { count: u8, guaranteed: u8 },
    /// Too few questions can ever match the claimed count; `max` is the ceiling.
    CountCeiling { max: u8 },
    /// A `MostCommonCount` below what `letter` alone already reaches.
    PeakFloor { letter: Answer, floor: u8 },
    /// A `MostCommonCount` above what any letter can reach.
    PeakCeiling { max: u8 },

    /// The question the claim points at is answered otherwise.
    TargetAnswered { at: u8, answer: Answer },
    /// The question the claim points at can no longer take the letter the claim needs there.
    TargetCannot { at: u8, letter: Answer },
    /// Another question holds a letter the claim reserves for one place — or for nowhere.
    OtherHasLetter { at: u8, letter: Answer },
    /// A question *before* the one the claim named holds the letter, so the named one isn't
    /// the first (or closest-after) question with it. Distinct from `OtherHasLetter`
    /// because the direction is what refutes the claim, and only the scan that found the
    /// question knows it — the renderer must not re-derive it from the indices.
    EarlierHasLetter { at: u8, letter: Answer },
    /// A question *after* the one the claim named holds the letter — mirror of
    /// `EarlierHasLetter`.
    LaterHasLetter { at: u8, letter: Answer },

    /// The `ConsecIdent` pair starting at `at` is answered `first` and `second`.
    PairDiffers {
        at: u8,
        first: Answer,
        second: Answer,
    },
    /// The `ConsecIdent` pair starting at `at` has no letter left it could share.
    PairImpossible { at: u8 },
    /// Another consecutive pair, starting at `at`, already shares its answer.
    OtherPairMatches { at: u8 },

    /// `EqualCount`: the two letters' counts can no longer meet — `short` reaches at most
    /// `short_max`, while `over` is already at `over_min` or more.
    CountsCantMeet {
        short: Answer,
        short_max: u8,
        over: Answer,
        over_min: u8,
    },
    /// `EqualCount`'s "no answer matches" option, refuted by a letter that does match.
    OtherLetterTies { letter: Answer },

    /// `LeastCommon`/`MostCommon`: `rival` is further out than the claimed letter, which
    /// sits at `claimed_count`.
    NotExtremum {
        rival: Answer,
        rival_count: u8,
        claimed_count: u8,
    },
    /// `LeastCommon`/`MostCommon`: `rival` ties the claimed letter, so neither is the
    /// single extreme.
    ExtremumTied { rival: Answer, count: u8 },
    /// `LeastCommon`/`MostCommon` on a partial board: `over` is placed more times than
    /// `short` can still reach. Which side holds the claimed letter flips with the kind —
    /// `short` for `MostCommon`, `over` for `LeastCommon`.
    ///
    /// Bounds, not `NotExtremum`'s settled tallies: routing a still-reachable ceiling
    /// through that variant would render it as a final count.
    ExtremumOutOfReach {
        over: Answer,
        over_min: u8,
        short: Answer,
        short_max: u8,
    },
    /// `LeastCommon`/`MostCommon`: the whole-board pigeonhole rules the claimed letter out — `n`
    /// answers over `oc` letters leave it no room to be the extreme one. No rival letter is
    /// named, which is why `NotExtremum`'s shape does not fit here.
    ///
    /// Both fields change sides with the kind:
    /// - `LeastCommon`: the least common letter can appear at most `threshold` times, and the
    ///   claimed letter is already placed `reach` times.
    /// - `MostCommon`: the most common letter must appear at least `threshold` times, and the
    ///   claimed letter reaches at most `reach` — its placed and still-open cells together.
    ExtremumPigeonhole { threshold: u8, reach: u8 },

    /// `LetterDist`: `at`'s answer sits `actual` letters away, not the claimed distance.
    WrongDistance { at: u8, actual: u8 },
    /// `LetterDist`: no letter at all is the claimed distance from this option, so the
    /// target's answer is irrelevant — `max` is the furthest any letter reaches from here.
    /// Names no question for that reason.
    DistanceUnreachable { max: u8 },
    /// `LetterDist` with `at` unanswered: some letter is the claimed distance away, but none
    /// `at` still has left. `WrongDistance`'s partial-board counterpart.
    NoLetterAtDistance { at: u8 },
}

impl InvalidReason {
    /// Which questions this reason argues from — its own `at` fields, nothing
    /// else. A hint derives its highlight from here rather than tracking one
    /// alongside, so the two can't drift: a stale list would aim the coach at a
    /// question the prose never mentions.
    ///
    /// A pair reason names two questions, `at` and `at + 1`, and returns both.
    /// Exhaustive: a new variant has to say which questions it argues from, if
    /// any.
    pub(crate) fn sources(self) -> ArrayVec<usize, 2> {
        use InvalidReason::*;
        let mut out = ArrayVec::new();
        match self {
            TargetAnswered { at, .. }
            | TargetCannot { at, .. }
            | OtherHasLetter { at, .. }
            | EarlierHasLetter { at, .. }
            | LaterHasLetter { at, .. }
            | WrongDistance { at, .. }
            | NoLetterAtDistance { at } => out.push(usize::from(at)),
            PairDiffers { at, .. } | PairImpossible { at } | OtherPairMatches { at } => {
                out.push(usize::from(at));
                out.push(usize::from(at) + 1);
            }
            // Tallies and bounds argue from the whole board, `DistanceUnreachable`
            // from the alphabet alone, and `ExtremumPigeonhole` from the board's
            // shape. No question to point at.
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
            | DistanceUnreachable { .. } => {}
        }
        out
    }
}

/// A [`Validity`] verdict with, when it is `Invalid`, why — see [`InvalidReason`]. The
/// pairing holds by construction: there is no way to report `Invalid` here without
/// supplying a reason, which is what keeps hint prose from drifting from the verdict it
/// describes. Callers that only want the verdict take [`Validity`] through
/// [`check_claim`] / [`check_answer`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ValidityWithReason {
    Neutral,
    Valid,
    Consistent,
    Pending,
    Invalid(InvalidReason),
}

impl ValidityWithReason {
    pub(crate) fn validity(self) -> Validity {
        match self {
            ValidityWithReason::Neutral => Validity::Neutral,
            ValidityWithReason::Valid => Validity::Valid,
            ValidityWithReason::Consistent => Validity::Consistent,
            ValidityWithReason::Pending => Validity::Pending,
            ValidityWithReason::Invalid(_) => Validity::Invalid,
        }
    }

    /// Why this claim is invalid, or `None` if it isn't.
    pub(crate) fn reason(self) -> Option<InvalidReason> {
        match self {
            ValidityWithReason::Invalid(reason) => Some(reason),
            _ => None,
        }
    }
}

/// The verdict for a value `check_form` rejects — see [`InvalidReason::Malformed`].
const MALFORMED: ValidityWithReason = ValidityWithReason::Invalid(InvalidReason::Malformed);

// ── Helpers ──

fn count_result(cr: MaskTally, ov: OptionValue) -> ValidityWithReason {
    // NONE/UNUSED on a count: malformed but check_answer routes them here for
    // semantic evaluation. Treat as Invalid (the count can never be null).
    if !ov.is_num() {
        return MALFORMED;
    }
    let ov = ov.value();
    // `min` counts forced-unanswered questions too, so ov below it is already exceeded
    // (not just by answered questions); `max` is the ceiling. Valid once pinned.
    if cr.min() > ov {
        ValidityWithReason::Invalid(InvalidReason::CountFloor {
            count: cr.count,
            guaranteed: cr.guaranteed,
        })
    } else if cr.max() < ov {
        ValidityWithReason::Invalid(InvalidReason::CountCeiling { max: cr.max() })
    } else if cr.min() == cr.max() {
        ValidityWithReason::Valid
    } else {
        ValidityWithReason::Pending
    }
}

/// The question a positional claim points at: answered otherwise, or unable to take the
/// letter. `None` when it holds up so far.
fn target_broken(
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    answer: Answer,
    at: usize,
) -> Option<ValidityWithReason> {
    match answers[at] {
        Some(pa) if pa != answer => {
            Some(ValidityWithReason::Invalid(InvalidReason::TargetAnswered {
                at: at as u8,
                answer: pa,
            }))
        }
        None if eliminated[at] & (1u8 << answer.idx()) != 0 => {
            Some(ValidityWithReason::Invalid(InvalidReason::TargetCannot {
                at: at as u8,
                letter: answer,
            }))
        }
        _ => None,
    }
}

fn first_in_range(
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    answer: Answer,
    start: usize,
    end: usize,
    pos: OptionValue,
) -> ValidityWithReason {
    let amask = 1u8 << answer.idx();
    if pos.is_num() {
        let p = pos.value() as usize;
        if p < start || p >= end {
            return MALFORMED;
        }
        if let Some(broken) = target_broken(answers, eliminated, answer, p) {
            return broken;
        }
        let mut all_certain = true;
        for j in start..p {
            if answers[j] == Some(answer) {
                // Before `p` by construction — the direction is the loop's, not something
                // the renderer should recover by comparing indices.
                return ValidityWithReason::Invalid(InvalidReason::EarlierHasLetter {
                    at: j as u8,
                    letter: answer,
                });
            }
            if answers[j].is_none() && eliminated[j] & amask == 0 {
                all_certain = false;
            }
        }
        if answers[p] == Some(answer) && all_certain {
            ValidityWithReason::Valid
        } else {
            ValidityWithReason::Pending
        }
    } else {
        none_in_range(answers, eliminated, answer, start, end)
    }
}

fn last_in_range(
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    answer: Answer,
    start: usize,
    end: usize,
    pos: OptionValue,
) -> ValidityWithReason {
    let amask = 1u8 << answer.idx();
    if pos.is_num() {
        let p = pos.value() as usize;
        if p < start || p >= end {
            return MALFORMED;
        }
        if let Some(broken) = target_broken(answers, eliminated, answer, p) {
            return broken;
        }
        let mut all_certain = true;
        for j in (p + 1)..end {
            if answers[j] == Some(answer) {
                // After `p` by construction — see `first_in_range`.
                return ValidityWithReason::Invalid(InvalidReason::LaterHasLetter {
                    at: j as u8,
                    letter: answer,
                });
            }
            if answers[j].is_none() && eliminated[j] & amask == 0 {
                all_certain = false;
            }
        }
        if answers[p] == Some(answer) && all_certain {
            ValidityWithReason::Valid
        } else {
            ValidityWithReason::Pending
        }
    } else {
        none_in_range(answers, eliminated, answer, start, end)
    }
}

/// The NONE ("no question in `start..end` has `answer`") arm shared by
/// `first_in_range`/`last_in_range`: Invalid if one already does, Pending if one
/// still could, else Valid. The claim names no position, so a match refutes it by
/// existing — `OtherHasLetter`, not the directional variants.
fn none_in_range(
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    answer: Answer,
    start: usize,
    end: usize,
) -> ValidityWithReason {
    let amask = 1u8 << answer.idx();
    let mut could_exist = false;
    for j in start..end {
        if answers[j] == Some(answer) {
            return other_has_letter(j, answer);
        }
        if answers[j].is_none() && eliminated[j] & amask == 0 {
            could_exist = true;
        }
    }
    if could_exist {
        ValidityWithReason::Pending
    } else {
        ValidityWithReason::Valid
    }
}

/// `Invalid` because `at` already holds `letter`, which the claim reserves for one
/// place — or for nowhere.
fn other_has_letter(at: usize, letter: Answer) -> ValidityWithReason {
    ValidityWithReason::Invalid(InvalidReason::OtherHasLetter {
        at: at as u8,
        letter,
    })
}

fn all_answered(answers: &[Option<Answer>; MAX_N], n: usize) -> bool {
    (0..n).all(|i| answers[i].is_some())
}

fn count_answer_simple(
    answers: &[Option<Answer>; MAX_N],
    target: Answer,
    from: usize,
    to: usize,
) -> u8 {
    let mut c: u8 = 0;
    for i in from..to {
        if answers[i] == Some(target) {
            c += 1;
        }
    }
    c
}

fn fill_counts(answers: &[Option<Answer>; MAX_N], n: usize) -> [u8; 5] {
    let mut counts = [0u8; 5];
    for i in 0..n {
        if let Some(a) = answers[i] {
            counts[a.idx()] += 1;
        }
    }
    counts
}

// ── Main function ──

/// Shared implementation behind [`check_claim`] and [`check_claim_fast`]. The
/// match only ever reads two things off a puzzle: its question count `n` and
/// option count `oc` — so it's parameterized on those instead of a
/// `&FlatPuzzle`, letting `check_claim_fast` build a throwaway `State` from a
/// flat `&[Answer]` slice without needing a whole `FlatPuzzle`.
///
/// This is NOT a wellformedness check. Although it incidentally short-circuits
/// on some structural issues (parity mismatch, value out of [0..=4] for letter
/// types), it assumes the claim is already well-formed and behaves unpredictably
/// otherwise (e.g. AnswerOf with an out-of-range `question_index` panics on
/// `answers[...]`). For form checks, see `check_form::check_claim_form`.
// Inlined on native so both wrappers can force the whole match into their call
// sites (`check_claim_fast` is the generator's hot path); outlined on wasm
// where every duplicated body shows up in the download.
#[cfg_attr(not(target_arch = "wasm32"), inline(always))]
fn check_claim_impl(
    n: usize,
    oc: usize,
    state: State,
    opt: OptionPos,
    claim: Claim,
) -> ValidityWithReason {
    let qt = &claim.question_type;
    let ov = claim.value;
    let qi = opt.qi;
    let self_oi = opt.oi;
    let self_letter = Answer::from(self_oi as u8);
    let answers = &state.answers;
    let eliminated = &state.eliminated;

    match *qt {
        // ── Counting ──
        QuestionType::CountAnswer { answer }
        | QuestionType::CountAnswerBefore { answer, .. }
        | QuestionType::CountAnswerAfter { answer, .. } => {
            let (from, to) = count_range(qt, n);
            let cr =
                count_matching_mask(answers, eliminated, Pred::IsAnswer(answer).mask(), from, to);
            count_result(cr, ov)
        }

        QuestionType::CountVowel => {
            let cr = count_matching_mask(answers, eliminated, Pred::IsVowel.mask(), 0, n);
            count_result(cr, ov)
        }

        QuestionType::CountConsonant => {
            let cr = count_matching_mask(answers, eliminated, Pred::IsConsonant.mask(), 0, n);
            count_result(cr, ov)
        }

        QuestionType::MostCommonCount => {
            if !ov.is_num() {
                return MALFORMED;
            }
            let ov = ov.value();
            // The most-common count is `ov` iff some letter can reach it
            // (ov <= max_possible) and none is already forced above it
            // (ov >= max_known). Valid only once the maximum is pinned to a single
            // value (max_known == max_possible).
            let mut peak = Answer::A;
            let mut max_known = 0u8;
            let mut max_possible = 0u8;
            for li in 0..oc {
                let cr = count_matching_mask(answers, eliminated, 1 << li, 0, n);
                if cr.min() > max_known {
                    max_known = cr.min();
                    peak = Answer::from(li as u8);
                }
                max_possible = max_possible.max(cr.max());
            }
            if ov < max_known {
                ValidityWithReason::Invalid(InvalidReason::PeakFloor {
                    letter: peak,
                    floor: max_known,
                })
            } else if ov > max_possible {
                ValidityWithReason::Invalid(InvalidReason::PeakCeiling { max: max_possible })
            } else if max_known == max_possible {
                ValidityWithReason::Valid
            } else {
                ValidityWithReason::Pending
            }
        }

        // ── Positional: first/closest-after ──
        QuestionType::FirstWith { answer } => first_in_range(answers, eliminated, answer, 0, n, ov),
        QuestionType::ClosestAfter {
            after_index,
            answer,
        } => first_in_range(answers, eliminated, answer, after_index as usize + 1, n, ov),

        // ── Positional: last/closest-before ──
        QuestionType::LastWith { answer } => last_in_range(answers, eliminated, answer, 0, n, ov),
        QuestionType::ClosestBefore {
            before_index,
            answer,
        } => last_in_range(answers, eliminated, answer, 0, before_index as usize, ov),

        // ── Reference ──
        QuestionType::AnswerOf { question_index } => {
            if !ov.is_num() || ov.value() > 4 {
                return MALFORMED;
            }
            let k = question_index as usize;
            match answers[k] {
                Some(target) => {
                    if target as u8 == ov.value() {
                        ValidityWithReason::Valid
                    } else {
                        ValidityWithReason::Invalid(InvalidReason::TargetAnswered {
                            at: k as u8,
                            answer: target,
                        })
                    }
                }
                // The target can never take the claimed letter if it's eliminated there.
                None if eliminated[k] & (1u8 << ov.value()) != 0 => {
                    ValidityWithReason::Invalid(InvalidReason::TargetCannot {
                        at: k as u8,
                        letter: Answer::from(ov.value()),
                    })
                }
                None => ValidityWithReason::Pending,
            }
        }

        // The target's answer decides it outright; failing that, its remaining options still
        // can. Mirrors `deduce`'s `LetterDistImpossible` / `LetterDistNoMatch` split.
        QuestionType::LetterDist { question_index } => {
            if !ov.is_num() {
                return MALFORMED;
            }
            let target = question_index as usize;
            match answers[target] {
                Some(other) => {
                    let dist = (self_oi as u8).abs_diff(other as u8);
                    if dist == ov.value() {
                        ValidityWithReason::Valid
                    } else {
                        ValidityWithReason::Invalid(InvalidReason::WrongDistance {
                            at: question_index,
                            actual: dist,
                        })
                    }
                }
                None => {
                    // Furthest any letter sits from `self_oi`; `saturating_sub` so a
                    // malformed `self_oi >= oc` can't underflow it.
                    let max_dist = self_oi.max(oc.saturating_sub(1 + self_oi)) as u8;
                    if ov.value() > max_dist {
                        // Further than any letter sits from this one, so the target never
                        // mattered.
                        ValidityWithReason::Invalid(InvalidReason::DistanceUnreachable {
                            max: max_dist,
                        })
                    } else {
                        let reachable = (0..oc).any(|letter| {
                            eliminated[target] & (1u8 << letter) == 0
                                && (self_oi as u8).abs_diff(letter as u8) == ov.value()
                        });
                        if reachable {
                            ValidityWithReason::Pending
                        } else {
                            // Some letter is that far off, but none the target still has left.
                            ValidityWithReason::Invalid(InvalidReason::NoLetterAtDistance {
                                at: question_index,
                            })
                        }
                    }
                }
            }
        }

        // Scoped sameness — never a claim. Fatal `check_form` error.
        QuestionType::OnlySameAmong | QuestionType::OnlySameAsAmong { .. } => {
            unreachable!("scoped sameness is checked as a question, never as a claim")
        }

        // ── NoOtherHasAnswer: "not the answer to any OTHER question" ──
        QuestionType::NoOtherHasAnswer => {
            if !ov.is_num() || ov.value() > 4 {
                return MALFORMED;
            }
            let letter = Answer::from(ov.value());
            let amask = 1u8 << ov.value();
            let mut other: Option<usize> = None;
            let mut could_match: u8 = 0;
            for j in 0..n {
                if j == qi {
                    continue;
                }
                match answers[j] {
                    Some(x) if x == letter => other = other.or(Some(j)),
                    None if eliminated[j] & amask == 0 => could_match += 1,
                    _ => {}
                }
            }
            if let Some(j) = other {
                other_has_letter(j, letter)
            } else if could_match == 0 {
                ValidityWithReason::Valid
            } else {
                ValidityWithReason::Pending
            }
        }

        // ── Previous/Next same ──
        // No pre-range check: first_in_range/last_in_range already reject a numeric
        // position outside `start..end` (here `0..qi` / `qi+1..n`).
        QuestionType::PrevSame => last_in_range(answers, eliminated, self_letter, 0, qi, ov),

        QuestionType::NextSame => first_in_range(answers, eliminated, self_letter, qi + 1, n, ov),

        // ── Only same ──
        QuestionType::OnlySame => check_whole_board_sameness(n, state, ov, self_letter, qi),

        QuestionType::OnlySameAs { question_index } => {
            let source = usize::from(question_index);
            // Fatal `check_form` error.
            assert!(
                source < n && source != qi,
                "OnlySameAs references {source} (qi={qi})"
            );
            let Some(matched) = answers[source] else {
                return ValidityWithReason::Pending;
            };
            check_whole_board_sameness(n, state, ov, matched, source)
        }

        // ── Consecutive identical ──
        QuestionType::ConsecIdent => {
            if ov.is_num() {
                let ov = ov.value() as usize;
                if ov + 1 >= n {
                    return MALFORMED;
                }

                if let (Some(pa), Some(pb)) = (answers[ov], answers[ov + 1])
                    && pa != pb
                {
                    return ValidityWithReason::Invalid(InvalidReason::PairDiffers {
                        at: ov as u8,
                        first: pa,
                        second: pb,
                    });
                }

                // Three ways the pair can no longer be made to match: no shared option
                // left, or one side answered a letter the other has eliminated.
                let poss_a = !eliminated[ov] & ALL_OPTIONS_MASK;
                let poss_b = !eliminated[ov + 1] & ALL_OPTIONS_MASK;
                let impossible = poss_a & poss_b == 0
                    || answers[ov].is_some_and(|pa| is_eliminated(eliminated, ov + 1, pa.idx()))
                    || answers[ov + 1].is_some_and(|pb| is_eliminated(eliminated, ov, pb.idx()));
                if impossible {
                    return ValidityWithReason::Invalid(InvalidReason::PairImpossible {
                        at: ov as u8,
                    });
                }

                let mut other_pair: Option<usize> = None;
                let mut uncertain_pairs: u8 = 0;
                for j in 0..n.saturating_sub(1) {
                    if j == ov {
                        continue;
                    }
                    match (answers[j], answers[j + 1]) {
                        (Some(x), Some(y)) if x == y => other_pair = other_pair.or(Some(j)),
                        (Some(_), Some(_)) => {}
                        _ => uncertain_pairs += 1,
                    }
                }

                if let Some(j) = other_pair {
                    return ValidityWithReason::Invalid(InvalidReason::OtherPairMatches {
                        at: j as u8,
                    });
                }

                if let (Some(pa), Some(pb)) = (answers[ov], answers[ov + 1])
                    && pa == pb
                    && uncertain_pairs == 0
                {
                    return ValidityWithReason::Valid;
                }

                ValidityWithReason::Pending
            } else if ov.is_none() {
                let mut pair: Option<usize> = None;
                let mut any_uncertain = false;
                for j in 0..n.saturating_sub(1) {
                    match (answers[j], answers[j + 1]) {
                        (Some(x), Some(y)) if x == y => pair = pair.or(Some(j)),
                        (Some(_), Some(_)) => {}
                        _ => any_uncertain = true,
                    }
                }
                if let Some(j) = pair {
                    ValidityWithReason::Invalid(InvalidReason::OtherPairMatches { at: j as u8 })
                } else if any_uncertain {
                    ValidityWithReason::Pending
                } else {
                    ValidityWithReason::Valid
                }
            } else {
                MALFORMED
            }
        }

        // ── Only odd / only even ──
        QuestionType::OnlyOdd { answer } | QuestionType::OnlyEven { answer } => {
            let parity = match *qt {
                QuestionType::OnlyOdd { .. } => 1,
                _ => 0,
            };
            let amask = 1u8 << answer.idx();

            if ov.is_num() {
                let ov = ov.value() as usize;
                if (ov + 1) % 2 != parity {
                    return MALFORMED;
                }

                if let Some(broken) = target_broken(answers, eliminated, answer, ov) {
                    return broken;
                }

                let mut other: Option<usize> = None;
                let mut other_remaining: u8 = 0;
                for j in 0..n {
                    if j == ov || (j + 1) % 2 != parity {
                        continue;
                    }
                    match answers[j] {
                        Some(x) if x == answer => other = other.or(Some(j)),
                        None if eliminated[j] & amask == 0 => other_remaining += 1,
                        _ => {}
                    }
                }

                if let Some(j) = other {
                    return other_has_letter(j, answer);
                }
                if answers[ov] == Some(answer) && other_remaining == 0 {
                    ValidityWithReason::Valid
                } else {
                    ValidityWithReason::Pending
                }
            } else if ov.is_none() {
                let mut matched: Option<usize> = None;
                let mut any_could = false;
                for j in 0..n {
                    if (j + 1) % 2 != parity {
                        continue;
                    }
                    if answers[j] == Some(answer) {
                        matched = matched.or(Some(j));
                    }
                    if answers[j].is_none() && eliminated[j] & amask == 0 {
                        any_could = true;
                    }
                }
                if let Some(j) = matched {
                    other_has_letter(j, answer)
                } else if any_could {
                    ValidityWithReason::Pending
                } else {
                    ValidityWithReason::Valid
                }
            } else {
                MALFORMED
            }
        }

        // ── Equal count ──
        QuestionType::EqualCount { answer } => {
            if ov.is_num() {
                if ov.value() as usize >= oc {
                    return MALFORMED;
                }
                let claimed = Answer::from(ov.value());
                // Fatal `check_form` error.
                assert!(claimed != answer, "EqualCount({answer}) points to itself");
                let CountResult {
                    count: rc,
                    remaining: rr,
                } = count_matching(answers, eliminated, Pred::IsAnswer(answer), 0, n);
                let CountResult {
                    count: sc,
                    remaining: sr,
                } = count_matching(answers, eliminated, Pred::IsAnswer(claimed), 0, n);
                let cant_meet = |short, short_max, over, over_min| {
                    ValidityWithReason::Invalid(InvalidReason::CountsCantMeet {
                        short,
                        short_max,
                        over,
                        over_min,
                    })
                };
                if rc + rr < sc {
                    return cant_meet(answer, rc + rr, claimed, sc);
                }
                if sc + sr < rc {
                    return cant_meet(claimed, sc + sr, answer, rc);
                }
                if rr == 0 && sr == 0 {
                    // The two guards above already rejected every inequality reachable
                    // here, so the `cant_meet` arm is a formality.
                    return if rc == sc {
                        ValidityWithReason::Valid
                    } else if rc < sc {
                        cant_meet(answer, rc, claimed, sc)
                    } else {
                        cant_meet(claimed, sc, answer, rc)
                    };
                }
                ValidityWithReason::Pending
            } else if ov.is_none() {
                if !all_answered(answers, n) {
                    return ValidityWithReason::Pending;
                }
                let ref_count = count_answer_simple(answers, answer, 0, n);
                let tie = LETTERS[..oc]
                    .iter()
                    .find(|&&l| l != answer && count_answer_simple(answers, l, 0, n) == ref_count);
                match tie {
                    Some(&letter) => {
                        ValidityWithReason::Invalid(InvalidReason::OtherLetterTies { letter })
                    }
                    None => ValidityWithReason::Valid,
                }
            } else {
                MALFORMED
            }
        }

        // ── Global: a full board pins the extreme, a partial one can still refute it ──
        QuestionType::LeastCommon | QuestionType::MostCommon => {
            if !ov.is_num() || ov.value() as usize >= oc {
                return MALFORMED;
            }
            let ov = ov.value() as usize;
            let least = matches!(*qt, QuestionType::LeastCommon);
            if !all_answered(answers, n) {
                // Cell bounds alone: a letter out of the claimed letter's reach settles the
                // claim under every completion. Counted off the state as given — a "what if
                // `qi` were `oi`" is the caller's to encode, and adjusting for it here would
                // apply it twice.
                let cells = compute_letter_cells(answers, eliminated, n);
                let out_of_reach = (0..oc).filter(|&rival| rival != ov).find_map(|rival| {
                    // Least needs the claim at or below the rival, so the claim's floor
                    // passing the rival's ceiling kills it; Most is the mirror.
                    let (over, over_min, short, short_max) = if least {
                        (ov, cells.filled[ov], rival, cells.cell_max(rival))
                    } else {
                        (rival, cells.filled[rival], ov, cells.cell_max(ov))
                    };
                    // Strict, so equal bounds stay `Pending`: a forced tie refutes a *unique*
                    // extremum too, but the clause would read as two matching numbers rather
                    // than as a refutation. Deduce draws the line in the same place.
                    (over_min > short_max).then_some(InvalidReason::ExtremumOutOfReach {
                        over: Answer::from(over as u8),
                        over_min,
                        short: Answer::from(short as u8),
                        short_max,
                    })
                });
                // No rival out of reach, so the board's own arithmetic instead: `n` answers over
                // `oc` letters bound what the extreme letter's count can be, and the claimed
                // letter already sits outside that bound, in every completion. Which side counts
                // as outside flips with the kind — see `ExtremumPigeonhole`. Cells only: folding
                // a sibling count question's bound in here would be cross-question reasoning,
                // which is deduce's job.
                let pigeonhole = || {
                    if oc < 2 {
                        return None;
                    }
                    let (threshold, reach, ruled_out) = if least {
                        // Uniquely least at k needs the other oc-1 letters strictly above it,
                        // so n >= k + (k + 1)(oc - 1), i.e. k <= (n + 1 - oc) / oc.
                        if n + 1 < oc {
                            return None;
                        }
                        let threshold = ((n + 1 - oc) / oc) as u8;
                        let reach = cells.filled[ov];
                        (threshold, reach, reach > threshold)
                    } else {
                        // Mirror: uniquely most at k needs the rest strictly below, so
                        // n <= k + (k - 1)(oc - 1), i.e. k >= ceil((n + oc - 1) / oc) — which is
                        // what the integer division spells as (n + 2 * oc - 2) / oc.
                        let threshold = ((n + 2 * oc - 2) / oc) as u8;
                        let reach = cells.cell_max(ov);
                        (threshold, reach, reach < threshold)
                    };
                    ruled_out.then_some(InvalidReason::ExtremumPigeonhole { threshold, reach })
                };
                match out_of_reach.or_else(pigeonhole) {
                    Some(reason) => ValidityWithReason::Invalid(reason),
                    None => ValidityWithReason::Pending,
                }
            } else {
                let c = fill_counts(answers, n);
                let extreme = if least {
                    c[..oc].iter().copied().min()
                } else {
                    c[..oc].iter().copied().max()
                }
                .unwrap_or(0);
                // The claimed letter has to hold the extreme, and hold it alone — so any
                // other letter at `extreme` refutes it, either by beating it or by tying.
                let rival = (0..oc)
                    .find(|&li| li != ov && c[li] == extreme)
                    .map(|li| Answer::from(li as u8));
                match (c[ov] == extreme, rival) {
                    (true, None) => ValidityWithReason::Valid,
                    (true, Some(rival)) => {
                        ValidityWithReason::Invalid(InvalidReason::ExtremumTied {
                            rival,
                            count: extreme,
                        })
                    }
                    // `extreme` is the min/max over `0..oc`, so when the claimed letter
                    // isn't holding it another letter is.
                    (false, rival) => ValidityWithReason::Invalid(InvalidReason::NotExtremum {
                        rival: rival.expect("some letter holds the extreme count"),
                        rival_count: extreme,
                        claimed_count: c[ov],
                    }),
                }
            }
        }

        // ── Always valid ──
        QuestionType::AnswerIsSelf => ValidityWithReason::Valid,

        // TrueStmt can't be checked via check_claim
        QuestionType::TrueStmt => ValidityWithReason::Pending,
    }
}

/// Evaluate the **semantic truth** of a claim against the current puzzle state.
/// Returns `Valid`/`Invalid`/`Pending` analogous to `check_answer`. See
/// `check_claim_impl` for the implementation and its caveats.
pub(crate) fn check_claim(fp: &FlatPuzzle, state: State, opt: OptionPos, claim: Claim) -> Validity {
    check_claim_with_reason(fp, state, opt, claim).validity()
}

/// [`check_claim`] with the reason attached — see [`ValidityWithReason`]. `explain` renders the
/// reason; everything else wants the bare verdict.
pub(crate) fn check_claim_with_reason(
    fp: &FlatPuzzle,
    state: State,
    opt: OptionPos,
    claim: Claim,
) -> ValidityWithReason {
    check_claim_impl(fp.n, fp.option_count, state, opt, claim)
}

/// Check an `OnlySame` / `OnlySameAs` answer. Both ask which question — over the
/// whole board, not a candidate list — is the only one *other than* `source` answered
/// with the matched letter M: a numeric option asserts that its target holds M and that
/// no other question does, the "none" option asserts only the latter.
///
/// `source` is the question M is read off: `qi` for `OnlySame`, the reference for
/// `OnlySameAs`. It's the one question excluded from the scan, holding M by
/// definition; `qi` is scanned like any other for `OnlySameAs`, so its own answer
/// can refute the claim.
fn check_whole_board_sameness(
    n: usize,
    state: State,
    ov: OptionValue,
    matched: Answer,
    source: usize,
) -> ValidityWithReason {
    let answers = &state.answers;
    let eliminated = &state.eliminated;
    let amask = 1u8 << matched.idx();

    // Whether any question other than `source` (and the claimed `target`, when there
    // is one) holds M, and how many could still take it.
    let scan = |target: Option<usize>| {
        let mut other: Option<usize> = None;
        let mut could_match: u8 = 0;
        for j in 0..n {
            if j == source || target == Some(j) {
                continue;
            }
            match answers[j] {
                Some(x) if x == matched => other = other.or(Some(j)),
                None if eliminated[j] & amask == 0 => could_match += 1,
                _ => {}
            }
        }
        (other, could_match)
    };

    if ov.is_none() {
        let (other, could_match) = scan(None);
        if let Some(j) = other {
            other_has_letter(j, matched)
        } else if could_match == 0 {
            ValidityWithReason::Valid
        } else {
            ValidityWithReason::Pending
        }
    } else if !ov.is_num() || usize::from(ov.value()) >= n {
        MALFORMED
    } else {
        let target = usize::from(ov.value());
        if target == source {
            return MALFORMED;
        }
        if let Some(broken) = target_broken(answers, eliminated, matched, target) {
            return broken;
        }
        let (other, other_remaining) = scan(Some(target));
        if let Some(j) = other {
            return other_has_letter(j, matched);
        }
        if answers[target] == Some(matched) && other_remaining == 0 {
            ValidityWithReason::Valid
        } else {
            ValidityWithReason::Pending
        }
    }
}

/// Check a `OnlySameAmong` / `OnlySameAsAmong` answer. Both list a candidate set and ask which
/// member is the **only** one answered with the matched letter M: a numeric option
/// asserts that its target holds M *and* that no other listed candidate does; the
/// "none" option asserts only the latter, over the whole list.
///
/// `source` is the question M is read off — `qi` for `OnlySameAmong`, the reference for
/// `OnlySameAsAmong` — so nothing is decided until it's answered. It is also the one value
/// excluded from the candidate list, holding M by definition. `qi` stays a candidate
/// for `OnlySameAsAmong`, where matching the reference is an ordinary proposition for it.
fn check_scoped_sameness(
    fp: &FlatPuzzle,
    state: State,
    qi: usize,
    answer: Answer,
    source: usize,
) -> ValidityWithReason {
    let Some(matched) = state.answers[source] else {
        return ValidityWithReason::Pending;
    };
    let amask = 1u8 << matched.idx();
    // Two independent cases: `eliminated` doesn't track answers, so a question
    // differs either by being answered otherwise or by having M eliminated.
    let known_differs = |j: usize| match state.answers[j] {
        Some(other) => other != matched,
        None => state.eliminated[j] & amask != 0,
    };

    let selected = fp.options[qi][answer.idx()];
    let target = if selected.is_num() {
        let target = usize::from(selected.value());
        // Fatal `check_form` error.
        assert!(
            target < fp.n && target != qi && target != source,
            "scoped-sameness option names {target} (qi={qi}, source={source})"
        );
        // First requirement: the target must hold M.
        if let Some(broken) = target_broken(&state.answers, &state.eliminated, matched, target) {
            return broken;
        }
        Some(target)
    } else if selected.is_none() {
        None
    } else {
        // An unfilled option slot is undecided, not wrong.
        return ValidityWithReason::Pending;
    };

    // Second requirement: no *other* listed candidate may hold M.
    let mut others_settled = true;
    for oi in 0..fp.option_count {
        let ov = fp.options[qi][oi];
        if !ov.is_num() {
            continue;
        }
        let j = usize::from(ov.value());
        // Out of range names no real question, so there's no candidate to check.
        if j >= fp.n || j == source || Some(j) == target {
            continue;
        }
        if state.answers[j] == Some(matched) {
            return other_has_letter(j, matched);
        }
        if !known_differs(j) {
            others_settled = false;
        }
    }

    // The "none" option names no target, so the first requirement doesn't apply.
    let target_shares = target.is_none_or(|t| state.answers[t] == Some(matched));
    if others_settled && target_shares {
        ValidityWithReason::Valid
    } else {
        ValidityWithReason::Pending
    }
}

fn affected_by_own_answer(qt: &QuestionType, qi: usize) -> bool {
    match *qt {
        QuestionType::AnswerOf { question_index } => question_index as usize == qi,
        QuestionType::OnlySameAsAmong { question_index } => question_index as usize == qi,
        _ => true,
    }
}

fn maybe_consistent(
    result: ValidityWithReason,
    qt: &QuestionType,
    qi: usize,
) -> ValidityWithReason {
    if result == ValidityWithReason::Valid && affected_by_own_answer(qt, qi) {
        ValidityWithReason::Consistent
    } else {
        result
    }
}

/// The value question `qi`'s option `ai` asserts.
fn claim_value(fp: &FlatPuzzle, qi: usize, ai: usize) -> OptionValue {
    let ov = fp.options[qi][ai];
    // Fatal check_form error, or an answer past `option_count`.
    assert!(!ov.is_unused(), "Q{} has no option {ai}", qi + 1);
    ov
}

/// The claim question `qi`'s current answer commits to: the statement it picked for a
/// `TrueStmt`, its own type and selected option value otherwise. `None` if `qi` is
/// unanswered (or the picked slot carries no statement). [`check_answer_with_reason`] checks exactly
/// this claim, so `explain` can render exactly it without guessing.
pub(crate) fn answered_claim(fp: &FlatPuzzle, state: &State, qi: usize) -> Option<Claim> {
    let ai = state.answers[qi]?.idx();
    let qt = fp.question_types[qi];
    if matches!(qt, QuestionType::TrueStmt) {
        return fp.claim_at(qi, ai);
    }
    Some(Claim {
        question_type: qt,
        value: claim_value(fp, qi, ai),
    })
}

pub fn check_answer(fp: &FlatPuzzle, state: State, qi: usize) -> Validity {
    check_answer_with_reason(fp, state, qi).validity()
}

/// [`check_answer`] with the reason attached — see [`ValidityWithReason`]. The reason describes
/// [`answered_claim`]'s claim, which for a `TrueStmt` is the statement it picked rather
/// than the question's own type.
pub(crate) fn check_answer_with_reason(
    fp: &FlatPuzzle,
    state: State,
    qi: usize,
) -> ValidityWithReason {
    let Some(a) = state.answers[qi] else {
        let oc = fp.option_count;
        if (!state.eliminated[qi] & ((1 << oc) - 1)) == 0 {
            return ValidityWithReason::Invalid(InvalidReason::NoOptionsLeft);
        }
        return ValidityWithReason::Neutral;
    };
    let ai = a.idx();
    let qt = &fp.question_types[qi];

    if matches!(qt, QuestionType::TrueStmt) {
        let Some(selected_claim) = fp.claim_at(qi, ai) else {
            return MALFORMED;
        };
        let selected = check_claim_with_reason(fp, state, OptionPos { qi, oi: ai }, selected_claim);
        if selected != ValidityWithReason::Valid {
            return selected;
        }
        for oi in 0..fp.option_count {
            if oi == ai {
                continue;
            }
            let mut hyp = state;
            hyp.answers[qi] = Some(Answer::from(oi as u8));
            if check_claim_with_reason(fp, hyp, OptionPos { qi, oi }, selected_claim)
                != ValidityWithReason::Valid
            {
                return ValidityWithReason::Consistent;
            }
        }
        return ValidityWithReason::Valid;
    }

    // The scoped-sameness types are checked here rather than through
    // `check_claim`, which can't see the candidate list. `OnlySameAmong` comes back
    // `Consistent` (via `maybe_consistent`) because its matched letter *is* qi's
    // own answer; `OnlySameAsAmong` takes it from another question, so it comes back
    // `Valid`.
    match *qt {
        QuestionType::OnlySameAmong => {
            let verdict = check_scoped_sameness(fp, state, qi, a, qi);
            return maybe_consistent(verdict, qt, qi);
        }
        QuestionType::OnlySameAsAmong { question_index } => {
            let verdict = check_scoped_sameness(fp, state, qi, a, usize::from(question_index));
            return maybe_consistent(verdict, qt, qi);
        }
        _ => {}
    }

    let claim = Claim {
        question_type: *qt,
        value: claim_value(fp, qi, ai),
    };
    let verdict = check_claim_with_reason(fp, state, OptionPos { qi, oi: ai }, claim);
    maybe_consistent(verdict, qt, qi)
}

pub(crate) fn check_all_answers(fp: &FlatPuzzle, answers: &[Option<Answer>; MAX_N]) -> bool {
    let state = State {
        answers: *answers,
        eliminated: [fp.initial_eliminated_mask(); MAX_N],
    };
    (0..fp.n).all(|qi| check_answer(fp, state, qi).is_valid())
}

/// Like `check_claim`, but assumes `answers` is fully populated; returns bool.
/// Same caveat applies: this is a **semantic** check (does the claim hold given
/// these answers?), not a wellformedness check.
///
/// Builds a throwaway fully-answered `State` from the flat slice and defers to
/// `check_claim_impl`. Eliminated bits only ever gate `None` answer slots there,
/// so leaving them at `State::initial`'s phantom-only mask (rather than
/// reconstructing "everything but the known answer") is sound — every slot up
/// to `n` is `Some`. Equivalence to `check_claim` is pinned by
/// `tests::check_claim_fast_matches_check_claim`.
// Inlined on native for the generator's inner loop; outlined on wasm
// where every duplicated body shows up in the download.
#[cfg_attr(not(target_arch = "wasm32"), inline(always))]
pub(crate) fn check_claim_fast(
    option_count: usize,
    answers: &[Answer],
    qi: usize,
    claim: &Claim,
) -> bool {
    let n = answers.len();
    let mut state = State::initial(option_count);
    state.answers[..n]
        .iter_mut()
        .zip(answers)
        .for_each(|(slot, &a)| *slot = Some(a));
    let opt = OptionPos {
        qi,
        oi: answers[qi].idx(),
    };
    check_claim_impl(n, option_count, state, opt, *claim)
        .validity()
        .is_valid()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn test_shared_check_answer() {
        let json_str = std::fs::read_to_string("../tests/check-answer.json")
            .expect("can't read tests/check-answer.json");
        let suite: Value = serde_json::from_str(&json_str).unwrap();
        let tests = suite["tests"].as_array().unwrap();

        let mut passed = 0;
        let mut failed = 0;

        for test in tests {
            if test.get("section").is_some() {
                continue;
            }
            let name = test["name"].as_str().unwrap();
            let qi = test["qi"].as_u64().unwrap() as usize;
            let states = test["state"].as_array().unwrap();
            let expect = test["expect"].as_str().unwrap();

            let fp = crate::serialize::parse_puzzle(&test["puzzle"]);
            let Some(fp) = fp else {
                eprintln!("SKIP: {name}: parse failed");
                continue;
            };

            let n = fp.n;
            let mut answers: [Option<Answer>; MAX_N] = [None; MAX_N];
            let mut eliminated = fp.initial_state.eliminated;
            for i in 0..n {
                let s = states[i].as_str().unwrap_or("");
                for ch in s.chars() {
                    if ch.is_ascii_uppercase() {
                        let oi = (ch as u8 - b'A') as usize;
                        answers[i] = Some(Answer::from(oi as u8));
                        eliminated[i] = ALL_OPTIONS_MASK ^ (1 << oi);
                    } else if ch.is_ascii_lowercase() {
                        let oi = (ch as u8 - b'a') as usize;
                        eliminated[i] |= 1 << oi;
                    }
                }
            }

            let got = check_answer(
                &fp,
                State {
                    answers,
                    eliminated,
                },
                qi,
            );
            let got_str = match got {
                Validity::Neutral => "neutral",
                Validity::Valid => "valid",
                Validity::Consistent => "consistent",
                Validity::Invalid => "invalid",
                Validity::Pending => "pending",
            };

            if got_str == expect {
                passed += 1;
            } else {
                failed += 1;
                eprintln!("FAIL: {name}");
                eprintln!("  expected: {expect}");
                eprintln!("  got:      {got_str}");
            }
        }

        eprintln!("{passed}/{} passed", passed + failed);
        assert_eq!(failed, 0, "{failed} test(s) failed");
    }

    #[test]
    fn test_shared_evaluators() {
        let json_str = std::fs::read_to_string("../tests/evaluate.json")
            .expect("can't read tests/evaluate.json");
        let suite: Value = serde_json::from_str(&json_str).unwrap();
        let tests = suite["tests"].as_array().unwrap();

        let mut passed = 0;
        let mut failed = 0;

        for test in tests {
            if test.get("section").is_some() {
                continue;
            }
            let name = test["name"].as_str().unwrap();
            let qi = test["qi"].as_u64().unwrap() as usize;
            let expect = test["expect"].as_bool().unwrap();

            let fp = crate::serialize::parse_puzzle(&test["puzzle"]);
            let Some(fp) = fp else {
                eprintln!("SKIP: {name}: parse failed");
                continue;
            };

            let n = fp.n;
            let mut answers: [Option<Answer>; MAX_N] = [None; MAX_N];
            let answer_arr = test["answers"].as_array().unwrap();
            for i in 0..n {
                if let Some(s) = answer_arr[i].as_str() {
                    answers[i] = Some(Answer::from(s.as_bytes()[0] - b'A'));
                }
            }

            let got = check_answer(
                &fp,
                State {
                    answers,
                    eliminated: fp.initial_state.eliminated,
                },
                qi,
            )
            .is_valid();

            if got == expect {
                passed += 1;
            } else {
                failed += 1;
                eprintln!("FAIL: {name}");
                eprintln!("  expected: {expect}");
                eprintln!("  got:      {got}");
            }
        }

        eprintln!("{passed}/{} passed", passed + failed);
        assert_eq!(failed, 0, "{failed} test(s) failed");
    }

    fn minimal_fp(n: usize, oc: usize) -> FlatPuzzle {
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

    /// Property test: on random fully-answered boards, `check_claim_fast` must
    /// agree with `check_claim(..).is_valid()` for every candidate claim value,
    /// across every kind it implements. Excluded: `AnswerIsSelf`/`TrueStmt`, which
    /// never appear as statements (see `check_form::check_stmt_kind`), and the
    /// scoped-sameness types, whose arm is `unreachable!` because their verdict
    /// needs the candidate list. Locks the equivalence the doc comment on
    /// `check_claim_fast` promises.
    #[test]
    fn check_claim_fast_matches_check_claim() {
        use crate::rng::Rng;
        use std::panic::{AssertUnwindSafe, catch_unwind};

        type Outcome = Result<bool, Box<dyn std::any::Any + Send>>;
        const OUTCOMES: [&str; 3] = ["valid", "invalid", "panic"];
        fn index(r: &Outcome) -> usize {
            match r {
                Ok(true) => 0,
                Ok(false) => 1,
                Err(_) => 2,
            }
        }

        // Some values assert, so silence the default hook for the sweep and report
        // from the collected list instead.
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let mut mismatches: Vec<String> = Vec::new();
        let mut agreed = [0u64; 3]; // indexed by outcome: valid, invalid, panic

        // Every value a claim could plausibly carry: all in-range
        // positions/counts, all five letters (including ones beyond `oc`, to
        // probe the phantom-letter bound checks), and NONE. Capped below
        // `MAX_N` rather than at `n`: values in `[n, MAX_N)` are still
        // well-formed positions in a *larger* puzzle and exercise the
        // `>= n` guards; `MAX_N` itself indexes past the fixed-size answers
        // array, which is the pre-existing §2 boundary issue, not this test's
        // target.
        let values: Vec<OptionValue> = (0..MAX_N as u8)
            .map(OptionValue::num)
            .chain(std::iter::once(OptionValue::NONE))
            .collect();

        let mut checked = 0u64;
        for seed in 0..200u32 {
            let mut rng = Rng::new(seed);
            let n = rng.int(2, MAX_N as i32) as usize;
            let oc = if rng.int(0, 1) == 0 { 3 } else { 5 };
            let mut sol = [Answer::A; MAX_N];
            for a in sol.iter_mut().take(n) {
                *a = rng.pick_letter(oc);
            }

            let fp = minimal_fp(n, oc);
            let mut state = fp.initial_state;
            for i in 0..n {
                state.answers[i] = Some(sol[i]);
            }

            for qi in 0..n {
                let other_qi = loop {
                    let c = rng.int(0, n as i32 - 1) as usize;
                    if c != qi {
                        break c as u8;
                    }
                };
                let answer = rng.pick_letter(oc);
                let before_index = rng.int(0, n as i32) as u8;
                let after_index = rng.int(0, (n as i32 - 1).max(0)) as u8;

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
                        question_index: other_qi,
                    },
                    QuestionType::LeastCommon,
                    QuestionType::MostCommon,
                    QuestionType::NoOtherHasAnswer,
                    QuestionType::EqualCount { answer },
                    QuestionType::LetterDist {
                        question_index: other_qi,
                    },
                ];

                let opt = OptionPos {
                    qi,
                    oi: sol[qi].idx(),
                };
                for question_type in kinds {
                    for &value in &values {
                        let claim = Claim {
                            question_type,
                            value,
                        };
                        // Outcome, not just verdict: a structurally impossible value
                        // asserts (see the module doc), and the two wrappers must
                        // agree on *that* too — they share `check_claim_impl`, so a
                        // panic on one path and a verdict on the other would mean the
                        // wrappers had diverged.
                        let fast = catch_unwind(AssertUnwindSafe(|| {
                            check_claim_fast(oc, &sol[..n], qi, &claim)
                        }));
                        let slow = catch_unwind(AssertUnwindSafe(|| {
                            check_claim(&fp, state, opt, claim).is_valid()
                        }));
                        let (fi, si) = (index(&fast), index(&slow));
                        if fi == si {
                            agreed[fi] += 1;
                        } else {
                            mismatches.push(format!(
                                "seed {seed} n={n} oc={oc} qi={qi} sol={:?} claim={claim:?}: \
                                 fast={} slow={}",
                                &sol[..n],
                                OUTCOMES[fi],
                                OUTCOMES[si],
                            ));
                        }
                        checked += 1;
                    }
                }
            }
        }
        std::panic::set_hook(hook);
        for m in mismatches.iter().take(10) {
            eprintln!("MISMATCH: {m}");
        }
        assert!(
            mismatches.is_empty(),
            "{} fast/slow mismatch(es)",
            mismatches.len()
        );
        // Agreement is worthless if the sweep stopped producing one of the outcomes —
        // a narrowed value or kind list would still "pass". The floor is deliberately
        // loose: it catches a count going to zero, not a drift in the mix. A zero on
        // `panic` means nothing in `check_claim_impl` asserts on a malformed value any
        // more, which is worth knowing either way.
        for (i, &count) in agreed.iter().enumerate() {
            assert!(
                count >= 10,
                "only {count} agreed `{}` outcome(s) in {checked} comparisons — \
                 the sweep no longer covers it",
                OUTCOMES[i]
            );
        }
        eprintln!(
            "check_claim_fast_matches_check_claim: {checked} comparisons, agreed \
             {} valid / {} invalid / {} panic",
            agreed[0], agreed[1], agreed[2]
        );
    }
}
