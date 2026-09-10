//! Lookahead: refute a candidate answer by assuming it and deducing to a
//! contradiction. Probes use `deduce` — sound-only, since `assume_unique` is false
//! (a false hypothesis would make uniqueness-assuming rules unsound). The two entry
//! points differ in how they pick a chain: `lookahead` takes the first refutable
//! candidate; `lookahead_shortest` scans all and returns the shortest chain, minimized
//! (drives the browser hint engine and `check`'s shortest-lookahead tier). Callers vary only the chain-length bound — generation
//! caps it at the recipe depth; verify and hints run unbounded.

use arrayvec::ArrayVec;

use crate::check_answer::{Validity, check_answer};
use crate::deduce::{
    DeduceReason, DeduceReasons, DeduceResult, apply_action, contradiction_question, deduce,
    deduce_with_reasons,
};
use crate::types::*;

/// Deductions recorded on the way to a contradiction; capacity is the largest board's
/// cell count, which a probe can't exceed.
pub(crate) type LookaheadChain = ArrayVec<DeduceResult, 80>;

/// How many of the shortest candidates `lookahead_shortest` minimizes before picking.
/// More than one because the scan ranks by *unpruned* length, which minimization can
/// reorder — a longer trace sometimes trims further than a shorter one.
const MINIMIZE_TOP_K: usize = 3;

/// The `N` best items offered, ranked by a `usize` key where lower is better, kept in
/// rank order. For a scan that would otherwise keep only the single best but whose
/// ranking key is provisional — the cheap key picks the shortlist, an expensive step then
/// re-ranks it. Ties keep the earlier offer, so a deterministic scan gives a
/// deterministic list.
struct BestN<T, const N: usize> {
    items: ArrayVec<(usize, T), N>,
}

impl<T, const N: usize> BestN<T, N> {
    fn new() -> Self {
        BestN {
            items: ArrayVec::new(),
        }
    }

    /// Keep `item` if `key` ranks it among the `N` best so far.
    fn offer(&mut self, key: usize, item: T) {
        let Some(rank) = self.items.iter().position(|(k, _)| key < *k) else {
            // Beaten by everything held, so it only belongs here while there is room.
            let _ = self.items.try_push((key, item));
            return;
        };
        if self.items.is_full() {
            self.items.pop();
        }
        self.items.insert(rank, (key, item));
    }

    /// The kept items, best first.
    fn into_items(self) -> impl Iterator<Item = T> {
        self.items.into_iter().map(|(_, item)| item)
    }
}

/// How a hypothesis was refuted at one question — paired with that question's index in
/// [`LookaheadResult`]. Recorded rather than re-derived, because the hint's closing
/// "But …. Contradiction." line states it and `Conflict` isn't recoverable from the end
/// state alone.
#[derive(Clone, Copy, Debug)]
pub enum Contradiction {
    /// The committed answer there is one `check_answer` rejects.
    AnswerInvalid,
    /// A rule concluded something the state contradicts there. `derived_from` is the state
    /// the rule fired at, which its reason has to be read against.
    Conflict {
        result: DeduceResult,
        derived_from: State,
    },
}

// Everything past the elimination is read only by the explain layer.
#[derive(Clone, Debug)]
pub struct LookaheadResult {
    pub eliminate_qi: usize,
    pub eliminate_oi: usize,
    pub assumption_qi: usize,
    pub assumption_answer: Answer,
    pub chain: LookaheadChain,
    pub contradiction_qi: usize,
    pub contradiction: Contradiction,
}

/// First eliminable option found: walk unanswered questions' live options in
/// order and return the first whose assumption reaches a contradiction within
/// `lookahead_deduce_until` deductions. Called by `run_engine` (the shared
/// deduce+lookahead solver behind generation's accept-gate and the offline
/// `check`/`solve`) — when solving a whole puzzle any single elimination advances
/// it, so first-hit is enough; hints use `lookahead_shortest` instead.
pub fn lookahead(
    fp: &FlatPuzzle,
    state: &State,
    lookahead_deduce_until: usize,
    deduce_calls: &mut u32,
) -> Option<LookaheadResult> {
    for qi in 0..fp.n {
        if state.answers[qi].is_some() {
            continue;
        }
        for oi in 0..5usize {
            if state.is_eliminated(qi, oi) {
                continue;
            }
            if let Some(r) =
                probe_candidate(fp, state, qi, oi, lookahead_deduce_until, deduce_calls)
            {
                return Some(r);
            }
        }
    }
    None
}

