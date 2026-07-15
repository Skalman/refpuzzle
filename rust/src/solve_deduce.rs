use crate::check_answer::check_answers;
use crate::deduce::{
    DeduceAction, DeduceResult, apply_action, contradiction_question, deduce,
    deduce_assuming_unique,
};
use crate::lookahead::{LookaheadResult, lookahead};
use crate::time::{us, wasm_now};
use crate::types::*;

/// Which variant of the shared solve engine to run. The presets are the engines
/// that used to be separate hand-rolled loops, now unified behind [`run_engine`]:
/// - [`generation`](EngineConfig::generation): sound `deduce` (no
///   uniqueness-assuming rules — it runs *before* uniqueness is brute-confirmed)
///   plus lookahead bounded to the recipe depth. The accept-gate.
/// - [`verify`](EngineConfig::verify): maximum power (uniqueness rules + unbounded,
///   full lookahead). Offline `check` / `solve`.
///
/// A `player` preset (`assuming_unique: true`, `lookahead_deduce_until: 1`,
/// `full: false`) mirrors the browser hint engine; add it when generation is
/// switched to certify against it.
#[derive(Clone, Copy)]
pub struct EngineConfig {
    /// `deduce_assuming_unique` (true) vs sound `deduce` (false).
    pub assuming_unique: bool,
    /// Handed to `lookahead`: within each hypothesis it deduces until the chain
    /// reaches this many results, then stops probing (0 disables lookahead — pure
    /// deduction). Not a hard cap: the batch that crosses the threshold is applied
    /// in full, so the chain can end slightly longer.
    pub lookahead_deduce_until: usize,
    /// full `deduce` (true) vs `deduce_fast` (false) inside lookahead hypotheses.
    pub lookahead_full: bool,
}

impl EngineConfig {
    pub fn generation(lookahead_deduce_until: usize) -> Self {
        Self {
            assuming_unique: false,
            lookahead_deduce_until,
            lookahead_full: false,
        }
    }
    pub fn verify() -> Self {
        Self {
            assuming_unique: true,
            lookahead_deduce_until: usize::MAX,
            lookahead_full: true,
        }
    }
}

/// Loop counters [`run_engine`] always tallies (cheap integer work). Generation
/// folds these into `Stats`; other callers discard them.
#[derive(Default)]
pub struct EngineTelemetry {
    pub deduce_calls: u32,
    pub deduce_results: u32,
    pub lookahead_calls: u32,
    pub lookahead_hits: u32,
    pub lookahead_us: u64,
    pub deduce_calls_in_lookahead: u32,
}

/// Observes each applied step. [`NoSteps`] is zero-sized and its methods inline to
/// nothing, so callers that don't report steps (generation, `solve`) compile to a
/// loop with no recording overhead. [`StepLog`] collects the ordered trace for the
/// one caller that reports it (`check`).
pub trait StepSink {
    fn on_deduce(&mut self, dr: &DeduceResult);
    fn on_lookahead(&mut self, lr: &LookaheadResult);
}

pub struct NoSteps;
impl StepSink for NoSteps {
    fn on_deduce(&mut self, _dr: &DeduceResult) {}
    fn on_lookahead(&mut self, _lr: &LookaheadResult) {}
}

#[derive(Default)]
pub struct StepLog(pub Vec<SolveStep>);
impl StepSink for StepLog {
    fn on_deduce(&mut self, dr: &DeduceResult) {
        self.0.push(SolveStep::Deduce(*dr));
    }
    fn on_lookahead(&mut self, lr: &LookaheadResult) {
        self.0.push(SolveStep::Lookahead(Box::new(lr.clone())));
    }
}

#[derive(Debug, Clone)]
pub enum SolveStep {
    Deduce(DeduceResult),
    Lookahead(Box<LookaheadResult>),
}

pub struct SolveResult {
    pub solved: bool,
    pub answers: [Option<Answer>; MAX_N],
}

