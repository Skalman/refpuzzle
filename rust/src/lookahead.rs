use arrayvec::ArrayVec;

use crate::check_validity::{Validity, check_answer_validity};
use crate::deduce::{DeduceAction, DeduceResult, deduce};
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
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
) -> Option<LookaheadResult> {
    let n = fp.n;
    for qi in 0..n {
        if answers[qi].is_some() {
            continue;
        }
        for oi in 0..5usize {
            if (eliminated[qi] >> oi) & 1 == 1 {
                continue;
            }

            let mut hyp_answers = *answers;
            let mut hyp_eliminated = *eliminated;
            hyp_answers[qi] = Some(LETTERS[oi]);
            hyp_eliminated[qi] = 0b11111 ^ (1 << oi);

            let mut chain = ArrayVec::new();

            for _ in 0..n * 5 {
                match deduce(fp, &hyp_answers, &hyp_eliminated) {
                    Some(dr) => {
                        apply_action(&dr.action, &mut hyp_answers, &mut hyp_eliminated);
                        if !chain.is_full() {
                            chain.push(dr);
                        }
                    }
                    None => break,
                }
            }

            for check_qi in 0..n {
                if hyp_answers[check_qi].is_none() {
                    if (!hyp_eliminated[check_qi] & 0b11111u8).count_ones() == 0 {
                        return Some(LookaheadResult {
                            eliminate_qi: qi,
                            eliminate_oi: oi,
                            assumption_qi: qi,
                            assumption_answer: LETTERS[oi],
                            chain,
                            contradiction_qi: check_qi,
                        });
                    }
                    continue;
                }
                if check_answer_validity(fp, &hyp_answers, &hyp_eliminated, check_qi)
                    == Validity::Invalid
                {
                    return Some(LookaheadResult {
                        eliminate_qi: qi,
                        eliminate_oi: oi,
                        assumption_qi: qi,
                        assumption_answer: LETTERS[oi],
                        chain,
                        contradiction_qi: check_qi,
                    });
                }
            }
        }
    }
    None
}

fn apply_action(
    action: &DeduceAction,
    answers: &mut [Option<Answer>; MAX_N],
    eliminated: &mut [u8; MAX_N],
) {
    match *action {
        DeduceAction::Force { qi, answer } => {
            eliminated[qi] = 0b11111 ^ (1 << answer.idx());
            answers[qi] = Some(answer);
        }
        DeduceAction::Eliminate { qi, oi } => {
            eliminated[qi] |= 1 << oi;
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

            let fp = crate::parse_puzzle(&test["puzzle"]);
            let fp = match fp {
                Some(fp) => fp,
                None => {
                    eprintln!("SKIP: {name}: parse failed");
                    continue;
                }
            };

            let n = fp.n;
            let mut answers: [Option<Answer>; MAX_N] = [None; MAX_N];
            let mut eliminated = [0u8; MAX_N];
            for i in 0..n {
                let s = states[i].as_str().unwrap_or("");
                for ch in s.chars() {
                    if ch.is_ascii_uppercase() {
                        let oi = (ch as u8 - b'A') as usize;
                        answers[i] = Some(LETTERS[oi]);
                        eliminated[i] = 0b11111 ^ (1 << oi);
                    } else if ch.is_ascii_lowercase() {
                        let oi = (ch as u8 - b'a') as usize;
                        eliminated[i] |= 1 << oi;
                    }
                }
            }

            let result = lookahead(&fp, &answers, &eliminated);
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
