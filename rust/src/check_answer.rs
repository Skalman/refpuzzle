//! Play-time validity: is an answer to question `qi` valid / pending / invalid on
//! the current (possibly partial) board? `check_claim` is the single authority the
//! solver, generator, and UI all share.

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

// ── Helpers ──

pub(crate) struct CountResult {
    pub(crate) count: u8,
    pub(crate) remaining: u8,
}

#[derive(Clone, Copy)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum Pred {
    IsAnswer(Answer),
    IsVowel,
    IsConsonant,
}

impl Pred {
    pub(crate) fn matches(self, a: Answer) -> bool {
        match self {
            Pred::IsAnswer(t) => a == t,
            Pred::IsVowel => a.is_vowel(),
            Pred::IsConsonant => !a.is_vowel(),
        }
    }
    fn mask(self) -> u8 {
        match self {
            Pred::IsAnswer(t) => 1u8 << t.idx(),
            Pred::IsVowel => 0b10001,
            Pred::IsConsonant => 0b01110,
        }
    }
}

pub(crate) fn count_matching(
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    pred: Pred,
    from: usize,
    to: usize,
) -> CountResult {
    let mask = pred.mask();
    let mut count: u8 = 0;
    let mut remaining: u8 = 0;
    for i in from..to {
        match answers[i] {
            Some(a) if pred.matches(a) => count += 1,
            None if eliminated[i] & mask != mask => remaining += 1,
            _ => {}
        }
    }
    CountResult { count, remaining }
}

fn count_validity(cr: CountResult, ov: OptionValue) -> Validity {
    // NONE/UNUSED on a count: malformed but check_answer routes them here for
    // semantic evaluation. Treat as Invalid (the count can never be null).
    if !ov.is_num() {
        return Validity::Invalid;
    }
    let ov = ov.value();
    if cr.count > ov || cr.count + cr.remaining < ov {
        Validity::Invalid
    } else if cr.count == ov && cr.remaining == 0 {
        Validity::Valid
    } else {
        Validity::Pending
    }
}

pub(crate) fn count_range(qt: &QuestionType, n: usize) -> (usize, usize) {
    match *qt {
        QuestionType::CountAnswerBefore { before_index, .. } => (0, before_index as usize),
        QuestionType::CountAnswerAfter { after_index, .. } => (after_index as usize + 1, n),
        _ => (0, n),
    }
}