/// Outcome of a [`run_engine`] call.
pub struct EngineOutcome {
    pub solved: bool,
    pub state: State,
    pub telemetry: EngineTelemetry,
    /// `Some(qi)` if some deduction contradicted an already-decided cell — a rule
    /// forcing a second answer for `qi`, or eliminating `qi`'s forced answer. A
    /// sound engine on a well-posed puzzle never does this; it's surfaced so every
    /// caller is guarded against an unsound rule (generation asserts it's `None`,
    /// `check` reports the first incorrect action).
    pub contradiction: Option<usize>,
}

/// Outer-loop iteration cap for the offline verify engine, as `n * this`. Each pass
/// applies at least one deduction or one lookahead elimination, and a puzzle settles
/// in far fewer passes per question; the factor is generous slack that still bounds a
/// non-converging engine.
pub const VERIFY_ITERS_PER_QUESTION: usize = 30;

/// The single deduce→lookahead solve loop shared by generation
/// (`run_hint_engine`), the offline `check` / `solve`, and (via wasm) the browser.
/// Every behavioral difference between those callers is captured by `cfg`;
/// `max_iters` bounds the outer loop.
pub fn run_engine<S: StepSink>(
    fp: &FlatPuzzle,
    mut state: State,
    cfg: EngineConfig,
    max_iters: usize,
    sink: &mut S,
) -> EngineOutcome {
    let n = fp.n;
    let all_answered = |st: &State| (0..n).all(|i| st.answers[i].is_some());
    let mut telemetry = EngineTelemetry::default();
    let mut contradiction = None;

    for _ in 0..max_iters {
        if all_answered(&state) {
            break;
        }

        telemetry.deduce_calls += 1;
        let drs = if cfg.assuming_unique {
            deduce_assuming_unique(fp, &state)
        } else {
            deduce(fp, &state)
        };
        telemetry.deduce_results += drs.len() as u32;
        if !drs.is_empty() {
            for dr in &drs {
                // First self-contradiction wins; keep solving so `check` still gets
                // the full trajectory and generation asserts after the fact.
                if contradiction.is_none() {
                    contradiction = contradiction_question(&dr.action, &state);
                }
                sink.on_deduce(dr);
                apply_action(&dr.action, &mut state);
            }
            continue;
        }

        // Budget 0 disables lookahead (intro puzzles must be pure-deduction).
        if cfg.lookahead_deduce_until == 0 {
            break;
        }
        telemetry.lookahead_calls += 1;
        let t = wasm_now();
        let lr = lookahead(
            fp,
            &state,
            cfg.lookahead_deduce_until,
            cfg.lookahead_full,
            &mut telemetry.deduce_calls_in_lookahead,
        );
        telemetry.lookahead_us += us(t);
        if let Some(lr) = lr {
            telemetry.lookahead_hits += 1;
            sink.on_lookahead(&lr);
            state.eliminated[lr.eliminate_qi] |= 1 << lr.eliminate_oi;
            continue;
        }

        break;
    }
    EngineOutcome {
        solved: all_answered(&state) && check_answers(fp, &state.answers),
        state,
        telemetry,
        contradiction,
    }
}

/// Solve with the offline `verify` engine (uniqueness rules + full, unbounded
/// lookahead), reporting only the final answers.
pub fn solve(fp: &FlatPuzzle) -> SolveResult {
    // `solve` only reports the final answers; skip step recording (`NoSteps`) so
    // there's no throwaway `Vec` on the wasm solve path.
    let out = run_engine(
        fp,
        fp.initial_state,
        EngineConfig::verify(),
        fp.n * VERIFY_ITERS_PER_QUESTION,
        &mut NoSteps,
    );
    SolveResult {
        solved: out.solved,
        answers: out.state.answers,
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
        SolveStep::Lookahead(lr) => {
            vec![format!(
                "{}{}",
                lr.eliminate_qi + 1,
                letters_lower[lr.eliminate_oi]
            )]
        }
    }
}

pub fn format_steps(steps: &[SolveStep]) -> Vec<String> {
    steps.iter().flat_map(format_step).collect()
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
