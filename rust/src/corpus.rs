//! The shipped daily corpus, loaded from `public/puzzles/daily` (one file per
//! year). Bin-only: it reads the repo's puzzle directory, so it exists for the
//! `reference` subcommand and the corpus-wide property tests, never for the
//! wasm build.

use serde_json::Value;

use refpuzzle::serialize;
use refpuzzle::types::FlatPuzzle;

/// Every shipped daily puzzle as `(label, FlatPuzzle)`, read from the daily dir
/// (one file per year). Shared by the generator fuzz tests, the symmetry sweep,
/// and the `reference` subcommand.
pub(crate) fn daily_puzzles() -> Vec<(String, FlatPuzzle)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../public/puzzles/daily");
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.extension()?.to_str()? == "json").then_some(path)
        })
        .collect();
    files.sort();
    let mut puzzles = Vec::new();
    for path in &files {
        let filename = path.file_name().unwrap().to_str().unwrap();
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let data: Value = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("invalid JSON in {}: {e}", path.display()));
        for (day, levels) in data.as_object().unwrap() {
            let Some(levels) = levels.as_object() else {
                continue;
            };
            for (lvl, puzzle) in levels {
                let key = format!("{filename}/{day}-{lvl}");
                let fp = serialize::parse_puzzle(puzzle)
                    .unwrap_or_else(|| panic!("failed to parse daily puzzle {key}"));
                puzzles.push((key, fp));
            }
        }
    }
    puzzles
}

#[cfg(test)]
mod tests {
    use super::daily_puzzles;
    use refpuzzle::test_util::{fuzz_base_seed, slow_test_duration};
    use refpuzzle::types::*;
    use refpuzzle::{check_answer, check_form, solve_brute, solve_deduce};

    /// Every chain the hint engine reports has to hold together where the hint renders
    /// it: each entry derivable at the round it lands in, and the blamed question
    /// contradictory once the chain has been applied (`lookahead::chain_contradiction`, which is
    /// also what `minimize_chain` prunes against). Corpus puzzles, but *fuzzed* states —
    /// wrong answers and arbitrary eliminations, which the solve path never visits and
    /// where minimization has the most room to overshoot.
    #[test]
    fn lookahead_chains_replay_to_their_contradiction() {
        use refpuzzle::lookahead::lookahead_shortest;
        use refpuzzle::lookahead::test_hooks::{chain_contradiction, hypothesis, replay_chain};
        use refpuzzle::rng::Rng;

        let puzzles = daily_puzzles();
        assert!(!puzzles.is_empty());
        // After the corpus load, which on a fast run costs more than the whole budget.
        let deadline = std::time::Instant::now() + slow_test_duration();
        let base = fuzz_base_seed("REPLAY_BASE");
        let mut failures: Vec<String> = Vec::new();
        let mut states = 0u32;
        let mut hits = 0u32;
        let mut chain_steps = 0usize;

        for round in 0u32.. {
            if round % 32 == 0 && std::time::Instant::now() > deadline {
                break;
            }
            let seed = base.wrapping_add(round);
            let mut rng = Rng::new(seed);
            let (key, fp) = &puzzles[rng.int(0, puzzles.len() as i32 - 1) as usize];
            let oc = fp.option_count as i32;

            // Each question: committed to an arbitrary option, partly eliminated, or
            // untouched. Nothing ties the answers to the solution, so most states are
            // off the solve path and some are self-contradictory.
            let mut answers: [Option<Answer>; MAX_N] = [None; MAX_N];
            let mut eliminated = [fp.initial_eliminated_mask(); MAX_N];
            for qi in 0..fp.n {
                match rng.int(0, 3) {
                    0 => {
                        let answer = rng.pick_letter(fp.option_count);
                        answers[qi] = Some(answer);
                        eliminated[qi] = ALL_OPTIONS_MASK ^ (1 << answer.idx());
                    }
                    1 | 2 => {
                        for _ in 0..rng.int(1, oc - 2) {
                            eliminated[qi] |= 1 << rng.int(0, oc - 1);
                        }
                    }
                    _ => {}
                }
            }
            // A row stripped to nothing has no candidate to probe and reads as an
            // already-lost board, not a state a hint is asked about.
            if (0..fp.n).any(|qi| (!eliminated[qi] & ALL_OPTIONS_MASK) == 0) {
                continue;
            }

            let state = State {
                answers,
                eliminated,
            };
            states += 1;
            let Some(lr) = lookahead_shortest(fp, &state, usize::MAX, &mut 0) else {
                continue;
            };
            hits += 1;
            chain_steps += lr.chain.len();
            if chain_contradiction(fp, &state, &lr, &mut 0).is_none() {
                // Which half broke: a chain step no longer derivable where it lands, or
                // a replay that no longer reaches the contradiction it blames.
                let mut hyp = hypothesis(&state, lr.assumption_qi, lr.assumption_answer);
                let replayed = replay_chain(fp, &mut hyp, &lr.chain, &mut 0, |_, _, _| {});
                failures.push(format!(
                    "{key} seed={seed}: chain of {} {} Q{}",
                    lr.chain.len(),
                    if replayed {
                        "replays but leaves no contradiction at"
                    } else {
                        "has a step its round can't derive; blames"
                    },
                    lr.contradiction_qi + 1
                ));
            }
        }

        eprintln!(
            "Replay fuzz (REPLAY_BASE={base}): {hits} chain(s) over {states} state(s), \
             {:.2} steps/chain, {} failure(s)",
            if hits > 0 {
                chain_steps as f64 / hits as f64
            } else {
                0.0
            },
            failures.len()
        );
        assert!(hits > 0, "no refutable candidate in {states} state(s)");
        assert!(failures.is_empty(), "replay failures: {failures:?}");
    }

