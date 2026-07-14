//! Distractor repair for the generation pipeline — the per-question edits
//! `validate_and_repair` applies to nudge a stuck-but-unique puzzle into one the
//! engine can actually solve. `repair_one_question` edits `fp.options[qi]` in
//! place under two invariants: the correct option (`solution[qi]`) stays correct,
//! and the row stays well-formed (distinct values).

use arrayvec::ArrayVec;

use crate::check_well_posed::check_well_posed_given_options;
use crate::deduce::deduce_question;
use crate::fill::{Stats, assert_accepted, run_hint_engine_from, valid_values};
use crate::rng::Rng;
use crate::solve_brute::solve;
use crate::types::*;

/// Repeatedly run a repair pass (mutate one distractor per stuck question, gate
/// the full engine behind a `deduce` probe, keep useful edits advancing `state`)
/// until a whole pass keeps nothing or an edit completes the puzzle. Each kept
/// edit only ever adds eliminations/answers to `state`, so the fixpoint loop
/// terminates. True if an edit completed the puzzle.
pub(super) fn repair_distractors(
    fp: &mut FlatPuzzle,
    solution: &[Answer; MAX_N],
    n: usize,
    lookahead_deduce_until: usize,
    rng: &mut Rng,
    stats: &mut Stats,
    state: &mut State,
    label: &str,
) -> bool {
    loop {
        match repair_pass(
            fp,
            solution,
            n,
            lookahead_deduce_until,
            rng,
            stats,
            state,
            label,
        ) {
            PassOutcome::Solved => return true,
            PassOutcome::Changed => {} // advanced — run another pass from the new position
            PassOutcome::NoChange | PassOutcome::FailedRecheck => return false,
        }
    }
}

enum PassOutcome {
    Solved,
    Changed,
    NoChange,
    /// An edit completed the resume solve, but the emitted puzzle failed a
    /// from-scratch re-check (ambiguous, or not hint-solvable). Terminal like
    /// `NoChange` — reject and regenerate — but the offending edit is left in
    /// place rather than restored (the puzzle is discarded anyway).
    FailedRecheck,
}

/// One repair pass. Walks the stuck questions, mutates one distractor each, and
/// keeps edits that advance `state`. `Solved` if an edit completed the puzzle;
/// `Changed` if at least one edit was kept; else `NoChange` (the caller stops
/// looping).
fn repair_pass(
    fp: &mut FlatPuzzle,
    solution: &[Answer; MAX_N],
    n: usize,
    lookahead_deduce_until: usize,
    rng: &mut Rng,
    stats: &mut Stats,
    state: &mut State,
    label: &str,
) -> PassOutcome {
    // Walk all questions in randomized order (so repair doesn't bias toward any
    // particular one), skipping those with nothing to repair.
    let mut order: ArrayVec<usize, MAX_N> = (0..n).collect();
    rng.shuffle(&mut order);
    let mut changed = false;
    for qi in order {
        let qt = fp.question_types[qi];
        // Nothing to repair: already answered (perhaps by an earlier edit's cascade
        // this pass), or a type with no free distractor — identity-option types
        // (options are fixed positions) and TrueStmt (claims, not values).
        if state.answers[qi].is_some()
            || qt.has_identity_options()
            || matches!(qt, QuestionType::TrueStmt)
        {
            continue;
        }
        stats.distractor_attempts += 1;
        // repair_one_question keeps only a well-formed edit that fires `qi`'s deduce
        // gate (else it restores the row and returns false). Trust that: a kept edit
        // is worth a full engine run, so the only thing to check here is whether it
        // found one.
        if !repair_one_question(fp, qi, solution, state, rng) {
            continue;
        }
        // Resume from `state` rather than re-solve from scratch (cheaper) to decide
        // whether this edit finishes the puzzle. NOTE: a distractor edit can
        // invalidate an elimination a global rule made on the edited option, which
        // `state` still carries — so a "solved" result here is neither guaranteed
        // unique nor guaranteed reproducible from scratch. Two independent backstops
        // below re-check the emitted puzzle before accepting: brute for uniqueness,
        // and a fresh hint solve for from-scratch solvability. A rejected edit is
        // left in place (the puzzle is discarded on regenerate) rather than restored.
        let (solved, advanced_state) =
            run_hint_engine_from(fp, *state, stats, lookahead_deduce_until);
        if solved {
            let solutions = solve(fp, 2);
            if solutions.len() != 1 {
                // Not actually unique — the edit added a second valid answer.
                stats.repair_ambiguous += 1;
                return PassOutcome::FailedRecheck;
            }
            // The resume finished from `state`, whose carried eliminations this edit
            // may have invalidated — so re-solve from scratch. Accept only if the
            // hint engine solves the emitted puzzle from `initial_state`; otherwise
            // it's uniquely solvable but not hint-solvable from a fresh start.
            let (fresh_solved, _) =
                run_hint_engine_from(fp, fp.initial_state, stats, lookahead_deduce_until);
            if !fresh_solved {
                stats.repair_unsolvable += 1;
                return PassOutcome::FailedRecheck;
            }
            assert_accepted(fp, solutions.len(), label);
            stats.distractor_ok += 1;
            return PassOutcome::Solved;
        }
        *state = advanced_state; // useful edit — keep it and repair from the new position
        changed = true;
    }
    if changed {
        PassOutcome::Changed
    } else {
        PassOutcome::NoChange
    }
}

