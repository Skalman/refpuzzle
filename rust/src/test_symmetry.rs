//! Symmetry property tests: a transform that relabels a puzzle must commute with
//! solving it — solving the transformed puzzle yields the transformed result.
//! Two transforms — `mirror_positional` (reverse question order, `i -> n-1-i`)
//! and `reverse_letters` (relabel answers, `x -> (oc-1)-x`) — are each checked
//! over the crafted corpus and every daily puzzle against two engines: the brute
//! solver's solution set and the pure-`deduce` fixpoint must both map through
//! the transform.
//!
//! Deduce runs without lookahead, which would otherwise compensate for an
//! asymmetric rule and hide it. Both checks compare sets/marks, not hint traces,
//! so neither depends on search order.

#![cfg(test)]

use std::collections::BTreeSet;

use serde_json::Value;

use crate::daily_puzzles;
use crate::deduce::{apply_action, deduce};
use crate::serialize::parse_puzzle;
use crate::solve_brute::solve;
use crate::test_util::fast_tests;
use crate::types::*;

/// Test-corpus files whose `tests[].puzzle` entries are well-formed puzzles.
const CORPUS: &[&str] = &[
    "../tests/solve.json",
    "../tests/deduce.json",
    "../tests/lookahead.json",
    "../tests/check-answer.json",
];

/// Brute-check skips puzzles with more solutions than this, so the two sides'
/// full solution sets stay comparable (deduce still covers them).
const SOLUTION_LIMIT: usize = 16;