    #[test]
    fn generated_puzzles_hint_solvable() {
        let duration = slow_test_duration();
        let deadline = std::time::Instant::now() + duration;
        let puzzles = daily_puzzles();
        assert!(!puzzles.is_empty());
        let mut failures: Vec<String> = Vec::new();

        for (key, fp) in &puzzles {
            if std::time::Instant::now() > deadline {
                break;
            }
            // "Hint-solvable" = the player's auto-solve completes: the `standard`
            // (player) engine, or the unbounded `fallback` if standard stalls. `solve`
            // runs standard-first then fallback, so this asserts at least one of the
            // two lands — the real player guarantee, not just that fallback alone does.
            if !solve_deduce::solve(fp).solved {
                failures.push(key.clone());
            }
        }

        eprintln!(
            "{}/{} hint-solvable",
            puzzles.len() - failures.len(),
            puzzles.len()
        );
        if !failures.is_empty() {
            eprintln!("To inspect a failure, run:");
            for f in &failures {
                let (file, key) = f.split_once('/').unwrap();
                eprintln!("  cargo run --release -- check ./public/puzzles/daily/{file} {key}");
            }
            panic!("{} hint-solve failure(s)", failures.len());
        }
    }

    #[test]
    fn generated_puzzles_unique_solution() {
        let duration = slow_test_duration();
        let deadline = std::time::Instant::now() + duration;
        let puzzles = daily_puzzles();
        assert!(!puzzles.is_empty());
        let mut failures: Vec<String> = Vec::new();

        // Form errors are covered by `generated_puzzles_wellformed` (whole corpus,
        // no deadline); this test only checks unique-solvability and validity.
        for (key, fp) in &puzzles {
            if std::time::Instant::now() > deadline {
                break;
            }
            let solutions = solve_brute::solve(fp, 2);
            if solutions.len() != 1 {
                failures.push(format!("{key}: found {} solutions", solutions.len()));
                continue;
            }

            let sol = &solutions[0];
            let answers: [Option<Answer>; MAX_N] =
                std::array::from_fn(|i| if i < fp.n { Some(sol[i]) } else { None });

            for qi in 0..fp.n {
                if !check_answer::check_answer(
                    fp,
                    State {
                        answers,
                        eliminated: [fp.initial_eliminated_mask(); MAX_N],
                    },
                    qi,
                )
                .is_valid()
                {
                    failures.push(format!("{key}: Q{} fails validation", qi + 1));
                }
            }
        }

        eprintln!(
            "{}/{} unique",
            puzzles.len() - failures.len(),
            puzzles.len()
        );
        assert!(failures.is_empty(), "uniqueness failures: {failures:?}");
    }

    /// Howard Hinnant's civil_from_days: http://howardhinnant.github.io/date_algorithms.html
    fn today_yyyymmdd() -> u32 {
        let days = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            / 86400) as i64;
        let z = days + 719468;
        let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
        let doe = (z - era * 146097) as u64;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y0 = era * 400 + yoe as i64;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y0 + 1 } else { y0 };
        (y as u32) * 10000 + m as u32 * 100 + d as u32
    }

    /// key format: "YYYY.json/MMDD-level". Returns false (treat as not-past)
    /// when the year isn't a 4-digit number.
    fn puzzle_is_past(key: &str, today: u32) -> bool {
        let Some((year_part, rest)) = key.split_once('.') else {
            return false;
        };
        let Some(mmdd) = rest.split('/').nth(1).and_then(|s| s.split('-').next()) else {
            return false;
        };
        let (Ok(y), Ok(m)) = (year_part.parse::<u32>(), mmdd.parse::<u32>()) else {
            return false;
        };
        y * 10000 + m < today
    }

    #[test]
    fn generated_puzzles_wellformed() {
        let puzzles = daily_puzzles();
        assert!(!puzzles.is_empty());
        let today = today_yyyymmdd();
        let mut failures: Vec<String> = Vec::new();

        for (key, fp) in &puzzles {
            let is_past = puzzle_is_past(key, today);
            let errors = check_form::check_form(fp);
            for e in &errors {
                let is_warning = e.severity == check_form::Severity::Warning;
                if is_warning && is_past {
                    continue;
                }
                failures.push(format!(
                    "{key} Q{}: {:?}: {}",
                    e.qi + 1,
                    e.severity,
                    e.message
                ));
            }
        }

        if !failures.is_empty() {
            for f in &failures {
                eprintln!("FAIL: {f}");
            }
            panic!("{} wellformedness failure(s)", failures.len());
        }
    }
}
