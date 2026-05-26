use crate::deduce::{DeduceAction, DeduceResult, deduce};
use crate::lookahead::lookahead;
use crate::types::*;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SolveStep {
    Deduce(DeduceResult),
    Lookahead {
        eliminate_qi: usize,
        eliminate_oi: usize,
        assumption_qi: usize,
        assumption_answer: Answer,
    },
}

pub struct SolveResult {
    pub solved: bool,
    pub answers: [Option<Answer>; MAX_N],
    pub steps: Vec<SolveStep>,
}

pub fn solve(fp: &FlatPuzzle) -> SolveResult {
    let n = fp.n;
    let mut state = fp.initial_state;
    let mut steps = Vec::new();

    for _ in 0..n * 30 {
        if (0..n).all(|i| state.answers[i].is_some()) {
            let valid = crate::check_answer::check_answers(fp, &state.answers);
            return SolveResult {
                solved: valid,
                answers: state.answers,
                steps,
            };
        }

        let drs = deduce(fp, &state);
        if !drs.is_empty() {
            for dr in &drs {
                apply_action(&dr.action, &mut state);
                steps.push(SolveStep::Deduce(*dr));
            }
            continue;
        }

        if let Some(lr) = lookahead(fp, &state, usize::MAX, false) {
            state.eliminated[lr.eliminate_qi] |= 1 << lr.eliminate_oi;
            steps.push(SolveStep::Lookahead {
                eliminate_qi: lr.eliminate_qi,
                eliminate_oi: lr.eliminate_oi,
                assumption_qi: lr.assumption_qi,
                assumption_answer: lr.assumption_answer,
            });
            continue;
        }

        break;
    }

    let solved = (0..n).all(|i| state.answers[i].is_some());
    SolveResult {
        solved,
        answers: state.answers,
        steps,
    }
}

pub fn format_step(step: &SolveStep) -> Vec<String> {
    let letters_lower = ['a', 'b', 'c', 'd', 'e'];
    match step {
        SolveStep::Deduce(dr) => match dr.action {
            DeduceAction::Force { qi, answer } => vec![format!("{}{}", qi + 1, answer.as_char())],
            DeduceAction::Eliminate { qi, oi } => {
                vec![format!("{}{}", qi + 1, letters_lower[oi])]
            }
            DeduceAction::EliminateMulti {
                question_mask,
                option_mask,
            } => {
                let mut out = Vec::new();
                for i in 0..MAX_N {
                    if (question_mask >> i) & 1 == 1 {
                        for oi in 0..5usize {
                            if (option_mask >> oi) & 1 == 1 {
                                out.push(format!("{}{}", i + 1, letters_lower[oi]));
                            }
                        }
                    }
                }
                out
            }
        },
        SolveStep::Lookahead {
            eliminate_qi,
            eliminate_oi,
            ..
        } => vec![format!(
            "{}{}",
            eliminate_qi + 1,
            letters_lower[*eliminate_oi]
        )],
    }
}

pub fn format_steps(steps: &[SolveStep]) -> Vec<String> {
    steps.iter().flat_map(format_step).collect()
}

fn apply_action(action: &DeduceAction, state: &mut State) {
    match *action {
        DeduceAction::Force { qi, answer } => {
            state.eliminated[qi] = 0b11111 ^ (1 << answer.idx());
            state.answers[qi] = Some(answer);
        }
        DeduceAction::Eliminate { qi, oi } => {
            state.eliminated[qi] |= 1 << oi;
        }
        DeduceAction::EliminateMulti {
            question_mask,
            option_mask,
        } => {
            for i in 0..MAX_N {
                if (question_mask >> i) & 1 == 1 {
                    state.eliminated[i] |= option_mask;
                }
            }
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

            let fp = crate::serialize::parse_puzzle(&test["puzzle"]);
            let fp = match fp {
                Some(fp) => fp,
                None => {
                    eprintln!("SKIP: {name}: parse failed");
                    continue;
                }
            };

            let result = solve(&fp);
            let got = if result.solved { "solved" } else { "stuck" };

            if got != expect {
                failed += 1;
                eprintln!("FAIL: {name}");
                eprintln!("  expected: {expect}");
                eprintln!("  got:      {got}");
                continue;
            }

            if let Some(expected_sol) = test.get("solution").and_then(|s| s.as_str()) {
                let got_sol: String = result
                    .answers
                    .iter()
                    .take(fp.n)
                    .map(|a| match a {
                        Some(a) => a.as_char(),
                        None => '?',
                    })
                    .collect();
                if got_sol != expected_sol {
                    failed += 1;
                    eprintln!("FAIL: {name}");
                    eprintln!("  expected solution: {expected_sol}");
                    eprintln!("  got solution:      {got_sol}");
                    continue;
                }
            }

            passed += 1;
        }

        eprintln!("{passed}/{} passed", passed + failed);
        assert_eq!(failed, 0, "{failed} test(s) failed");
    }
}