/// Probe *every* candidate and return the elimination whose minimized contradiction chain
/// has the fewest deductions — the `MINIMIZE_TOP_K` shortest get minimized, and the best
/// of those wins.
pub fn lookahead_shortest(
    fp: &FlatPuzzle,
    state: &State,
    lookahead_deduce_until: usize,
    deduce_calls: &mut u32,
) -> Option<LookaheadResult> {
    // Try assuming each live option in turn, shortlisting the refutations by raw length.
    let mut leaders: BestN<LookaheadResult, MINIMIZE_TOP_K> = BestN::new();
    for qi in 0..fp.n {
        if state.answers[qi].is_some() {
            continue;
        }
        for oi in 0..5usize {
            if state.is_eliminated(qi, oi) {
                continue;
            }
            if let Some(r) =
                probe_candidate(fp, state, qi, oi, lookahead_deduce_until, deduce_calls)
            {
                leaders.offer(r.chain.len(), r);
            }
        }
    }

    // Then minimize the shortlist and re-rank, since trimming reorders it.
    let mut best: Option<LookaheadResult> = None;
    for mut r in leaders.into_items() {
        minimize_chain(fp, state, &mut r, deduce_calls);
        if best.as_ref().is_none_or(|b| r.chain.len() < b.chain.len()) {
            best = Some(r);
        }
    }
    best
}

/// Trim a chain down to what the contradiction needs. `probe_candidate` logs everything a
/// round derived, in rule order, so entries can precede a conflict that was already
/// visible or have no bearing on it at all.
fn minimize_chain(
    fp: &FlatPuzzle,
    state: &State,
    result: &mut LookaheadResult,
    deduce_calls: &mut u32,
) {
    // Lands on a chain no single step can be removed from, not the shortest that exists.
    let mut shrank = true;
    while shrank {
        shrank = false;
        // Back to front: late steps are the likeliest to be droppable, and each drop makes
        // every later replay cheaper. Front to back measures worse on both.
        for idx in (0..result.chain.len()).rev() {
            shrank |= drop_step_unless_needed(fp, state, result, deduce_calls, idx);
        }
    }
}

/// Drop the step at `idx` unless the contradiction needs it. Returns whether it went,
/// leaving `result.contradiction` matching whatever chain remains.
fn drop_step_unless_needed(
    fp: &FlatPuzzle,
    state: &State,
    result: &mut LookaheadResult,
    deduce_calls: &mut u32,
    idx: usize,
) -> bool {
    let step = result.chain.remove(idx);
    match chain_contradiction(fp, state, result, deduce_calls) {
        // Still refutes without the step, though possibly by a different mechanism.
        Some(contradiction) => {
            result.contradiction = contradiction;
            true
        }
        // The step was load-bearing, so the chain keeps it.
        None => {
            result.chain.insert(idx, step);
            false
        }
    }
}

/// Does the chain still hold up? Every step has to be derivable at the point it is
/// applied, and the question it blames has to still be broken at the end. Returns how that
/// question is broken, or `None` if either check fails.
///
/// Every chain this module reports passes this, and `minimize_chain` uses it to decide
/// whether a step can go.
pub(crate) fn chain_contradiction(
    fp: &FlatPuzzle,
    state: &State,
    result: &LookaheadResult,
    deduce_calls: &mut u32,
) -> Option<Contradiction> {
    let mut hyp = hypothesis(state, result.assumption_qi, result.assumption_answer);
    // Keep the last round's pre-state: `contradiction_at` needs it to spot a clash the
    // chain's own steps have since hidden.
    let mut last_round_pre = None;
    if !replay_chain(
        fp,
        &mut hyp,
        &result.chain,
        deduce_calls,
        |round_pre, _, _| {
            last_round_pre = Some(*round_pre);
        },
    ) {
        return None;
    }
    contradiction_at(
        fp,
        &hyp,
        last_round_pre.as_ref(),
        result.contradiction_qi,
        deduce_calls,
    )
}

