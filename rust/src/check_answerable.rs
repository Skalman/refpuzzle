//! "Answerable?" — does a question have a *unique* answer given the answer key,
//! for the types whose answer is forced by the solution's letter histogram
//! (`MostCommon`/`LeastCommon`/`EqualCount`)? A tie means no unique answer exists.
//!
//! The builder consults this *before* placing one of these types and skips it if
//! the key can't make it answerable — otherwise the puzzle would be built and then
//! rejected by the key-consistency check (for `MostCommon`/`LeastCommon`) or, worse,
//! emitted ambiguous (for `EqualCount`, whose `check_answer` doesn't require a unique
//! tie). The *option*-level uniqueness of `SameAs`/`SameAsWhich`/`TrueStmt` is a
//! different concern — see [`crate::check_answer::check_unambiguous_answer`].

use crate::types::*;

/// Letter histogram of the solution's first `n` answers.
fn counts(sol: &[Answer], n: usize) -> [u8; 5] {
    let mut c = [0u8; 5];
    for &a in sol.iter().take(n) {
        c[a.idx()] += 1;
    }
    c
}

/// Whether `qt` has a unique answer given the key `sol`. Only the histogram-forced
/// count types can be *un*answerable here; every other type is always answerable
/// (its option-level uniqueness is `check_answer`'s concern).
pub fn answerable(qt: &QuestionType, sol: &[Answer], n: usize, oc: usize) -> bool {
    let c = counts(sol, n);
    match qt {
        QuestionType::MostCommon => {
            let max = c[..oc].iter().copied().max().unwrap_or(0);
            c[..oc].iter().filter(|&&x| x == max).count() == 1
        }
        QuestionType::LeastCommon => {
            let min = c[..oc].iter().copied().min().unwrap_or(0);
            c[..oc].iter().filter(|&&x| x == min).count() == 1
        }
        QuestionType::EqualCount { answer } => {
            // Answerable iff at most one *other* letter shares X's count: one tying
            // letter (that's the answer) or none (the "none" answer). Two+ ties are
            // ambiguous — and `check_answer` won't catch it.
            let ref_count = c[answer.idx()];
            (0..oc)
                .filter(|&l| l != answer.idx() && c[l] == ref_count)
                .count()
                <= 1
        }
        _ => true,
    }
}

/// Per-question wrapper for the accept-gate and `refpuzzle check`: `None` if `qi`
/// is answerable, `Some(reason)` if a histogram tie leaves it without a unique
/// answer.
pub fn check_answerable(fp: &FlatPuzzle, qi: usize, sol: &[Answer]) -> Option<String> {
    let qt = fp.question_types[qi];
    if answerable(&qt, sol, fp.n, fp.option_count) {
        return None;
    }
    Some(format!(
        "{:?} has no unique answer for this key (a letter-count tie)",
        qt.kind()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a solution from a letter string, e.g. `sol("AAB")` → [A, A, B].
    fn sol(s: &str) -> Vec<Answer> {
        s.bytes().map(|b| Answer::from(b - b'A')).collect()
    }

    #[test]
    fn most_common_needs_a_unique_maximum() {
        // A:3 B:1 C:1 — unique most-common.
        let s = sol("AAABC");
        assert!(answerable(&QuestionType::MostCommon, &s, s.len(), 3));
        // A:2 B:2 C:1 — A and B tie for most-common, so there is no answer.
        let s = sol("AABBC");
        assert!(!answerable(&QuestionType::MostCommon, &s, s.len(), 3));
    }

    #[test]
    fn least_common_needs_a_unique_minimum() {
        // A:2 B:2 C:1 — unique least-common.
        let s = sol("AABBC");
        assert!(answerable(&QuestionType::LeastCommon, &s, s.len(), 3));
        // A:2 B:1 C:1 — B and C tie for least-common.
        let s = sol("AABC");
        assert!(!answerable(&QuestionType::LeastCommon, &s, s.len(), 3));
    }

    #[test]
    fn equal_count_allows_at_most_one_other_tying_letter() {
        let x = QuestionType::EqualCount { answer: Answer::A };
        // A:2 B:1 C:0 — nothing else shares A's count: the "none" answer is unique.
        let s = sol("AAB");
        assert!(answerable(&x, &s, s.len(), 3));
        // A:2 B:2 C:0 — exactly one other letter (B) shares A's count: B is the answer.
        let s = sol("AABB");
        assert!(answerable(&x, &s, s.len(), 3));
        // A:2 B:2 C:2 — both B and C share A's count: ambiguous (this is the gap
        // `check_answer` misses for EqualCount).
        let s = sol("AABBCC");
        assert!(!answerable(&x, &s, s.len(), 3));
    }

    #[test]
    fn other_types_are_always_answerable() {
        let s = sol("AB");
        assert!(answerable(&QuestionType::CountVowel, &s, s.len(), 3));
        assert!(answerable(&QuestionType::SameAs, &s, s.len(), 3));
    }
}