/// Repair one stuck question with no per-type heuristics: try mutating each live
/// distractor to a legal value and keep the first edit that lets `qi`'s own rules
/// make a fresh move. The logic lives entirely in `valid_values` (the legal
/// domain) and the deduce rules.
///
/// Mutates only `fp.options[qi]`, never the correct option (takes `&mut FlatPuzzle`
/// only because the deduce check below reads the whole puzzle). Returns `true` if
/// it kept an edit, `false` if nothing worked (the row is left unchanged).
///
/// A kept edit is guaranteed **well-formed** — distinct option values, via
/// `row_has_duplicate` — and to give `qi`'s own rules a move (the `deduce_question`
/// gate). It is **not** guaranteed to keep the puzzle uniquely solvable: the new
/// value can invalidate an elimination a global rule made on the old one, so the
/// puzzle may gain extra solutions. The caller confirms uniqueness with a brute
/// `solve`.
fn repair_one_question(
    fp: &mut FlatPuzzle,
    qi: usize,
    solution: &[Answer; MAX_N],
    state: &State,
    rng: &mut Rng,
) -> bool {
    // repair_pass only calls this for unanswered questions; mutating an answered
    // one would corrupt a solved cell, so flag a caller bug rather than no-op.
    assert!(
        state.answers[qi].is_none(),
        "repair_one_question on already-answered Q{}",
        qi + 1
    );
    let oc = fp.option_count;
    let n = fp.n;
    let correct_oi = solution[qi].idx();
    let qt = fp.question_types[qi];
    let domain = valid_values(&qt, qi, n, oc);

    // Live distractors (not the correct option, not already eliminated), random order.
    let mut distractors: ArrayVec<usize, 5> = (0..oc)
        .filter(|&oi| oi != correct_oi && (state.eliminated[qi] >> oi) & 1 == 0)
        .collect();
    rng.shuffle(&mut distractors);

    for oi in distractors {
        let before_value = fp.options[qi][oi];
        let mut candidates = domain.clone();
        rng.shuffle(&mut candidates);
        for v in candidates {
            if v == before_value {
                continue;
            }
            fp.options[qi][oi] = v;
            // Keep the first edit that is well-formed (no duplicate value, no
            // ambiguous match) and gives qi's rules a move.
            if !row_has_duplicate(fp, qi)
                && check_well_posed_given_options(fp, solution, qi).is_none()
                && !deduce_question(fp, state, qi).is_empty()
            {
                return true;
            }
        }
        fp.options[qi][oi] = before_value; // none worked — restore, try next distractor
    }
    false
}

/// True if two real option slots (`0..option_count`) share a value — the one
/// well-formedness hazard `repair_one_question` can introduce. Cheap (≤ oc²), so
/// it gates every edit; full `check_form` is left to the accept-time assert.
fn row_has_duplicate(fp: &FlatPuzzle, qi: usize) -> bool {
    let oc = fp.option_count;
    (0..oc).any(|a| ((a + 1)..oc).any(|b| fp.options[qi][a] == fp.options[qi][b]))
}