/// The counting predicate for a count-type question, or `None` for any other
/// kind. Mirrors the TS `countPred`; lets `explain` reproduce count-based
/// contradiction reasons off the same primitive `check_answer` counts with.
#[allow(dead_code)] // wired into explain (and thence wasm) in a later increment
pub(crate) fn count_pred(qt: &QuestionType) -> Option<Pred> {
    match *qt {
        QuestionType::CountAnswer { answer }
        | QuestionType::CountAnswerBefore { answer, .. }
        | QuestionType::CountAnswerAfter { answer, .. } => Some(Pred::IsAnswer(answer)),
        QuestionType::CountVowel => Some(Pred::IsVowel),
        QuestionType::CountConsonant => Some(Pred::IsConsonant),
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
) -> Validity {
    let amask = 1u8 << answer.idx();
    if pos.is_num() {
        let p = pos.value() as usize;
        if p < start || p >= end {
            return Validity::Invalid;
        }
        if let Some(pa) = answers[p] {
            if pa != answer {
                return Validity::Invalid;
            }
        } else if eliminated[p] & amask != 0 {
            return Validity::Invalid;
        }
        let mut all_certain = true;
        for j in start..p {
            if answers[j] == Some(answer) {
                return Validity::Invalid;
            }
            if answers[j].is_none() && eliminated[j] & amask == 0 {
                all_certain = false;
            }
        }
        if answers[p] == Some(answer) && all_certain {
            Validity::Valid
        } else {
            Validity::Pending
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
) -> Validity {
    let amask = 1u8 << answer.idx();
    if pos.is_num() {
        let p = pos.value() as usize;
        if p < start || p >= end {
            return Validity::Invalid;
        }
        if let Some(pa) = answers[p] {
            if pa != answer {
                return Validity::Invalid;
            }
        } else if eliminated[p] & amask != 0 {
            return Validity::Invalid;
        }
        let mut all_certain = true;
        for j in (p + 1)..end {
            if answers[j] == Some(answer) {
                return Validity::Invalid;
            }
            if answers[j].is_none() && eliminated[j] & amask == 0 {
                all_certain = false;
            }
        }
        if answers[p] == Some(answer) && all_certain {
            Validity::Valid
        } else {
            Validity::Pending
        }
    } else {
        none_in_range(answers, eliminated, answer, start, end)
    }
}

/// The NONE ("no question in `start..end` has `answer`") arm shared by
/// `first_in_range`/`last_in_range`: Invalid if one already does, Pending if one
/// still could, else Valid.
fn none_in_range(
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    answer: Answer,
    start: usize,
    end: usize,
) -> Validity {
    let amask = 1u8 << answer.idx();
    let mut could_exist = false;
    for j in start..end {
        if answers[j] == Some(answer) {
            return Validity::Invalid;
        }
        if answers[j].is_none() && eliminated[j] & amask == 0 {
            could_exist = true;
        }
    }
    if could_exist {
        Validity::Pending
    } else {
        Validity::Valid
    }
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
fn check_claim_core(n: usize, oc: usize, state: State, opt: OptionPos, claim: Claim) -> Validity {
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
            let cr = count_matching(answers, eliminated, Pred::IsAnswer(answer), from, to);
            count_validity(cr, ov)
        }

        QuestionType::CountVowel => {
            let cr = count_matching(answers, eliminated, Pred::IsVowel, 0, n);
            count_validity(cr, ov)
        }

        QuestionType::CountConsonant => {
            let cr = count_matching(answers, eliminated, Pred::IsConsonant, 0, n);
            count_validity(cr, ov)
        }

        QuestionType::MostCommonCount => {
            if !ov.is_num() {
                return Validity::Invalid;
            }
            let ov = ov.value();
            let c = fill_counts(answers, n);
            for i in 0..5 {
                if c[i] > ov {
                    return Validity::Invalid;
                }
            }
            if !all_answered(answers, n) {
                return Validity::Pending;
            }
            let max = c.iter().copied().max().unwrap_or(0);
            if max == ov {
                Validity::Valid
            } else {
                Validity::Invalid
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
                return Validity::Invalid;
            }
            match answers[question_index as usize] {
                Some(target) => {
                    if target as u8 == ov.value() {
                        Validity::Valid
                    } else {
                        Validity::Invalid
                    }
                }
                // The target can never take the claimed letter if it's struck out there.
                None if eliminated[question_index as usize] & (1u8 << ov.value()) != 0 => {
                    Validity::Invalid
                }
                None => Validity::Pending,
            }
        }

        QuestionType::LetterDist { question_index } => match answers[question_index as usize] {
            Some(other) => {
                if !ov.is_num() {
                    return Validity::Invalid;
                }
                let dist = (self_oi as u8).abs_diff(other as u8);
                if dist == ov.value() {
                    Validity::Valid
                } else {
                    Validity::Invalid
                }
            }
            None => Validity::Pending,
        },

        QuestionType::SameAs => {
            if ov.is_none() {
                // "none": valid iff no other question shares qi's (candidate) answer.
                let amask = 1u8 << self_letter.idx();
                let mut could_exist = false;
                for j in 0..n {
                    if j == qi {
                        continue;
                    }
                    if answers[j] == Some(self_letter) {
                        return Validity::Invalid;
                    }
                    if answers[j].is_none() && eliminated[j] & amask == 0 {
                        could_exist = true;
                    }
                }
                return if could_exist {
                    Validity::Pending
                } else {
                    Validity::Valid
                };
            }
            if !ov.is_num() {
                return Validity::Invalid;
            }
            let ov = ov.value() as usize;
            if ov >= n || ov == qi {
                return Validity::Invalid;
            }
            match answers[ov] {
                Some(ta) => {
                    if ta == self_letter {
                        Validity::Valid
                    } else {
                        Validity::Invalid
                    }
                }
                // Impossible if qi's letter is already struck out at the target.
                None if eliminated[ov] & (1u8 << self_letter.idx()) != 0 => Validity::Invalid,
                None => Validity::Pending,
            }
        }

        QuestionType::SameAsWhich { question_index } => {
            if !ov.is_num() {
                return Validity::Invalid;
            }
            let ov = ov.value() as usize;
            if ov >= n || ov == qi || ov == question_index as usize {
                return Validity::Invalid;
            }
            let ref_ans = match answers[question_index as usize] {
                Some(a) => a,
                None => return Validity::Pending,
            };
            match answers[ov] {
                Some(ta) => {
                    if ta == ref_ans {
                        Validity::Valid
                    } else {
                        Validity::Invalid
                    }
                }
                // Impossible if the ref's letter is already struck out at the target.
                None if eliminated[ov] & (1u8 << ref_ans.idx()) != 0 => Validity::Invalid,
                None => Validity::Pending,
            }
        }

        // ── NoOtherHasAnswer: "not the answer to any OTHER question" ──
        QuestionType::NoOtherHasAnswer => {
            if !ov.is_num() || ov.value() > 4 {
                return Validity::Invalid;
            }
            let letter = Answer::from(ov.value());
            let amask = 1u8 << ov.value();
            let mut others: u8 = 0;
            let mut could_match: u8 = 0;
            for j in 0..n {
                if j == qi {
                    continue;
                }
                match answers[j] {
                    Some(x) if x == letter => others += 1,
                    None if eliminated[j] & amask == 0 => could_match += 1,
                    _ => {}
                }
            }
            if others > 0 {
                Validity::Invalid
            } else if could_match == 0 {
                Validity::Valid
            } else {
                Validity::Pending
            }
        }

        // ── Previous/Next same ──
        // No pre-range check: first_in_range/last_in_range already reject a numeric
        // position outside `start..end` (here `0..qi` / `qi+1..n`).
        QuestionType::PrevSame => last_in_range(answers, eliminated, self_letter, 0, qi, ov),

        QuestionType::NextSame => first_in_range(answers, eliminated, self_letter, qi + 1, n, ov),

        // ── Only same ──
        QuestionType::OnlySame => {
            let amask = 1u8 << self_oi;

            if ov.is_none() {
                let mut matches: u8 = 0;
                let mut could_match: u8 = 0;
                for j in 0..n {
                    if j == qi {
                        continue;
                    }
                    match answers[j] {
                        Some(x) if x == self_letter => matches += 1,
                        None if eliminated[j] & amask == 0 => could_match += 1,
                        _ => {}
                    }
                }
                if matches > 0 {
                    Validity::Invalid
                } else if could_match == 0 {
                    Validity::Valid
                } else {
                    Validity::Pending
                }
            } else if !ov.is_num() || ov.value() as usize >= n {
                Validity::Invalid
            } else {
                let target = ov.value() as usize;
                if target == qi {
                    return Validity::Invalid;
                }

                if let Some(ta) = answers[target]
                    && ta != self_letter
                {
                    return Validity::Invalid;
                }
                // Target can't take qi's letter if it's struck out there.
                if answers[target].is_none() && eliminated[target] & amask != 0 {
                    return Validity::Invalid;
                }

                let mut other_matches: u8 = 0;
                let mut other_remaining: u8 = 0;
                for j in 0..n {
                    if j == qi || j == target {
                        continue;
                    }
                    match answers[j] {
                        Some(x) if x == self_letter => other_matches += 1,
                        None if eliminated[j] & amask == 0 => other_remaining += 1,
                        _ => {}
                    }
                }

                if other_matches > 0 {
                    return Validity::Invalid;
                }
                if answers[target] == Some(self_letter) && other_remaining == 0 {
                    Validity::Valid
                } else {
                    Validity::Pending
                }
            }
        }

        // ── Consecutive identical ──
        QuestionType::ConsecIdent => {
            if ov.is_num() {
                let ov = ov.value() as usize;
                if ov + 1 >= n {
                    return Validity::Invalid;
                }

                if let (Some(pa), Some(pb)) = (answers[ov], answers[ov + 1])
                    && pa != pb
                {
                    return Validity::Invalid;
                }

                let poss_a = !eliminated[ov] & ALL_OPTIONS_MASK;
                let poss_b = !eliminated[ov + 1] & ALL_OPTIONS_MASK;
                if poss_a & poss_b == 0 {
                    return Validity::Invalid;
                }
                if let Some(pa) = answers[ov]
                    && is_eliminated(eliminated, ov + 1, pa.idx())
                {
                    return Validity::Invalid;
                }
                if let Some(pb) = answers[ov + 1]
                    && is_eliminated(eliminated, ov, pb.idx())
                {
                    return Validity::Invalid;
                }

                let mut other_confirmed_pairs: u8 = 0;
                let mut uncertain_pairs: u8 = 0;
                for j in 0..n.saturating_sub(1) {
                    if j == ov {
                        continue;
                    }
                    match (answers[j], answers[j + 1]) {
                        (Some(x), Some(y)) if x == y => other_confirmed_pairs += 1,
                        (Some(_), Some(_)) => {}
                        _ => uncertain_pairs += 1,
                    }
                }

                if other_confirmed_pairs > 0 {
                    return Validity::Invalid;
                }

                if let (Some(pa), Some(pb)) = (answers[ov], answers[ov + 1])
                    && pa == pb
                    && uncertain_pairs == 0
                {
                    return Validity::Valid;
                }

                Validity::Pending
            } else if ov.is_none() {
                let mut any_confirmed_pair = false;
                let mut any_uncertain = false;
                for j in 0..n.saturating_sub(1) {
                    match (answers[j], answers[j + 1]) {
                        (Some(x), Some(y)) if x == y => any_confirmed_pair = true,
                        (Some(_), Some(_)) => {}
                        _ => any_uncertain = true,
                    }
                }
                if any_confirmed_pair {
                    Validity::Invalid
                } else if any_uncertain {
                    Validity::Pending
                } else {
                    Validity::Valid
                }
            } else {
                Validity::Invalid
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
                    return Validity::Invalid;
                }

                if let Some(pa) = answers[ov]
                    && pa != answer
                {
                    return Validity::Invalid;
                }
                // Target can't take `answer` if it's struck out there.
                if answers[ov].is_none() && eliminated[ov] & amask != 0 {
                    return Validity::Invalid;
                }

                let mut other_matches: u8 = 0;
                let mut other_remaining: u8 = 0;
                for j in 0..n {
                    if j == ov || (j + 1) % 2 != parity {
                        continue;
                    }
                    match answers[j] {
                        Some(x) if x == answer => other_matches += 1,
                        None if eliminated[j] & amask == 0 => other_remaining += 1,
                        _ => {}
                    }
                }

                if other_matches > 0 {
                    return Validity::Invalid;
                }
                if answers[ov] == Some(answer) && other_remaining == 0 {
                    Validity::Valid
                } else {
                    Validity::Pending
                }
            } else if ov.is_none() {
                let mut any_match = false;
                let mut any_could = false;
                for j in 0..n {
                    if (j + 1) % 2 != parity {
                        continue;
                    }
                    if answers[j] == Some(answer) {
                        any_match = true;
                    }
                    if answers[j].is_none() && eliminated[j] & amask == 0 {
                        any_could = true;
                    }
                }
                if any_match {
                    Validity::Invalid
                } else if any_could {
                    Validity::Pending
                } else {
                    Validity::Valid
                }
            } else {
                Validity::Invalid
            }
        }

        // ── Equal count ──
        QuestionType::EqualCount { answer } => {
            if ov.is_num() {
                if ov.value() as usize >= oc {
                    return Validity::Invalid;
                }
                let claimed = Answer::from(ov.value());
                if claimed == answer {
                    return Validity::Invalid;
                }
                let CountResult {
                    count: rc,
                    remaining: rr,
                } = count_matching(answers, eliminated, Pred::IsAnswer(answer), 0, n);
                let CountResult {
                    count: sc,
                    remaining: sr,
                } = count_matching(answers, eliminated, Pred::IsAnswer(claimed), 0, n);
                if rc + rr < sc || sc + sr < rc {
                    return Validity::Invalid;
                }
                if rr == 0 && sr == 0 {
                    return if rc == sc {
                        Validity::Valid
                    } else {
                        Validity::Invalid
                    };
                }
                Validity::Pending
            } else if ov.is_none() {
                if !all_answered(answers, n) {
                    return Validity::Pending;
                }
                let ref_count = count_answer_simple(answers, answer, 0, n);
                let any_match = LETTERS[..oc]
                    .iter()
                    .any(|&l| l != answer && count_answer_simple(answers, l, 0, n) == ref_count);
                if any_match {
                    Validity::Invalid
                } else {
                    Validity::Valid
                }
            } else {
                Validity::Invalid
            }
        }

        // ── Global: need all answers ──
        QuestionType::LeastCommon | QuestionType::MostCommon => {
            if !ov.is_num() || ov.value() as usize >= oc {
                return Validity::Invalid;
            }
            if !all_answered(answers, n) {
                return Validity::Pending;
            }
            let ov = ov.value() as usize;
            let c = fill_counts(answers, n);
            match *qt {
                QuestionType::LeastCommon => {
                    let min = c[..oc].iter().copied().min().unwrap_or(0);
                    if c[ov] == min && c[..oc].iter().filter(|&&x| x == min).count() == 1 {
                        Validity::Valid
                    } else {
                        Validity::Invalid
                    }
                }
                QuestionType::MostCommon => {
                    let max = c[..oc].iter().copied().max().unwrap_or(0);
                    if c[ov] == max && c[..oc].iter().filter(|&&x| x == max).count() == 1 {
                        Validity::Valid
                    } else {
                        Validity::Invalid
                    }
                }
                _ => unreachable!(),
            }
        }

        // ── Always valid ──
        QuestionType::AnswerIsSelf => Validity::Valid,

        // TrueStmt can't be checked via check_claim
        QuestionType::TrueStmt => Validity::Pending,
    }
}

/// Evaluate the **semantic truth** of a claim against the current puzzle state.
/// Returns `Valid`/`Invalid`/`Pending` analogous to `check_answer`. See
/// [`check_claim_core`] for the implementation and its caveats.
pub fn check_claim(fp: &FlatPuzzle, state: State, opt: OptionPos, claim: Claim) -> Validity {
    check_claim_core(fp.n, fp.option_count, state, opt, claim)
}

fn affected_by_own_answer(qt: &QuestionType, qi: usize) -> bool {
    match *qt {
        QuestionType::AnswerOf { question_index } => question_index as usize == qi,
        QuestionType::SameAsWhich { question_index } => question_index as usize == qi,
        _ => true,
    }
}

fn maybe_consistent(result: Validity, qt: &QuestionType, qi: usize) -> Validity {
    if result == Validity::Valid && affected_by_own_answer(qt, qi) {
        Validity::Consistent
    } else {
        result
    }
}

pub fn check_answer(fp: &FlatPuzzle, state: State, qi: usize) -> Validity {
    let a = match state.answers[qi] {
        Some(a) => a,
        None => {
            let oc = fp.option_count;
            if (!state.eliminated[qi] & ((1 << oc) - 1)) == 0 {
                return Validity::Invalid;
            }
            return Validity::Neutral;
        }
    };
    let ai = a.idx();
    let qt = &fp.question_types[qi];

    if matches!(qt, QuestionType::TrueStmt) {
        let selected_claim = match fp.claim_at(qi, ai) {
            Some(c) => c,
            None => return Validity::Invalid,
        };
        let selected_v = check_claim(fp, state, OptionPos { qi, oi: ai }, selected_claim);
        if selected_v != Validity::Valid {
            return selected_v;
        }
        for oi in 0..fp.option_count {
            if oi == ai {
                continue;
            }
            let mut hyp = state;
            hyp.answers[qi] = Some(Answer::from(oi as u8));
            if check_claim(fp, hyp, OptionPos { qi, oi }, selected_claim) != Validity::Valid {
                return Validity::Consistent;
            }
        }
        return Validity::Valid;
    }

    let ov = fp.options[qi][ai];
    // Value routing into check_claim, and how an UNUSED selected slot is handled —
    // the asymmetry is by design:
    //  - letter-valued types (AnswerOf, extrema) pass the stored value through, so an
    //    UNUSED slot reaches check_claim, which rejects it (→ Invalid);
    //  - identity-option types take the value from the option index (never UNUSED);
    //  - all other (numeric) types short-circuit an UNUSED slot to Pending, treating
    //    an unfilled option as undecided rather than wrong.
    let ov = match *qt {
        QuestionType::AnswerOf { .. } | QuestionType::LeastCommon | QuestionType::MostCommon => ov,
        _ if qt.has_identity_options() => OptionValue::num(ai as u8),
        _ => {
            if ov.is_unused() {
                return Validity::Pending;
            }
            ov
        }
    };
    maybe_consistent(
        check_claim(
            fp,
            state,
            OptionPos { qi, oi: ai },
            Claim {
                question_type: *qt,
                value: ov,
            },
        ),
        qt,
        qi,
    )
}

pub fn check_answers(fp: &FlatPuzzle, answers: &[Option<Answer>; MAX_N]) -> bool {
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
/// `check_claim_core`. Eliminated bits only ever gate `None` answer slots there,
/// so leaving them at `State::initial`'s phantom-only mask (rather than
/// reconstructing "everything but the known answer") is sound — every slot up
/// to `n` is `Some`. Equivalence to `check_claim` is pinned by
/// `tests::check_claim_fast_matches_check_claim`.
// Inlined on native for the generator's inner loop; outlined on wasm
// where every duplicated body shows up in the download.
#[cfg_attr(not(target_arch = "wasm32"), inline(always))]
pub fn check_claim_fast(option_count: usize, answers: &[Answer], qi: usize, claim: &Claim) -> bool {
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
    check_claim_core(n, option_count, state, opt, *claim).is_valid()
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
            let fp = match fp {
                Some(fp) => fp,
                None => {
                    eprintln!("SKIP: {name}: parse failed");
                    continue;
                }
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
            let fp = match fp {
                Some(fp) => fp,
                None => {
                    eprintln!("SKIP: {name}: parse failed");
                    continue;
                }
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
    /// across every kind it implements (all but `AnswerIsSelf`/`TrueStmt`, which
    /// are never claim subjects — see `is_claim_type` in fill.rs). Locks the
    /// equivalence the doc comment on `check_claim_fast` promises.
    #[test]
    fn check_claim_fast_matches_check_claim() {
        use crate::rng::Rng;

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
                    QuestionType::SameAs,
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
                    QuestionType::SameAsWhich {
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
                        let fast = check_claim_fast(oc, &sol[..n], qi, &claim);
                        let slow = check_claim(&fp, state, opt, claim).is_valid();
                        assert_eq!(
                            fast,
                            slow,
                            "seed {seed} n={n} oc={oc} qi={qi} sol={:?} claim={:?}",
                            &sol[..n],
                            claim
                        );
                        checked += 1;
                    }
                }
            }
        }
        eprintln!("check_claim_fast_matches_check_claim: {checked} comparisons");
    }
}
