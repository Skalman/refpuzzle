//! Well-posedness: does question `qi` have *exactly one* valid answer given the
//! answer key? (Existence + uniqueness — a discrete problem has no stability
//! notion, so Hadamard's third condition is vacuous here.) Split by what the check
//! depends on, which dictates where in the pipeline it can run:
//!
//! - [`check_well_posed_given_key`] — types whose answer is fixed by the key (+ the
//!   question's own params): MostCommon/LeastCommon/EqualCount (histogram),
//!   OnlySame/ConsecIdent/OnlyOdd/OnlyEven, NoOtherHasAnswer. Knowable at
//!   parametrize; `fill_options`/repair never change it.
//! - [`check_well_posed_given_options`] — types whose answer depends on the
//!   filled option/claim values: SameAs/SameAsWhich (distractor targets) and
//!   TrueStmt (claims). Only knowable once options exist.
//!
//! Both return `None` when well-posed, `Some(reason)` when ambiguous or answerless.
//! This is *uniqueness only*; feasibility ("can this kind be built for the key at
//! all?") is left to the reserve/downstream, not checked here.

use crate::check_answer::{Validity, check_claim};
use crate::types::*;

/// Letter histogram of the solution's first `n` answers.
fn counts(sol: &[Answer], n: usize) -> [u8; 5] {
    let mut c = [0u8; 5];
    for &a in sol.iter().take(n) {
        c[a.idx()] += 1;
    }
    c
}

/// Well-posedness that the answer key alone (plus the question's params) settles.
/// `None` if `qi` has a unique answer; `Some(reason)` if the key leaves it
/// ambiguous. Called at parametrize (authoritative — nothing downstream changes it)
/// and in `refpuzzle check` (untrusted input).
pub fn check_well_posed_given_key(
    n: usize,
    oc: usize,
    sol: &[Answer],
    qi: usize,
    qt: QuestionType,
) -> Option<String> {
    let c = counts(sol, n);
    match qt {
        QuestionType::MostCommon => {
            let max = c[..oc].iter().copied().max().unwrap_or(0);
            (c[..oc].iter().filter(|&&x| x == max).count() != 1)
                .then(|| "MostCommon: no unique most-common letter (tie)".to_string())
        }
        QuestionType::LeastCommon => {
            let min = c[..oc].iter().copied().min().unwrap_or(0);
            (c[..oc].iter().filter(|&&x| x == min).count() != 1)
                .then(|| "LeastCommon: no unique least-common letter (tie)".to_string())
        }
        QuestionType::EqualCount { answer } => {
            // At most one *other* letter may share `answer`'s count: one tying letter
            // (that's the answer) or none (the "none" answer). Two+ ties are ambiguous.
            let ref_count = c[answer.idx()];
            let ties = (0..oc)
                .filter(|&l| l != answer.idx() && c[l] == ref_count)
                .count();
            (ties > 1).then(|| format!("EqualCount: {ties} letters share the count — ambiguous"))
        }
        QuestionType::OnlySame => {
            let matches = (0..n).filter(|&i| i != qi && sol[i] == sol[qi]).count();
            (matches > 1)
                .then(|| "OnlySame: more than one other question shares the answer".to_string())
        }
        QuestionType::ConsecIdent => {
            let pairs = (0..n.saturating_sub(1))
                .filter(|&i| sol[i] == sol[i + 1])
                .count();
            (pairs > 1).then(|| "ConsecIdent: more than one adjacent-equal pair".to_string())
        }
        QuestionType::OnlyOdd { answer } | QuestionType::OnlyEven { answer } => {
            let parity = usize::from(matches!(qt, QuestionType::OnlyOdd { .. }));
            let matches = (0..n)
                .filter(|&i| (i + 1) % 2 == parity && sol[i] == answer)
                .count();
            (matches > 1).then(|| {
                format!(
                    "{:?}: more than one match in the parity positions",
                    qt.kind()
                )
            })
        }
        QuestionType::NoOtherHasAnswer => {
            // Well-posed iff the answer letter appears exactly once (so "no other has
            // it" is true) AND no *other* candidate letter is absent from the key —
            // an absent letter is vacuously "held by no other" too, a rival answer.
            if c[sol[qi].idx()] != 1 {
                return Some("NoOtherHasAnswer: the answer letter is not unique".to_string());
            }
            (0..oc).find(|&l| l != sol[qi].idx() && c[l] == 0).map(|l| {
                format!(
                    "NoOtherHasAnswer: letter {} also appears in no question — ambiguous",
                    Answer::from(l as u8).as_char()
                )
            })
        }
        _ => None,
    }
}

