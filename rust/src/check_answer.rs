use crate::types::*;

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

struct CountResult {
    count: u8,
    remaining: u8,
}

#[derive(Clone, Copy)]
#[allow(clippy::enum_variant_names)]
enum Pred {
    IsAnswer(Answer),
    IsVowel,
    IsConsonant,
}

impl Pred {
    fn matches(self, a: Answer) -> bool {
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

fn count_matching(
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

fn count_validity(cr: CountResult, value: OptionValue) -> Validity {
    // NONE/UNUSED on a count: malformed but check_answer routes them here for
    // semantic evaluation. Treat as Invalid (the count can never be null).
    if !value.is_num() {
        return Validity::Invalid;
    }
    let value = value.value();
    if cr.count > value || cr.count + cr.remaining < value {
        Validity::Invalid
    } else if cr.count == value && cr.remaining == 0 {
        Validity::Valid
    } else {
        Validity::Pending
    }
}

fn count_range(t: &QuestionType, n: usize) -> (usize, usize) {
    match *t {
        QuestionType::CountAnswerBefore { before_index, .. } => (0, before_index as usize),
        QuestionType::CountAnswerAfter { after_index, .. } => (after_index as usize + 1, n),
        _ => (0, n),
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

fn count_answer_with_remaining(
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    target: Answer,
    from: usize,
    to: usize,
) -> (u8, u8) {
    let mask = 1u8 << target.idx();
    let mut count: u8 = 0;
    let mut remaining: u8 = 0;
    for i in from..to {
        if answers[i] == Some(target) {
            count += 1;
        } else if answers[i].is_none() && eliminated[i] & mask == 0 {
            remaining += 1;
        }
    }
    (count, remaining)
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

/// Evaluate the **semantic truth** of a claim against the current puzzle state.
/// Returns `Valid`/`Invalid`/`Pending` analogous to `check_answer`.
///
/// This is NOT a wellformedness check. Although it incidentally short-circuits
/// on some structural issues (parity mismatch, value out of [0..=4] for letter
/// types), it assumes the claim is already well-formed and behaves unpredictably
/// otherwise (e.g. AnswerOf with an out-of-range `question_index` panics on
/// `answers[...]`). For form checks, see `check_form::check_claim_form`.
pub fn check_claim(fp: &FlatPuzzle, state: State, opt: OptionPos, claim: Claim) -> Validity {
    let qt = &claim.question_type;
    let value = claim.value;
    let qi = opt.qi;
    let si = opt.oi;
    let self_letter = Answer::from(si as u8);
    let answers = &state.answers;
    let eliminated = &state.eliminated;
    let n = fp.n;
    let oc = fp.option_count;

    match *qt {
        // ── Counting ──
        QuestionType::CountAnswer { answer }
        | QuestionType::CountAnswerBefore { answer, .. }
        | QuestionType::CountAnswerAfter { answer, .. } => {
            let (from, to) = count_range(qt, n);
            let cr = count_matching(answers, eliminated, Pred::IsAnswer(answer), from, to);
            count_validity(cr, value)
        }

        QuestionType::CountVowel => {
            let cr = count_matching(answers, eliminated, Pred::IsVowel, 0, n);
            count_validity(cr, value)
        }

        QuestionType::CountConsonant => {
            let cr = count_matching(answers, eliminated, Pred::IsConsonant, 0, n);
            count_validity(cr, value)
        }

        QuestionType::MostCommonCount => {
            if !value.is_num() {
                return Validity::Invalid;
            }
            let v = value.value();
            let c = fill_counts(answers, n);
            for i in 0..5 {
                if c[i] > v {
                    return Validity::Invalid;
                }
            }
            if !all_answered(answers, n) {
                return Validity::Pending;
            }
            let max = c.iter().copied().max().unwrap_or(0);
            if max == v {
                Validity::Valid
            } else {
                Validity::Invalid
            }
        }

        // ── Positional: first/closest-after ──
        QuestionType::FirstWith { answer } => {
            first_in_range(answers, eliminated, answer, 0, n, value)
        }
        QuestionType::ClosestAfter {
            after_index,
            answer,
        } => first_in_range(
            answers,
            eliminated,
            answer,
            after_index as usize + 1,
            n,
            value,
        ),

        // ── Positional: last/closest-before ──
        QuestionType::LastWith { answer } => {
            last_in_range(answers, eliminated, answer, 0, n, value)
        }
        QuestionType::ClosestBefore {
            before_index,
            answer,
        } => last_in_range(answers, eliminated, answer, 0, before_index as usize, value),

        // ── Reference ──
        QuestionType::AnswerOf { question_index } => {
            if !value.is_num() || value.value() > 4 {
                return Validity::Invalid;
            }
            match answers[question_index as usize] {
                Some(target) => {
                    if target as u8 == value.value() {
                        Validity::Valid
                    } else {
                        Validity::Invalid
                    }
                }
                None => Validity::Pending,
            }
        }

        QuestionType::LetterDist { question_index } => match answers[question_index as usize] {
            Some(other) => {
                if !value.is_num() {
                    return Validity::Invalid;
                }
                let dist = (si as u8).abs_diff(other as u8);
                if dist == value.value() {
                    Validity::Valid
                } else {
                    Validity::Invalid
                }
            }
            None => Validity::Pending,
        },

        QuestionType::SameAs => {
            if value.is_none() {
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
            if !value.is_num() {
                return Validity::Invalid;
            }
            let v = value.value() as usize;
            if v >= n || v == qi {
                return Validity::Invalid;
            }
            match answers[v] {
                Some(ta) => {
                    if ta == self_letter {
                        Validity::Valid
                    } else {
                        Validity::Invalid
                    }
                }
                None => Validity::Pending,
            }
        }

        QuestionType::SameAsWhich { question_index } => {
            if !value.is_num() {
                return Validity::Invalid;
            }
            let v = value.value() as usize;
            if v >= n || v == qi || v == question_index as usize {
                return Validity::Invalid;
            }
            let ref_ans = match answers[question_index as usize] {
                Some(a) => a,
                None => return Validity::Pending,
            };
            match answers[v] {
                Some(ta) => {
                    if ta == ref_ans {
                        Validity::Valid
                    } else {
                        Validity::Invalid
                    }
                }
                None => Validity::Pending,
            }
        }

        // ── NoOtherHasAnswer: "not the answer to any OTHER question" ──
        QuestionType::NoOtherHasAnswer => {
            if !value.is_num() || value.value() > 4 {
                return Validity::Invalid;
            }
            let letter = Answer::from(value.value());
            let amask = 1u8 << value.value();
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
        QuestionType::PrevSame => {
            if value.is_num() && value.value() as usize >= qi {
                return Validity::Invalid;
            }
            last_in_range(answers, eliminated, self_letter, 0, qi, value)
        }

        QuestionType::NextSame => {
            if value.is_num() {
                let v = value.value() as usize;
                if v <= qi || v >= n {
                    return Validity::Invalid;
                }
            }
            first_in_range(answers, eliminated, self_letter, qi + 1, n, value)
        }

        // ── Only same ──
        QuestionType::OnlySame => {
            let amask = 1u8 << si;

            if value.is_none() {
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
            } else if !value.is_num() || value.value() as usize >= n {
                Validity::Invalid
            } else {
                let target = value.value() as usize;
                if target == qi {
                    return Validity::Invalid;
                }

                if let Some(ta) = answers[target]
                    && ta != self_letter
                {
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
            if value.is_num() {
                let p = value.value() as usize;
                if p + 1 >= n {
                    return Validity::Invalid;
                }

                if let (Some(pa), Some(pb)) = (answers[p], answers[p + 1])
                    && pa != pb
                {
                    return Validity::Invalid;
                }

                let poss_a = !eliminated[p] & 0b11111u8;
                let poss_b = !eliminated[p + 1] & 0b11111u8;
                if poss_a & poss_b == 0 {
                    return Validity::Invalid;
                }
                if let Some(pa) = answers[p]
                    && eliminated[p + 1] & (1 << pa.idx()) != 0
                {
                    return Validity::Invalid;
                }
                if let Some(pb) = answers[p + 1]
                    && eliminated[p] & (1 << pb.idx()) != 0
                {
                    return Validity::Invalid;
                }

                let mut other_confirmed_pairs: u8 = 0;
                let mut uncertain_pairs: u8 = 0;
                for j in 0..n.saturating_sub(1) {
                    if j == p {
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

                if let (Some(pa), Some(pb)) = (answers[p], answers[p + 1])
                    && pa == pb
                    && uncertain_pairs == 0
                {
                    return Validity::Valid;
                }

                Validity::Pending
            } else if value.is_none() {
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

            if value.is_num() {
                let p = value.value() as usize;
                if (p + 1) % 2 != parity {
                    return Validity::Invalid;
                }

                if let Some(pa) = answers[p]
                    && pa != answer
                {
                    return Validity::Invalid;
                }

                let mut other_matches: u8 = 0;
                let mut other_remaining: u8 = 0;
                for j in 0..n {
                    if j == p || (j + 1) % 2 != parity {
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
                if answers[p] == Some(answer) && other_remaining == 0 {
                    Validity::Valid
                } else {
                    Validity::Pending
                }
            } else if value.is_none() {
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
            if value.is_num() {
                if value.value() > 4 {
                    return Validity::Invalid;
                }
                let claimed = Answer::from(value.value());
                if claimed == answer {
                    return Validity::Invalid;
                }
                let (rc, rr) = count_answer_with_remaining(answers, eliminated, answer, 0, n);
                let (sc, sr) = count_answer_with_remaining(answers, eliminated, claimed, 0, n);
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
            } else if value.is_none() {
                if !all_answered(answers, n) {
                    return Validity::Pending;
                }
                let ref_count = count_answer_simple(answers, answer, 0, n);
                let any_match = LETTERS
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
            if !value.is_num() || value.value() as usize >= oc {
                return Validity::Invalid;
            }
            if !all_answered(answers, n) {
                return Validity::Pending;
            }
            let v = value.value() as usize;
            let c = fill_counts(answers, n);
            match *qt {
                QuestionType::LeastCommon => {
                    let min = c[..oc].iter().copied().min().unwrap_or(0);
                    if c[v] == min && c[..oc].iter().filter(|&&x| x == min).count() == 1 {
                        Validity::Valid
                    } else {
                        Validity::Invalid
                    }
                }
                QuestionType::MostCommon => {
                    let max = c[..oc].iter().copied().max().unwrap_or(0);
                    if c[v] == max && c[..oc].iter().filter(|&&x| x == max).count() == 1 {
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

fn affected_by_own_answer(t: &QuestionType, qi: usize) -> bool {
    match *t {
        QuestionType::AnswerOf { question_index } => question_index as usize == qi,
        QuestionType::SameAsWhich { question_index } => question_index as usize == qi,
        _ => true,
    }
}

fn maybe_consistent(result: Validity, t: &QuestionType, qi: usize) -> Validity {
    if result == Validity::Valid && affected_by_own_answer(t, qi) {
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
    let t = &fp.question_types[qi];

    if matches!(t, QuestionType::TrueStmt) {
        let selected_claim = match fp.claim_at(qi, ai) {
            Some(c) => c,
            None => return Validity::Invalid,
        };
        let selected_v = check_claim(fp, state, OptionPos { qi, oi: ai }, selected_claim);
        if selected_v != Validity::Valid {
            return selected_v;
        }
        for oi in 0..5usize {
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

    let on = fp.options[qi][ai];
    let value = match *t {
        QuestionType::AnswerOf { .. } | QuestionType::LeastCommon | QuestionType::MostCommon => on,
        _ if t.has_identity_options() => OptionValue::num(ai as u8),
        _ => {
            if on.is_unused() {
                return Validity::Pending;
            }
            on
        }
    };
    maybe_consistent(
        check_claim(
            fp,
            state,
            OptionPos { qi, oi: ai },
            Claim {
                question_type: *t,
                value,
            },
        ),
        t,
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

/// True iff `value` represents this position: `Some(p)` matches `num(p)`,
/// `None` matches `NONE`. Used by position-finder arms in `check_claim_fast`.
fn pos_matches(value: OptionValue, pos: Option<usize>) -> bool {
    match pos {
        Some(p) => value.is_num() && value.value() as usize == p,
        None => value.is_none(),
    }
}

/// True iff `value` is `num(c)` with `c == count`. NONE / UNUSED never match.
fn count_matches(value: OptionValue, count: usize) -> bool {
    value.is_num() && value.value() as usize == count
}

/// Like `check_claim`, but assumes `answers` is fully populated; returns bool.
/// Same caveat applies: this is a **semantic** check (does the claim hold given
/// these answers?), not a wellformedness check.
// Inlined on native for the generator's inner loop; outlined on wasm
// where every duplicated body shows up in the download.
#[cfg_attr(not(target_arch = "wasm32"), inline(always))]
pub fn check_claim_fast(option_count: usize, answers: &[Answer], qi: usize, claim: &Claim) -> bool {
    let n = answers.len();
    let value = claim.value;
    match claim.question_type {
        QuestionType::CountAnswer { answer } => {
            count_matches(value, answers.iter().filter(|&&a| a == answer).count())
        }
        QuestionType::CountConsonant => {
            count_matches(value, answers.iter().filter(|&&a| !a.is_vowel()).count())
        }
        QuestionType::CountVowel => {
            count_matches(value, answers.iter().filter(|&&a| a.is_vowel()).count())
        }
        QuestionType::CountAnswerAfter {
            answer,
            after_index,
        } => count_matches(
            value,
            answers[(after_index as usize + 1)..]
                .iter()
                .filter(|&&a| a == answer)
                .count(),
        ),
        QuestionType::CountAnswerBefore {
            answer,
            before_index,
        } => count_matches(
            value,
            answers[..before_index as usize]
                .iter()
                .filter(|&&a| a == answer)
                .count(),
        ),
        QuestionType::AnswerOf { question_index } => {
            count_matches(value, answers[question_index as usize].idx())
        }
        QuestionType::FirstWith { answer } => {
            pos_matches(value, answers.iter().position(|&a| a == answer))
        }
        QuestionType::LastWith { answer } => {
            pos_matches(value, answers.iter().rposition(|&a| a == answer))
        }
        QuestionType::MostCommon => {
            if !value.is_num() || (value.value() as usize) >= option_count {
                return false;
            }
            let v = value.value() as usize;
            let mut counts = [0u16; 5];
            for &a in answers {
                counts[a.idx()] += 1;
            }
            let max = *counts[..option_count].iter().max().unwrap_or(&0);
            counts[v] == max && counts[..option_count].iter().filter(|&&c| c == max).count() == 1
        }
        QuestionType::ClosestAfter {
            after_index,
            answer,
        } => pos_matches(
            value,
            answers[(after_index as usize + 1)..]
                .iter()
                .position(|&a| a == answer)
                .map(|i| after_index as usize + 1 + i),
        ),
        QuestionType::ClosestBefore {
            before_index,
            answer,
        } => pos_matches(
            value,
            answers[..before_index as usize]
                .iter()
                .rposition(|&a| a == answer),
        ),
        QuestionType::MostCommonCount => {
            let mut counts = [0u16; 5];
            for &a in answers {
                counts[a.idx()] += 1;
            }
            let max = *counts[..option_count].iter().max().unwrap_or(&0);
            count_matches(value, max as usize)
        }
        QuestionType::LeastCommon => {
            if !value.is_num() || (value.value() as usize) >= option_count {
                return false;
            }
            let v = value.value() as usize;
            let mut counts = [0u16; 5];
            for &a in answers {
                counts[a.idx()] += 1;
            }
            let min = *counts[..option_count].iter().min().unwrap_or(&0);
            counts[v] == min && counts[..option_count].iter().filter(|&&c| c == min).count() == 1
        }
        QuestionType::NoOtherHasAnswer => {
            if !value.is_num() {
                return false;
            }
            let letter = Answer::from(value.value());
            (0..n).filter(|&j| j != qi).all(|j| answers[j] != letter)
        }
        QuestionType::EqualCount { answer } => {
            if !value.is_num() || (value.value() as usize) >= option_count {
                return false;
            }
            let v = value.value() as usize;
            if v == answer.idx() {
                return false;
            }
            let ref_count = answers.iter().filter(|&&a| a == answer).count();
            answers
                .iter()
                .filter(|&&a| a == Answer::from(v as u8))
                .count()
                == ref_count
        }
        QuestionType::ConsecIdent => pos_matches(
            value,
            (0..n.saturating_sub(1)).find(|&i| answers[i] == answers[i + 1]),
        ),
        QuestionType::OnlyOdd { answer } | QuestionType::OnlyEven { answer } => {
            let parity = matches!(claim.question_type, QuestionType::OnlyEven { .. }) as usize;
            let mut found: Option<usize> = None;
            let mut count = 0;
            for i in 0..n {
                if i % 2 == parity && answers[i] == answer {
                    found = Some(i);
                    count += 1;
                }
            }
            match count {
                0 => value.is_none(),
                1 => pos_matches(value, found),
                _ => false,
            }
        }
        QuestionType::PrevSame => {
            let self_ans = answers[qi];
            pos_matches(value, (0..qi).rev().find(|&i| answers[i] == self_ans))
        }
        QuestionType::NextSame => {
            let self_ans = answers[qi];
            pos_matches(value, ((qi + 1)..n).find(|&i| answers[i] == self_ans))
        }
        QuestionType::OnlySame => {
            let self_ans = answers[qi];
            let mut found: Option<usize> = None;
            let mut count = 0;
            for i in 0..n {
                if i != qi && answers[i] == self_ans {
                    found = Some(i);
                    count += 1;
                }
            }
            match count {
                0 => value.is_none(),
                1 => pos_matches(value, found),
                _ => false,
            }
        }
        QuestionType::SameAs => {
            let self_ans = answers[qi];
            let any_match = (0..n).any(|i| i != qi && answers[i] == self_ans);
            if !any_match {
                value.is_none()
            } else if value.is_num() {
                let v = value.value() as usize;
                v < n && v != qi && answers[v] == self_ans
            } else {
                false
            }
        }
        QuestionType::SameAsWhich { question_index } => {
            if !value.is_num() {
                return false;
            }
            let v = value.value() as usize;
            let ref_ans = answers[question_index as usize];
            v < n && v != qi && v != question_index as usize && answers[v] == ref_ans
        }
        QuestionType::LetterDist { question_index } => {
            let dist = (answers[qi] as u8).abs_diff(answers[question_index as usize] as u8);
            count_matches(value, dist as usize)
        }
        QuestionType::AnswerIsSelf | QuestionType::TrueStmt => false,
    }
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
                        eliminated[i] = 0b11111 ^ (1 << oi);
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
}