/// Re-apply `chain` to `hyp`, handing every step to `on_step` along with the state its
/// reason has to be read against and the [`DeduceReason`] it re-derived — the chain
/// itself carries none (it must stay `Copy` and equality-matchable), so this replay is
/// where the explain path collects them. `false` if the chain doesn't hold together.
pub(crate) fn replay_chain(
    fp: &FlatPuzzle,
    hyp: &mut State,
    chain: &[DeduceResult],
    deduce_calls: &mut u32,
    mut on_step: impl FnMut(&State, &DeduceResult, DeduceReason),
) -> bool {
    let mut applied = 0;
    while applied < chain.len() {
        // One round: everything it covers was derived from this one pre-state.
        let round_pre = *hyp;
        *deduce_calls += 1;
        let mut reasons = DeduceReasons::new();
        let results = deduce_with_reasons(fp, &round_pre, &mut reasons);
        // Sorted as pairs so each result keeps its reason (stable, so same-rule
        // entries stay in emission order — the same batch `probe_candidate` built).
        let mut batch: ArrayVec<(DeduceResult, DeduceReason), 80> =
            results.iter().copied().zip(reasons).collect();
        batch.sort_by_key(|(dr, _)| dr.rule as u8);

        let round_start = applied;
        for (dr, reason) in &batch {
            if applied == chain.len() {
                break;
            }
            // Matching in batch order rather than searching keeps the rounds identical to
            // the ones `probe_candidate` built, so an untrimmed chain replays as recorded.
            if *dr != chain[applied] {
                continue;
            }
            // Applying this would overwrite the very thing it contradicts, losing it.
            if contradiction_question(&dr.action, hyp).is_some() {
                return false;
            }
            on_step(&round_pre, dr, *reason);
            apply_action(&dr.action, hyp);
            applied += 1;
        }

        // Nothing matched: the next step isn't derivable here, so its reason would be a
        // non-sequitur. What stops a step being dropped that a later one needed.
        if applied == round_start {
            return false;
        }
    }
    true
}

/// How `qi` is broken in `hyp`, the state a chain replay ends in, or `None` if it isn't.
/// `last_round_pre` is the pre-state of the round the chain's final step came from, and
/// `None` for an empty chain.
///
/// The same two mechanisms `probe_candidate` reports, reached the other way round. There
/// the chain is being built, so a refuting action is in hand at the moment it is applied
/// and one question after another falls out; here the chain is already applied and the
/// blamed question already chosen, so a `Conflict` has to be recovered by deducing again.
fn contradiction_at(
    fp: &FlatPuzzle,
    hyp: &State,
    last_round_pre: Option<&State>,
    qi: usize,
    deduce_calls: &mut u32,
) -> Option<Contradiction> {
    // Readable off the end state, so it comes before paying for a deduce. Note this is the
    // opposite preference to `probe_candidate`, which reports a `Conflict` it met mid-loop
    // even where the answer is invalid too.
    if check_answer(fp, *hyp, qi) == Validity::Invalid {
        return Some(Contradiction::AnswerInvalid);
    }

    // Otherwise the refutation is a rule concluding something `hyp` contradicts at `qi`.
    let mut conflict_derived_at = |from: &State| {
        *deduce_calls += 1;
        deduce(fp, from)
            .iter()
            .find(|dr| contradiction_question(&dr.action, hyp) == Some(qi))
            .map(|dr| Contradiction::Conflict {
                result: *dr,
                derived_from: *from,
            })
    };
    // Prefer one derivable here and now: that one a player can reach from the state the
    // chain leaves them in.
    if let Some(found) = conflict_derived_at(hyp) {
        return Some(found);
    }
    // Failing that, the batch the last step came from. A round can force an option and
    // eliminate it at the same time, and once the force is applied the eliminating rule
    // stops firing, so only the earlier batch still shows the clash. Sound either way: a
    // conclusion drawn earlier survives the steps applied after it.
    last_round_pre.and_then(conflict_derived_at)
}

/// `state` with `answer` committed at `qi`: what a probe and its replays start from.
pub(crate) fn hypothesis(state: &State, qi: usize, answer: Answer) -> State {
    let mut hyp = *state;
    hyp.answers[qi] = Some(answer);
    hyp.eliminated[qi] = ALL_OPTIONS_MASK ^ (1 << answer.idx());
    hyp
}

/// Chain-replay internals for the bin's corpus replay test. Test builds only.
#[cfg(any(test, feature = "test-util"))]
pub mod test_hooks {
    use super::{Contradiction, LookaheadResult};
    use crate::deduce::{DeduceReason, DeduceResult};
    use crate::types::*;

    pub fn chain_contradiction(
        fp: &FlatPuzzle,
        state: &State,
        result: &LookaheadResult,
        deduce_calls: &mut u32,
    ) -> Option<Contradiction> {
        super::chain_contradiction(fp, state, result, deduce_calls)
    }

    pub fn replay_chain(
        fp: &FlatPuzzle,
        hyp: &mut State,
        chain: &[DeduceResult],
        deduce_calls: &mut u32,
        on_step: impl FnMut(&State, &DeduceResult, DeduceReason),
    ) -> bool {
        super::replay_chain(fp, hyp, chain, deduce_calls, on_step)
    }

    pub fn hypothesis(state: &State, qi: usize, answer: Answer) -> State {
        super::hypothesis(state, qi, answer)
    }
}

