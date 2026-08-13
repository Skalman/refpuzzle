use crate::check_answer::check_all_answers;
use crate::deduce::{
    DeduceResult, apply_action, contradiction_question, deduce, deduce_assuming_unique,
};
use crate::lookahead::{LookaheadResult, lookahead, lookahead_shortest};
use crate::recipes::{LevelRecipe, guess_recipe};
use crate::time::{us, wasm_now};
use crate::types::*;

/// Which variant of the shared solve engine [`run_engine`] runs — see each preset
/// for what it is and where it's used. They differ on three axes only: whether the
/// outer `deduce` may assume the puzzle is unique, how deep lookahead may search, and
/// which refutable candidate lookahead commits to. (Lookahead itself always deduces
/// with sound `deduce` regardless of preset — see the `lookahead` module doc — so
/// `standard` and `fallback` differ purely in the depth cap.)
#[derive(Clone, Copy)]
pub struct EngineConfig {
    /// `deduce_assuming_unique` (true) vs sound `deduce` (false).
    pub assuming_unique: bool,
    /// Handed to `lookahead`: within each hypothesis it deduces until the chain
    /// reaches this many results, then stops probing (0 disables lookahead — pure
    /// deduction). Not a hard cap: the batch that crosses the threshold is applied
    /// in full, so the chain can end slightly longer.
    pub lookahead_deduce_until: usize,
    /// `lookahead_shortest` (true) vs `lookahead` (false): probe every candidate and
    /// eliminate the one with the shortest contradiction chain, rather than the first
    /// candidate that refutes at all.
    pub pick_shortest: bool,
}

impl EngineConfig {
    /// Used for generation's pre-uniqueness work: repair's distractor-proposal solves
    /// and the stuck state repair advances from. Sound `deduce` (no uniqueness-
    /// assuming rules), lookahead bounded to the recipe depth. Assumes nothing about
    /// the number of solutions, so it may run before brute has confirmed the puzzle
    /// unique.
    pub fn generation(lookahead_deduce_until: usize) -> Self {
        Self {
            assuming_unique: false,
            lookahead_deduce_until,
            pick_shortest: false,
        }
    }
    /// Used for the ship bar and the player-facing default: generation's acceptance
    /// gate, `type_stats`' tally solve, `check`'s recipe-depth tier, and the wasm
    /// `solve` / hints. Uniqueness-assuming `deduce`, lookahead bounded to the recipe
    /// depth — the engine a player faces at the intended difficulty. Its rules assume
    /// a unique solution, so it is sound only once brute has confirmed uniqueness;
    /// every generation caller runs brute first.
    pub fn standard(lookahead_deduce_until: usize) -> Self {
        Self {
            assuming_unique: true,
            lookahead_deduce_until,
            pick_shortest: false,
        }
    }
    /// Used as the break-glass fallback: `check`'s full-depth tier and un-vetted
    /// (playground) puzzles `standard` can't finish. Uniqueness-assuming `deduce`,
    /// unbounded lookahead — searches to any depth. Because `deduce` isn't confluent,
    /// the extra depth can strand a rule and leave it stuck on a puzzle `standard`
    /// solves at recipe depth, so it is not reliably stronger — a `check` warning, not
    /// a failure.
    pub fn fallback() -> Self {
        Self {
            assuming_unique: true,
            lookahead_deduce_until: usize::MAX,
            pick_shortest: false,
        }
    }
    /// Used by `check`'s shortest-lookahead tier: `fallback` strength
    /// with the browser hint engine's picker (`lookahead_shortest`, unbounded), so the
    /// tier measures how a hint-following player's solve path differs from the
    /// first-hit one — same deduce/depth, different candidate each time lookahead
    /// fires. Probes every live candidate per call, so it is much slower than the
    /// first-hit presets; offline use only.
    pub fn shortest() -> Self {
        Self {
            assuming_unique: true,
            lookahead_deduce_until: usize::MAX,
            pick_shortest: true,
        }
    }
}

/// The two presets a level's recipe pins. Written here rather than in `recipes` so
/// that module names no engine code and stays a leaf.
impl LevelRecipe {
    /// [`EngineConfig::generation`] at this recipe's depth — the sound pre-uniqueness
    /// engine (repair proposals, the working state repair advances).
    pub fn generation_config(&self) -> EngineConfig {
        EngineConfig::generation(self.lookahead_deduce_until)
    }
    /// [`EngineConfig::standard`] at this recipe's depth — the player engine / ship bar.
    pub fn standard_config(&self) -> EngineConfig {
        EngineConfig::standard(self.lookahead_deduce_until)
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
    /// Deductions in the contradiction chains of the hits — i.e. summed over hits
    /// only, not over the probes lookahead discarded on the way there.
    pub lookahead_chain_steps: u32,
    /// The same chains counted by length rather than summed, so the tail is visible
    /// and not just the mean. Indexed by chain length; the last bucket is a
    /// "that long or longer" catch-all.
    pub lookahead_chain_hist: [u32; CHAIN_HIST_BUCKETS],
}

/// Buckets in [`EngineTelemetry::lookahead_chain_hist`]: lengths 0..=8 exactly, then
/// 9-or-more.
pub const CHAIN_HIST_BUCKETS: usize = 10;

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
    /// `Some(qi)` if some deduction contradicted an already-decided question — a rule
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
/// (`run_hint_sound` / `run_hint_standard`), the offline `check` / `solve`, and (via
/// wasm) the browser. Every behavioral difference between those callers is captured
/// by `cfg`; `max_iters` bounds the outer loop.
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
        let pick = if cfg.pick_shortest {
            lookahead_shortest
        } else {
            lookahead
        };
        let lr = pick(
            fp,
            &state,
            cfg.lookahead_deduce_until,
            &mut telemetry.deduce_calls_in_lookahead,
        );
        telemetry.lookahead_us += us(t);
        if let Some(lr) = lr {
            telemetry.lookahead_hits += 1;
            telemetry.lookahead_chain_steps += lr.chain.len() as u32;
            telemetry.lookahead_chain_hist[lr.chain.len().min(CHAIN_HIST_BUCKETS - 1)] += 1;
            sink.on_lookahead(&lr);
            state.eliminated[lr.eliminate_qi] |= 1 << lr.eliminate_oi;
            continue;
        }

        break;
    }
    EngineOutcome {
        solved: all_answered(&state) && check_all_answers(fp, &state.answers),
        state,
        telemetry,
        contradiction,
    }
}

/// The player-facing auto-solve. Tries the `standard` (player) engine at the best-
/// guess recipe depth first; only if that doesn't finish does it fall back to the
/// unbounded `fallback` engine (an un-vetted / playground puzzle beyond any recipe's
/// depth — a shipped puzzle always solves under `standard` by construction). Reports
/// only the final answers; skips step recording (`NoSteps`).
pub fn solve(fp: &FlatPuzzle) -> SolveResult {
    let recipe = guess_recipe(fp.n);
    let max_iters = fp.n * VERIFY_ITERS_PER_QUESTION;
    let out = run_engine(
        fp,
        fp.initial_state,
        recipe.standard_config(),
        max_iters,
        &mut NoSteps,
    );
    let out = if out.solved {
        out
    } else {
        run_engine(
            fp,
            fp.initial_state,
            EngineConfig::fallback(),
            max_iters,
            &mut NoSteps,
        )
    };
    SolveResult {
        solved: out.solved,
        answers: out.state.answers,
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
