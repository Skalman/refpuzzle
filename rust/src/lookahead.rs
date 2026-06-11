use arrayvec::ArrayVec;

use crate::check_answer::{Validity, check_answer};
use crate::deduce::{DeduceAction, DeduceResult, deduce, deduce_fast};
use crate::types::*;

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

pub fn lookahead(
    fp: &FlatPuzzle,
    state: &State,
    stop_deducing_after_n_results: usize,
    full: bool,
    deduce_calls: &mut u32,
) -> Option<LookaheadResult> {
    let n = fp.n;
    let answers = &state.answers;
    let eliminated = &state.eliminated;
    for qi in 0..n {
        if answers[qi].is_some() {
            continue;
        }
        for oi in 0..5usize {
            if (eliminated[qi] >> oi) & 1 == 1 {
                continue;
            }

            let mut hyp = *state;
            hyp.answers[qi] = Some(Answer::from(oi as u8));
            hyp.eliminated[qi] = 0b11111 ^ (1 << oi);

            let mut chain = ArrayVec::new();
            let mut contradiction = false;
            while chain.len() < stop_deducing_after_n_results {
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
                    if has_contradiction(&dr.action, &hyp) {
                        contradiction = true;
                        break;
                    }
                    apply_action(&dr.action, &mut hyp);
                    if !chain.is_full() {
                        chain.push(*dr);
                    }
                }
                if contradiction {
                    break;
                }
            }

            if contradiction {
                return Some(LookaheadResult {
                    eliminate_qi: qi,
                    eliminate_oi: oi,
                    assumption_qi: qi,
                    assumption_answer: Answer::from(oi as u8),
                    chain,
                    contradiction_qi: qi,
                });
            }

            for check_qi in 0..n {
                if hyp.answers[check_qi].is_none() {
                    if (!hyp.eliminated[check_qi] & 0b11111u8).count_ones() == 0 {
                        return Some(LookaheadResult {
                            eliminate_qi: qi,
                            eliminate_oi: oi,
                            assumption_qi: qi,
                            assumption_answer: Answer::from(oi as u8),
                            chain,
                            contradiction_qi: check_qi,
                        });
                    }
                    continue;
                }
                if check_answer(fp, hyp, check_qi) == Validity::Invalid {
                    return Some(LookaheadResult {
                        eliminate_qi: qi,
                        eliminate_oi: oi,
                        assumption_qi: qi,
                        assumption_answer: Answer::from(oi as u8),
                        chain,
                        contradiction_qi: check_qi,
                    });
                }
            }
        }
    }
    None
}

fn has_contradiction(action: &DeduceAction, hyp: &State) -> bool {
    match *action {
        DeduceAction::Force { qi, answer } => {
            (hyp.answers[qi].is_some() && hyp.answers[qi] != Some(answer))
                || (hyp.eliminated[qi] >> answer.idx()) & 1 == 1
        }
        DeduceAction::Eliminate { qi, oi } => hyp.answers[qi] == Some(Answer::from(oi as u8)),
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
                    return true;
                }
            }
            false
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
            let mut eliminated = [fp.phantom_mask(); MAX_N];
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
}