fn build_flat(
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

/// Complete solution set (position -> letter index). `None` above
/// `SOLUTION_LIMIT`, so only fully-comparable sets are returned.
fn solution_set(fp: &FlatPuzzle) -> Option<BTreeSet<Vec<u8>>> {
    let sols = solve(fp, SOLUTION_LIMIT + 1);
    if sols.len() > SOLUTION_LIMIT {
        return None;
    }
    Some(
        sols.iter()
            .map(|sol| (0..fp.n).map(|i| sol[i].idx() as u8).collect())
            .collect(),
    )
}

// ── Positional mirror (i -> n-1-i) ──

/// Remap a positional option value: `v -> n-1-v`, except `ConsecIdent`'s value
/// is a pair's left index, so `v -> n-2-v`. Counts, letters, NONE, and
/// out-of-range values pass through; slots aren't reversed (a positional mirror
/// leaves answer letters, hence slots, fixed).
fn mirror_option_value(qt: &QuestionType, ov: OptionValue, n: usize) -> OptionValue {
    use QuestionType::*;
    if !ov.is_num() {
        return ov;
    }
    let v = ov.value() as usize;
    let mapped = match qt {
        ConsecIdent if v + 1 < n => n - 2 - v,
        ClosestAfter { .. }
        | ClosestBefore { .. }
        | FirstWith { .. }
        | LastWith { .. }
        | PrevSame
        | NextSame
        | SameAs
        | OnlySame
        | SameAsWhich { .. }
        | OnlyOdd { .. }
        | OnlyEven { .. }
            if v < n =>
        {
            n - 1 - v
        }
        _ => return ov,
    };
    OptionValue::num(mapped as u8)
}

/// Mirror a question type across `i -> n-1-i`: directional kinds swap, index
/// refs remap, `OnlyOdd`/`OnlyEven` swap iff `n` is even (position parity flips
/// under reversal exactly then). `TrueStmt` keeps its kind; its claims are
/// mirrored separately.
fn mirror_qtype(qt: QuestionType, n: usize) -> QuestionType {
    use QuestionType::*;
    let m = |idx: u8| (n as u8) - 1 - idx;
    let parity_flips = n.is_multiple_of(2);
    match qt {
        ClosestAfter {
            after_index,
            answer,
        } => ClosestBefore {
            before_index: m(after_index),
            answer,
        },
        ClosestBefore {
            before_index,
            answer,
        } => ClosestAfter {
            after_index: m(before_index),
            answer,
        },
        CountAnswerAfter {
            after_index,
            answer,
        } => CountAnswerBefore {
            before_index: m(after_index),
            answer,
        },
        CountAnswerBefore {
            before_index,
            answer,
        } => CountAnswerAfter {
            after_index: m(before_index),
            answer,
        },
        PrevSame => NextSame,
        NextSame => PrevSame,
        FirstWith { answer } => LastWith { answer },
        LastWith { answer } => FirstWith { answer },
        OnlyOdd { answer } if parity_flips => OnlyEven { answer },
        OnlyEven { answer } if parity_flips => OnlyOdd { answer },
        AnswerOf { question_index } => AnswerOf {
            question_index: m(question_index),
        },
        LetterDist { question_index } => LetterDist {
            question_index: m(question_index),
        },
        SameAsWhich { question_index } => SameAsWhich {
            question_index: m(question_index),
        },
        // Parity-preserved OnlyOdd/OnlyEven (n odd), TrueStmt, and kinds with no
        // position/index fall through unchanged; positional option values (if
        // any) are remapped separately.
        other => other,
    }
}

fn mirror_positional(fp: &FlatPuzzle) -> FlatPuzzle {
    let n = fp.n;
    let oc = fp.option_count;
    let mut question_types = [QuestionType::AnswerIsSelf; MAX_N];
    let mut options = [[OptionValue::UNUSED; 5]; MAX_N];

    for old_i in 0..n {
        let new_i = n - 1 - old_i;
        let old_qt = fp.question_types[old_i];
        question_types[new_i] = mirror_qtype(old_qt, n);
        // A TrueStmt row's option values are per-claim; remap each with its own
        // claim type. Every other row uses the question's own type.
        for oi in 0..oc {
            let value_type = fp
                .claim_at(old_i, oi)
                .map(|c| c.question_type)
                .unwrap_or(old_qt);
            options[new_i][oi] = mirror_option_value(&value_type, fp.options[old_i][oi], n);
        }
    }
    // Single puzzle-wide TrueStmt claim-type row (if any): mirror each claim.
    let true_stmt = fp
        .true_stmt_question_types
        .map(|types| std::array::from_fn(|oi| mirror_qtype(types[oi], n)));
    build_flat(question_types, options, true_stmt, n, oc)
}

/// Mirror a full solution: `s'[n-1-i] = s[i]`, letters unchanged.
fn mirror_solution(sol: &[u8], n: usize) -> Vec<u8> {
    (0..n).map(|i| sol[n - 1 - i]).collect()
}

// ── Letter reversal (x -> (oc-1)-x) ──

/// Reverse an answer letter within the present alphabet `[0, oc)`. Panics on an
/// out-of-range letter (ill-defined, and never expected in a valid puzzle).
fn reverse_letter(a: Answer, oc: usize) -> Answer {
    let idx = a.idx();
    assert!(idx < oc, "letter index {idx} out of range for oc={oc}");
    Answer::from((oc - 1 - idx) as u8)
}

/// Question types whose option *values* are answer letters (so the values, not
/// just the slots, relabel under letter reversal).
fn option_is_letter(qt: &QuestionType) -> bool {
    use QuestionType::*;
    matches!(
        qt,
        AnswerOf { .. } | MostCommon | LeastCommon | EqualCount { .. }
    )
}

fn reverse_letters_qtype(qt: QuestionType, oc: usize) -> QuestionType {
    use QuestionType::*;
    let rev = |a| reverse_letter(a, oc);
    match qt {
        CountAnswer { answer } => CountAnswer {
            answer: rev(answer),
        },
        CountAnswerBefore {
            answer,
            before_index,
        } => CountAnswerBefore {
            answer: rev(answer),
            before_index,
        },
        CountAnswerAfter {
            answer,
            after_index,
        } => CountAnswerAfter {
            answer: rev(answer),
            after_index,
        },
        ClosestAfter {
            after_index,
            answer,
        } => ClosestAfter {
            after_index,
            answer: rev(answer),
        },
        ClosestBefore {
            before_index,
            answer,
        } => ClosestBefore {
            before_index,
            answer: rev(answer),
        },
        FirstWith { answer } => FirstWith {
            answer: rev(answer),
        },
        LastWith { answer } => LastWith {
            answer: rev(answer),
        },
        OnlyOdd { answer } => OnlyOdd {
            answer: rev(answer),
        },
        OnlyEven { answer } => OnlyEven {
            answer: rev(answer),
        },
        EqualCount { answer } => EqualCount {
            answer: rev(answer),
        },
        // Equality-, count-, distance- or option-based; no letter in the type.
        other => other,
    }
}

/// Reverse one option/claim value under letter reversal: letter-valued payloads
/// relabel `v -> (oc-1)-v`; counts, positions and distances pass through.
fn reverse_option_value(value_type: &QuestionType, ov: OptionValue, oc: usize) -> OptionValue {
    if option_is_letter(value_type) && ov.is_num() {
        OptionValue::num(oc as u8 - 1 - ov.value())
    } else {
        ov
    }
}

/// Relabel answers `x -> (oc-1)-x`. Panics on a vowel/consonant count at
/// `oc != 5` — the vowel set {A,E} is reversal-symmetric only in the full
/// alphabet, and that case is believed never generated.
fn reverse_letters(fp: &FlatPuzzle) -> FlatPuzzle {
    let n = fp.n;
    let oc = fp.option_count;
    if oc != 5 {
        let is_vc = |qt: &QuestionType| {
            matches!(qt, QuestionType::CountVowel | QuestionType::CountConsonant)
        };
        let has_vc = (0..n).any(|qi| is_vc(&fp.question_types[qi]))
            || fp
                .true_stmt_question_types
                .is_some_and(|types| types.iter().any(is_vc));
        assert!(
            !has_vc,
            "vowel/consonant count at oc={oc}: letter reversal undefined"
        );
    }

    let mut question_types = fp.question_types;
    let mut options = fp.options;
    for qi in 0..n {
        let old_qt = fp.question_types[qi];
        question_types[qi] = reverse_letters_qtype(old_qt, oc);
        // Identity-option kinds keep canonical `[0,1,2,...]`: slot i already
        // means "letter i", so answer slot rev(a) -> value rev(a) is the
        // reversed claim; slot-reversing would corrupt the row.
        if old_qt.has_identity_options() {
            continue;
        }
        // The answer letter is the option slot, so reversing letters reverses
        // slots (`oi -> oc-1-oi`). A TrueStmt row's values are per-claim (use each
        // claim's own type); every other row uses the question's own type.
        for oi in 0..oc {
            let value_type = fp
                .claim_at(qi, oi)
                .map(|c| c.question_type)
                .unwrap_or(old_qt);
            options[qi][oc - 1 - oi] = reverse_option_value(&value_type, fp.options[qi][oi], oc);
        }
    }
    // Single puzzle-wide TrueStmt claim-type row (if any): relabel each claim and
    // reverse the claim slot order to match the option-slot reversal above.
    let true_stmt = fp.true_stmt_question_types.map(|types| {
        let mut out = types;
        for oi in 0..oc {
            out[oc - 1 - oi] = reverse_letters_qtype(types[oi], oc);
        }
        out
    });
    build_flat(question_types, options, true_stmt, n, oc)
}

/// Reverse a full solution's letters: `s'[i] = (oc-1) - s[i]`, positions unchanged.
fn reverse_solution_letters(sol: &[u8], oc: usize) -> Vec<u8> {
    sol.iter().map(|&a| oc as u8 - 1 - a).collect()
}

// ── Deduce-engine fixpoint ──
//
// Run pure `deduce` to a fixpoint (no lookahead — it would mask an asymmetric
// rule) and compare the reached marks. Deduce only adds marks, so the fixpoint
// is confluent and order-independent.

fn deduce_fixpoint(fp: &FlatPuzzle) -> State {
    let mut state = fp.initial_state;
    // Each pass sets at least one bit until convergence; the cap only guards
    // against a hypothetical non-idempotent rule re-emitting forever.
    for _ in 0..(MAX_N * 8) {
        let before = state;
        for dr in deduce(fp, &state).iter() {
            apply_action(&dr.action, &mut state);
        }
        if state.answers[..fp.n] == before.answers[..fp.n]
            && state.eliminated[..fp.n] == before.eliminated[..fp.n]
        {
            break;
        }
    }
    state
}

/// Reverse the low `oc` bits of an option-slot mask (`oi -> oc-1-oi`); bits at
/// or above `oc` (phantom slots) pass through unchanged.
fn reverse_mask(mask: u8, oc: usize) -> u8 {
    let mut out = mask & !(((1u16 << oc) - 1) as u8);
    for oi in 0..oc {
        if mask & (1 << oi) != 0 {
            out |= 1 << (oc - 1 - oi);
        }
    }
    out
}

/// Positional mirror of a deduce state: reverse the position order. Answer
/// letters and per-slot elimination masks are unchanged (positions move, slots
/// don't).
fn mirror_state(s: &State, n: usize) -> State {
    let mut out = *s;
    for i in 0..n {
        out.answers[i] = s.answers[n - 1 - i];
        out.eliminated[i] = s.eliminated[n - 1 - i];
    }
    out
}

/// Letter reversal of a deduce state: relabel each answer and bit-reverse each
/// elimination mask; positions unchanged.
fn reverse_state_letters(s: &State, n: usize, oc: usize) -> State {
    let mut out = *s;
    for i in 0..n {
        out.answers[i] = s.answers[i].map(|a| reverse_letter(a, oc));
        out.eliminated[i] = reverse_mask(s.eliminated[i], oc);
    }
    out
}

fn states_eq(a: &State, b: &State, n: usize) -> bool {
    a.answers[..n] == b.answers[..n] && a.eliminated[..n] == b.eliminated[..n]
}

// ── Harness ──

fn push_corpus(out: &mut Vec<(String, FlatPuzzle)>) {
    for path in CORPUS {
        let json = std::fs::read_to_string(path).unwrap_or_else(|_| panic!("can't read {path}"));
        let suite: Value = serde_json::from_str(&json).unwrap();
        for (i, test) in suite["tests"].as_array().unwrap().iter().enumerate() {
            let Some(puzzle) = test.get("puzzle") else {
                continue;
            };
            let Some(fp) = parse_puzzle(puzzle) else {
                continue;
            };
            let name = test.get("name").and_then(|v| v.as_str()).unwrap_or("");
            out.push((format!("{path}#{i} {name}"), fp));
        }
    }
}

/// The crafted corpus, plus every daily puzzle on a full (non-fast) run —
/// brute-solving the whole daily set is too slow for the fast path.
fn all_puzzles() -> Vec<(String, FlatPuzzle)> {
    let mut out = Vec::new();
    push_corpus(&mut out);
    if !fast_tests() {
        out.extend(daily_puzzles());
    }
    out
}

/// Run both engines for one transform over the whole corpus: `solve` solution
/// sets and pure-`deduce` fixpoints must each map through the transform.
/// `map_solution`/`map_state` apply it to a solution / a deduce state.
fn check_symmetry(
    label: &str,
    transform: impl Fn(&FlatPuzzle) -> FlatPuzzle,
    map_solution: impl Fn(&[u8], usize, usize) -> Vec<u8>,
    map_state: impl Fn(&State, usize, usize) -> State,
) {
    let (mut brute_ok, mut brute_skip, mut brute_fail) = (0, 0, 0);
    let (mut deduce_ok, mut deduce_fail) = (0, 0);
    for (name, fp) in all_puzzles() {
        let transformed = transform(&fp);
        let (n, oc) = (fp.n, fp.option_count);

        // Brute: compare complete solution sets.
        match (solution_set(&fp), solution_set(&transformed)) {
            (Some(orig), Some(got)) => {
                let expected: BTreeSet<Vec<u8>> =
                    orig.iter().map(|s| map_solution(s, n, oc)).collect();
                if got == expected {
                    brute_ok += 1;
                } else {
                    brute_fail += 1;
                    eprintln!("FAIL brute ({label}): {name}");
                    eprintln!("  expected {expected:?}");
                    eprintln!("  got      {got:?}");
                }
            }
            _ => brute_skip += 1,
        }

        // Deduce: compare pure-deduce fixpoint marks.
        let expected = map_state(&deduce_fixpoint(&fp), n, oc);
        let got = deduce_fixpoint(&transformed);
        if states_eq(&got, &expected, n) {
            deduce_ok += 1;
        } else {
            deduce_fail += 1;
            eprintln!("FAIL deduce ({label}): {name}");
            eprintln!(
                "  expected answers {:?} elim {:?}",
                &expected.answers[..n],
                &expected.eliminated[..n]
            );
            eprintln!(
                "  got      answers {:?} elim {:?}",
                &got.answers[..n],
                &got.eliminated[..n]
            );
        }
    }
    eprintln!(
        "{label}: brute {brute_ok} ok / {brute_skip} unbounded / {brute_fail} fail | deduce {deduce_ok} ok / {deduce_fail} fail"
    );
    assert_eq!(brute_fail, 0, "{label}: brute solution sets diverged");
    assert_eq!(deduce_fail, 0, "{label}: deduce fixpoints diverged");
}

#[test]
fn positional_mirror_symmetry() {
    check_symmetry(
        "positional mirror",
        mirror_positional,
        |s, n, _oc| mirror_solution(s, n),
        |st, n, _oc| mirror_state(st, n),
    );
}

#[test]
fn letter_reversal_symmetry() {
    check_symmetry(
        "letter reversal",
        reverse_letters,
        |s, _n, oc| reverse_solution_letters(s, oc),
        reverse_state_letters,
    );
}
