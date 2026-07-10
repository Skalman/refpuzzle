use arrayvec::ArrayVec;

use crate::check_answer::{Validity, check_answer};
use crate::deduce::{DeduceAction, DeduceResult, deduce, deduce_fast};
use crate::types::*;

#[derive(Clone, Debug)]
pub struct LookaheadResult {
    pub eliminate_qi: usize,
    pub eliminate_oi: usize,
    #[allow(dead_code)] // used by explain layer
    pub assumption_qi: usize,
    #[allow(dead_code)] // used by explain layer
    pub assumption_answer: Answer,
    #[allow(dead_code)] // used by explain layer
    pub chain: ArrayVec<DeduceResult, 80>,
    #[allow(dead_code)] // used by explain layer
    pub contradiction_qi: usize,
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
    full: bool,
    deduce_calls: &mut u32,
) -> Option<LookaheadResult> {
    for qi in 0..fp.n {
        if state.answers[qi].is_some() {
            continue;
        }
        for oi in 0..5usize {
            if (state.eliminated[qi] >> oi) & 1 == 1 {
                continue;
            }
            if let Some(r) = probe_candidate(
                fp,
                state,
                qi,
                oi,
                lookahead_deduce_until,
                full,
                deduce_calls,
            ) {
                return Some(r);
            }
        }
    }
    None
}

/// Probe *every* candidate to a full fixpoint (unbounded, full `deduce`) and
/// return the elimination whose contradiction chain has the fewest deductions —
/// the shortest, most explainable hint. Ties break toward the first candidate in
/// (question, option) order. Drives the browser hint engine; unbounded depth also
/// makes it as strong as any puzzle generation accepts.
// Only caller is the wasm `lookaheadShortest` export, so it's dead in native builds.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn lookahead_shortest(fp: &FlatPuzzle, state: &State) -> Option<LookaheadResult> {
    let mut best: Option<LookaheadResult> = None;
    for qi in 0..fp.n {
        if state.answers[qi].is_some() {
            continue;
        }
        for oi in 0..5usize {
            if (state.eliminated[qi] >> oi) & 1 == 1 {
                continue;
            }
            if let Some(r) = probe_candidate(fp, state, qi, oi, usize::MAX, true, &mut 0)
                && best.as_ref().is_none_or(|b| r.chain.len() < b.chain.len())
            {
                best = Some(r);
            }
        }
    }
    best
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
    full: bool,
    deduce_calls: &mut u32,
) -> Option<LookaheadResult> {
    let n = fp.n;
    let mut hyp = *state;
    hyp.answers[qi] = Some(Answer::from(oi as u8));
    hyp.eliminated[qi] = 0b11111 ^ (1 << oi);

    let mut chain = ArrayVec::new();
    let mut contradiction_qi = None;
    while chain.len() < lookahead_deduce_until {
        *deduce_calls += 1;
        let mut drs = if full {
            deduce(fp, &hyp)
        } else {
            deduce_fast(fp, &hyp)
        };
        if drs.is_empty() {
            break;
        }
        drs.sort_by_key(|dr| dr.rule as u8);
        for dr in &drs {
            // A rule whose conclusion conflicts with `hyp` refutes the hypothesis.
            // Report the question where the conflict surfaces (the action's target),
            // not the assumption `qi` — the explain layer renders "Q{contradiction_qi}
            // would be invalid" against the replayed chain state, and naming the
            // assumption (which is trivially consistent) yields a false generic hint.
            if let Some(cqi) = contradiction_question(&dr.action, &hyp) {
                contradiction_qi = Some(cqi);
                break;
            }
            apply_action(&dr.action, &mut hyp);
            if !chain.is_full() {
                chain.push(*dr);
            }
        }
        if contradiction_qi.is_some() {
            break;
        }
    }

    let result = |contradiction_qi| {
        Some(LookaheadResult {
            eliminate_qi: qi,
            eliminate_oi: oi,
            assumption_qi: qi,
            assumption_answer: Answer::from(oi as u8),
            chain: chain.clone(),
            contradiction_qi,
        })
    };

    if let Some(cqi) = contradiction_qi {
        return result(cqi);
    }
    for check_qi in 0..n {
        if hyp.answers[check_qi].is_none() {
            if (!hyp.eliminated[check_qi] & 0b11111u8).count_ones() == 0 {
                return result(check_qi);
            }
            continue;
        }
        if check_answer(fp, hyp, check_qi) == Validity::Invalid {
            return result(check_qi);
        }
    }
    None
}

