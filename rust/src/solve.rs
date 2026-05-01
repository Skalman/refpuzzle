use crate::check_validity::{Validity, check_answer_validity};
use crate::deduce::{DeduceAction, deduce};
use crate::lookahead::lookahead;
use crate::types::*;

#[derive(Debug)]
pub enum SolveOutcome {
    Solved,
    Stuck,
}

pub fn check_solvable(fp: &FlatPuzzle) -> SolveOutcome {
    let n = fp.n;
    let mut answers = [None; MAX_N];
    let mut eliminated = [0u8; MAX_N];

    for _ in 0..n * 30 {
        if (0..n).all(|i| answers[i].is_some()) {
            return SolveOutcome::Solved;
        }

        if let Some(dr) = deduce(fp, &answers, &eliminated) {
            apply_action(&dr.action, &mut answers, &mut eliminated);
            continue;
        }

        if let Some(lr) = lookahead(fp, &answers, &eliminated) {
            eliminated[lr.eliminate_qi] |= 1 << lr.eliminate_oi;
            continue;
        }

        break;
    }

    if (0..n).all(|i| answers[i].is_some()) {
        SolveOutcome::Solved
    } else {
        SolveOutcome::Stuck
    }
}

pub fn check_puzzle_solved(
    fp: &FlatPuzzle,
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
) -> bool {
    let n = fp.n;
    (0..n).all(|i| {
        answers[i].is_some() && check_answer_validity(fp, answers, eliminated, i) == Validity::Valid
    })
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
    fn test_shared_solve() {
        let json_str =
            std::fs::read_to_string("../tests/solve.json").expect("can't read tests/solve.json");
        let suite: Value = serde_json::from_str(&json_str).unwrap();
        let tests = suite["tests"].as_array().unwrap();

        let mut passed = 0;
        let mut failed = 0;

        for test in tests {
            if test.get("section").is_some() {
                continue;
            }
            let name = test["name"].as_str().unwrap();
            let expect = test["expect"].as_str().unwrap();

            let fp = crate::parse_puzzle(&test["puzzle"]);
            let fp = match fp {
                Some(fp) => fp,
                None => {
                    eprintln!("SKIP: {name}: parse failed");
                    continue;
                }
            };

            let outcome = check_solvable(&fp);
            let got = match outcome {
                SolveOutcome::Solved => "solved",
                SolveOutcome::Stuck => "stuck",
            };

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