/// Well-posedness that depends on the filled option/claim values. `None` if `qi`
/// has a unique answer; `Some(reason)` if a distractor is *also* valid (SameAs /
/// SameAsWhich) or the true-claim count isn't exactly one (TrueStmt). Called at the
/// accept-gate, the repair keep-gate, and `refpuzzle check`.
pub fn check_well_posed_given_options(
    fp: &FlatPuzzle,
    sol: &[Answer],
    qi: usize,
) -> Option<String> {
    match fp.question_types[qi] {
        QuestionType::SameAs => ambiguating_distractor(fp, qi, sol, sol[qi], qi),
        QuestionType::SameAsWhich { question_index } => {
            let ref_q = usize::from(question_index);
            // A malformed puzzle could put the ref out of range, which would panic on
            // sol[ref_q]; reporting a bad index is form validation's job, so here we
            // just avoid the panic.
            if ref_q >= fp.n {
                return None;
            }
            ambiguating_distractor(fp, qi, sol, sol[ref_q], ref_q)
        }
        QuestionType::TrueStmt => {
            let state = State {
                // `sol` may be longer than `fp.n` (some callers pass the full MAX_N
                // key), so cap — slots past `fp.n` must stay None for the histogram.
                answers: std::array::from_fn(|i| (i < fp.n).then(|| sol[i])),
                eliminated: [fp.initial_eliminated_mask(); MAX_N],
            };
            // A claim is true iff it holds against the actual solution (qi at its real
            // answer — not the claim's slot, which would alter the histogram). Exactly
            // one may be true; a missing claim is malformed — report it.
            let mut true_claims = 0;
            for oi in 0..fp.option_count {
                let Some(claim) = fp.claim_at(qi, oi) else {
                    return Some(format!("TrueStmt option {oi} has no claim"));
                };
                if check_claim(fp, state, OptionPos { qi, oi }, claim) == Validity::Valid {
                    true_claims += 1;
                }
            }
            (true_claims != 1)
                .then(|| format!("TrueStmt has {true_claims} true claims; need exactly 1"))
        }
        _ => None,
    }
}