/// If `action`'s conclusion conflicts with `hyp` — a `Force` to a cell already
/// answered otherwise or whose target option is eliminated, or an
/// `Eliminate`/`EliminateMulti` striking a cell's current answer — the question
/// where the conflict surfaces. `None` if the action is consistent with `hyp`.
/// Sibling of `solve_deduce::contradiction_qi`, but also flags the
/// forced-onto-eliminated case that one omits.
fn contradiction_question(action: &DeduceAction, hyp: &State) -> Option<usize> {
    match *action {
        DeduceAction::Force { qi, answer } => {
            let conflicts = (hyp.answers[qi].is_some() && hyp.answers[qi] != Some(answer))
                || (hyp.eliminated[qi] >> answer.idx()) & 1 == 1;
            conflicts.then_some(qi)
        }
        DeduceAction::Eliminate { qi, oi } => {
            (hyp.answers[qi] == Some(Answer::from(oi as u8))).then_some(qi)
        }
        DeduceAction::EliminateMulti {
            question_mask,
            option_mask,
        } => {
            let mut qm = question_mask;
            while qm != 0 {
                let i = qm.trailing_zeros() as usize;
                qm &= qm - 1;
                if let Some(a) = hyp.answers[i]
                    && (option_mask >> a.idx()) & 1 == 1
                {
                    return Some(i);
                }
            }
            None
        }
    }
}

fn apply_action(action: &DeduceAction, hyp: &mut State) {
    match *action {
        DeduceAction::Force { qi, answer } => {
            hyp.eliminated[qi] = 0b11111 ^ (1 << answer.idx());
            hyp.answers[qi] = Some(answer);
        }
        DeduceAction::Eliminate { qi, oi } => {
            hyp.eliminated[qi] |= 1 << oi;
        }
        DeduceAction::EliminateMulti {
            question_mask,
            option_mask,
        } => {
            let mut qm = question_mask;
            while qm != 0 {
                let i = qm.trailing_zeros() as usize;
                qm &= qm - 1;
                hyp.eliminated[i] |= option_mask;
            }
        }
    }
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
            let fp = match fp {
                Some(fp) => fp,
                None => {
                    eprintln!("SKIP: {name}: parse failed");
                    continue;
                }
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
                        eliminated[i] = 0b11111 ^ (1 << oi);
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
                true,
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

    /// A contradiction is attributed to the conflicting action's target question,
    /// not the assumption; the forced-onto-eliminated case (which
    /// `solve_deduce::contradiction_qi` omits) is still detected.
    #[test]
    fn contradiction_question_reports_the_conflicting_target() {
        let mut st = State {
            answers: [None; MAX_N],
            eliminated: [0; MAX_N],
        };
        st.answers[3] = Some(Answer::B);
        st.eliminated[3] = 0b11111 ^ (1 << Answer::B.idx());
        st.answers[5] = Some(Answer::C);
        st.eliminated[5] = 0b11111 ^ (1 << Answer::C.idx());

        // Force onto a cell answered otherwise → that cell.
        assert_eq!(
            contradiction_question(
                &DeduceAction::Force {
                    qi: 3,
                    answer: Answer::A
                },
                &st
            ),
            Some(3)
        );
        // Force onto a cell whose target option is eliminated (cell unanswered) →
        // that cell. This is the case solve_deduce's twin misses.
        let mut st_elim = State {
            answers: [None; MAX_N],
            eliminated: [0; MAX_N],
        };
        st_elim.eliminated[2] = 1 << Answer::A.idx();
        assert_eq!(
            contradiction_question(
                &DeduceAction::Force {
                    qi: 2,
                    answer: Answer::A
                },
                &st_elim
            ),
            Some(2)
        );
        // Eliminate striking a cell's current answer → that cell.
        assert_eq!(
            contradiction_question(
                &DeduceAction::Eliminate {
                    qi: 5,
                    oi: Answer::C.idx()
                },
                &st
            ),
            Some(5)
        );
        // EliminateMulti → the lowest conflicting question in the mask (both 3 and 5
        // conflict here).
        assert_eq!(
            contradiction_question(
                &DeduceAction::EliminateMulti {
                    question_mask: (1 << 3) | (1 << 5),
                    option_mask: (1 << Answer::B.idx()) | (1 << Answer::C.idx()),
                },
                &st
            ),
            Some(3)
        );
        // Consistent actions → None (no false contradiction).
        assert_eq!(
            contradiction_question(
                &DeduceAction::Force {
                    qi: 3,
                    answer: Answer::B
                },
                &st
            ),
            None
        );
        assert_eq!(
            contradiction_question(
                &DeduceAction::Eliminate {
                    qi: 3,
                    oi: Answer::A.idx()
                },
                &st
            ),
            None
        );
    }
}