/// Assume `oi` is the answer to `qi` and deduce forward, stopping once the chain
/// reaches `lookahead_deduce_until` results: if it hits a contradiction (a rule
/// contradicting the hypothesis, a question with no options left, or an invalid
/// answer), return the elimination of `(qi, oi)` with the deduction chain that led
/// there; otherwise `None`.
fn probe_candidate(
    fp: &FlatPuzzle,
    state: &State,
    qi: usize,
    oi: usize,
    lookahead_deduce_until: usize,
    deduce_calls: &mut u32,
) -> Option<LookaheadResult> {
    let n = fp.n;
    let mut hyp = hypothesis(state, qi, Answer::from(oi as u8));

    let mut chain = LookaheadChain::new();
    let mut refutation = None;
    while chain.len() < lookahead_deduce_until {
        *deduce_calls += 1;
        let mut drs = deduce(fp, &hyp);
        if drs.is_empty() {
            break;
        }
        // Sort by rule ordinal so the chain (and thus which contradiction surfaces
        // first) is deterministic, independent of deduce()'s emission order.
        // `run_engine` applies its batch unsorted — it needs only the fixpoint, not a
        // stable chain — so the asymmetry is deliberate.
        drs.sort_by_key(|dr| dr.rule as u8);
        let round_pre = hyp;
        for dr in &drs {
            // A rule whose conclusion conflicts with `hyp` refutes the hypothesis. Blame
            // the question the conflict surfaces at, not the assumption — the assumption
            // is trivially consistent, so naming it yields a false hint. The refuting
            // deduction stays *out* of the chain: the loop bound counts chain entries, so
            // pushing it would change how deep a bounded probe may go.
            if let Some(cqi) = contradiction_question(&dr.action, &hyp) {
                refutation = Some((
                    cqi,
                    Contradiction::Conflict {
                        result: *dr,
                        derived_from: round_pre,
                    },
                ));
                break;
            }
            apply_action(&dr.action, &mut hyp);
            if !chain.is_full() {
                chain.push(*dr);
            } else {
                unreachable!(
                    "lookahead chain exceeded capacity — a probe's deductions are bounded by the board's cell count"
                )
            }
        }
        if refutation.is_some() {
            break;
        }
    }

    // No rule conflicted mid-loop, so sweep for a question the fixpoint has broken — one
    // `check_answer` rejects, which covers a question left with no options (`NoOptionsLeft`).
    // The fixpoint shouldn't be able to leave one: `contradiction_question` catches the
    // elimination that would take a question's last option before it lands, and a board
    // arriving that way is one the player-facing callers reject first.
    let refutation = refutation.or_else(|| {
        (0..n).find_map(|check_qi| {
            (check_answer(fp, hyp, check_qi) == Validity::Invalid)
                .then_some((check_qi, Contradiction::AnswerInvalid))
        })
    });

    let (contradiction_qi, contradiction) = refutation?;
    Some(LookaheadResult {
        eliminate_qi: qi,
        eliminate_oi: oi,
        assumption_qi: qi,
        assumption_answer: Answer::from(oi as u8),
        chain,
        contradiction_qi,
        contradiction,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn test_shared_lookahead() {
        let json_str = std::fs::read_to_string("../tests/lookahead.json")
            .expect("can't read tests/lookahead.json");
        let suite: Value = serde_json::from_str(&json_str).unwrap();
        let tests = suite["tests"].as_array().unwrap();

        let mut passed = 0;
        let mut failed = 0;

        for test in tests {
            if test.get("section").is_some() {
                continue;
            }
            let name = test["name"].as_str().unwrap();
            let states = test["state"].as_array().unwrap();
            let expect = test["expect"].as_str();

            let fp = crate::serialize::parse_puzzle(&test["puzzle"]);
            let Some(fp) = fp else {
                eprintln!("SKIP: {name}: parse failed");
                continue;
            };

            let n = fp.n;
            let mut answers: [Option<Answer>; MAX_N] = [None; MAX_N];
            let mut eliminated = [fp.initial_eliminated_mask(); MAX_N];
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

            let result = lookahead(
                &fp,
                &State {
                    answers,
                    eliminated,
                },
                usize::MAX,
                &mut 0,
            );
            let got = match result {
                Some(r) => format!(
                    "{}{}",
                    r.eliminate_qi + 1,
                    (b'a' + r.eliminate_oi as u8) as char
                ),
                None => "null".to_string(),
            };
            let expected = expect.unwrap_or("null");

            if got == expected {
                passed += 1;
            } else {
                failed += 1;
                eprintln!("FAIL: {name}");
                eprintln!("  expected: {expected}");
                eprintln!("  got:      {got}");
            }
        }

        eprintln!("{passed}/{} passed", passed + failed);
        assert_eq!(failed, 0, "{failed} test(s) failed");
    }
}