/// SameAs/SameAsWhich helper: find a distractor that is *also* a valid answer — an
/// option (other than `qi` or the reference `ref_q`) pointing to a question whose
/// answer equals `matched`. Returns the ambiguity reason, else `None`.
fn ambiguating_distractor(
    fp: &FlatPuzzle,
    qi: usize,
    sol: &[Answer],
    matched: Answer,
    ref_q: usize,
) -> Option<String> {
    let answer_slot = sol[qi].idx();
    for oi in 0..fp.option_count {
        if oi == answer_slot {
            continue;
        }
        let v = fp.options[qi][oi];
        if v.is_num() {
            let t = usize::from(v.value());
            if t < fp.n && t != qi && t != ref_q && sol[t] == matched {
                return Some(format!(
                    "{:?} distractor (option {oi}) points to Q{} which shares the matched answer {}",
                    fp.question_types[qi].kind(),
                    t + 1,
                    matched.as_char(),
                ));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a solution from a letter string, e.g. `sol("AAB")` → [A, A, B].
    fn sol(s: &str) -> Vec<Answer> {
        s.bytes().map(|b| Answer::from(b - b'A')).collect()
    }

    fn key(qt: QuestionType, s: &str, oc: usize) -> Option<String> {
        let sol = sol(s);
        check_well_posed_given_key(sol.len(), oc, &sol, 0, qt)
    }

    #[test]
    fn most_common_needs_a_unique_maximum() {
        assert!(key(QuestionType::MostCommon, "AAABC", 3).is_none()); // A:3 unique
        assert!(key(QuestionType::MostCommon, "AABBC", 3).is_some()); // A:2 B:2 tie
    }

    #[test]
    fn least_common_needs_a_unique_minimum() {
        assert!(key(QuestionType::LeastCommon, "AABBC", 3).is_none()); // C:1 unique
        assert!(key(QuestionType::LeastCommon, "AABC", 3).is_some()); // B:1 C:1 tie
    }

    #[test]
    fn equal_count_allows_at_most_one_other_tying_letter() {
        let x = QuestionType::EqualCount { answer: Answer::A };
        assert!(key(x, "AAB", 3).is_none()); // A:2 B:1 C:0 — none ties → "none"
        assert!(key(x, "AABB", 3).is_none()); // A:2 B:2 — B ties → B
        assert!(key(x, "AABBCC", 3).is_some()); // A:2 B:2 C:2 — two tie
    }

    #[test]
    fn only_same_needs_at_most_one_sharer() {
        // Q0=A; one other A → OnlySame well-posed. n=3, oc=3.
        assert!(key(QuestionType::OnlySame, "AAB", 3).is_none());
        // Q0=A; two other A → ambiguous.
        assert!(key(QuestionType::OnlySame, "AAA", 3).is_some());
    }

    #[test]
    fn no_other_needs_unique_and_no_absent_rival() {
        // A:1 B:1 C:1 — answer A unique, every candidate present → well-posed.
        assert!(key(QuestionType::NoOtherHasAnswer, "ABC", 3).is_none());
        // A:1 B:2 C:0 — C absent → vacuous rival, ambiguous.
        assert!(key(QuestionType::NoOtherHasAnswer, "ABB", 3).is_some());
        // A:2 ... — answer letter not unique.
        assert!(key(QuestionType::NoOtherHasAnswer, "AAB", 3).is_some());
    }

    // ── distractor-side ──

    fn build_fp(
        question_types: [QuestionType; MAX_N],
        options: [[OptionValue; 5]; MAX_N],
        true_stmt_question_types: Option<[QuestionType; 5]>,
        n: usize,
        option_count: usize,
    ) -> FlatPuzzle {
        let (affected_by, global_indices) = FlatPuzzle::build_deps(&question_types, n);
        FlatPuzzle {
            question_types,
            options,
            true_stmt_question_types,
            affected_by,
            global_indices,
            n,
            option_count,
            initial_state: State::initial(option_count),
        }
    }

    #[test]
    fn same_as_flags_a_distractor_that_shares_the_answer() {
        // sol: Q0=Q1=Q2=A, Q3=B. Q0 answered A (slot 0). Correct option → Q1; a
        // distractor → Q2, which also answers A → ambiguous.
        let sol = [Answer::A, Answer::A, Answer::A, Answer::B];
        let mut qts = [QuestionType::AnswerIsSelf; MAX_N];
        qts[0] = QuestionType::SameAs;
        let mut opts = [[OptionValue::UNUSED; 5]; MAX_N];
        opts[0][0] = OptionValue::num(1); // answer slot (skipped) → genuine sharer Q1
        opts[0][1] = OptionValue::num(2); // distractor → Q2, ALSO shares A
        opts[0][2] = OptionValue::num(3); // distractor → Q3 (B)
        let fp = build_fp(qts, opts, None, 4, 3);
        assert!(check_well_posed_given_options(&fp, &sol, 0).is_some());
    }

    #[test]
    fn same_as_accepts_distractors_that_point_elsewhere() {
        let sol = [Answer::A, Answer::A, Answer::B, Answer::C];
        let mut qts = [QuestionType::AnswerIsSelf; MAX_N];
        qts[0] = QuestionType::SameAs;
        let mut opts = [[OptionValue::UNUSED; 5]; MAX_N];
        opts[0][0] = OptionValue::num(1); // answer slot → genuine sharer Q1
        opts[0][1] = OptionValue::num(2); // distractor → Q2 (B)
        opts[0][2] = OptionValue::num(3); // distractor → Q3 (C)
        let fp = build_fp(qts, opts, None, 4, 3);
        assert!(check_well_posed_given_options(&fp, &sol, 0).is_none());
    }

    #[test]
    fn same_as_which_compares_against_the_referenced_question() {
        let sol = [Answer::B, Answer::A, Answer::A, Answer::A];
        let mut qts = [QuestionType::AnswerIsSelf; MAX_N];
        qts[0] = QuestionType::SameAsWhich { question_index: 1 };
        let mut opts = [[OptionValue::UNUSED; 5]; MAX_N];
        opts[0][0] = OptionValue::num(2); // distractor → Q2, shares matched answer A
        opts[0][1] = OptionValue::num(3); // answer slot (B → idx 1), skipped
        opts[0][2] = OptionValue::num(0); // distractor → Q0 itself, ignored
        let fp = build_fp(qts, opts, None, 4, 3);
        assert!(check_well_posed_given_options(&fp, &sol, 0).is_some());
    }

    /// TrueStmt row with `CountVowel`/`CountConsonant` claims over `sol = [A, B, C]`
    /// (1 vowel, 2 consonants).
    fn true_stmt_fp(claim_types: [QuestionType; 3], values: [u8; 3]) -> FlatPuzzle {
        let mut qts = [QuestionType::AnswerIsSelf; MAX_N];
        qts[0] = QuestionType::TrueStmt;
        let mut opts = [[OptionValue::UNUSED; 5]; MAX_N];
        for oi in 0..3 {
            opts[0][oi] = OptionValue::num(values[oi]);
        }
        let mut stmt_types = [QuestionType::AnswerIsSelf; 5];
        stmt_types[..3].copy_from_slice(&claim_types);
        build_fp(qts, opts, Some(stmt_types), 3, 3)
    }

    #[test]
    fn true_stmt_requires_exactly_one_true_claim() {
        use QuestionType::{CountConsonant, CountVowel};
        let sol = [Answer::A, Answer::B, Answer::C];
        // Exactly one true (vowels == 1).
        let fp = true_stmt_fp([CountVowel, CountVowel, CountConsonant], [1, 2, 0]);
        assert!(check_well_posed_given_options(&fp, &sol, 0).is_none());
        // Two true (vowels == 1 and consonants == 2).
        let fp = true_stmt_fp([CountVowel, CountConsonant, CountVowel], [1, 2, 0]);
        assert!(check_well_posed_given_options(&fp, &sol, 0).is_some());
        // Zero true.
        let fp = true_stmt_fp([CountVowel, CountVowel, CountConsonant], [0, 2, 0]);
        assert!(check_well_posed_given_options(&fp, &sol, 0).is_some());
    }

    #[test]
    fn true_stmt_reports_a_missing_claim() {
        let sol = [Answer::A, Answer::B, Answer::C];
        let mut qts = [QuestionType::AnswerIsSelf; MAX_N];
        qts[0] = QuestionType::TrueStmt;
        let mut opts = [[OptionValue::UNUSED; 5]; MAX_N];
        opts[0][0] = OptionValue::num(1);
        opts[0][1] = OptionValue::num(2);
        // opts[0][2] stays UNUSED — the missing claim
        let mut stmt_types = [QuestionType::AnswerIsSelf; 5];
        stmt_types[0] = QuestionType::CountVowel;
        stmt_types[1] = QuestionType::CountVowel;
        let fp = build_fp(qts, opts, Some(stmt_types), 3, 3);
        assert!(check_well_posed_given_options(&fp, &sol, 0).is_some());
    }
}
