//! CLI harness for `refpuzzle check`: runs a puzzle (or a whole year file)
//! through solve / deduce / form / well-posed and prints a verdict. Not on the
//! play or generate path.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::check_form;
use crate::check_well_posed;
use crate::construct;
use crate::deduce;
use crate::format;
use crate::serialize;
use crate::solve_brute;
use crate::solve_deduce;
use crate::types::*;

// ── Color ──

static USE_COLOR: AtomicBool = AtomicBool::new(false);

fn init_color(enabled: bool) {
    USE_COLOR.store(enabled, Ordering::Relaxed);
}

fn c(code: &str, s: &str) -> String {
    if USE_COLOR.load(Ordering::Relaxed) {
        format!("{code}{s}\x1b[0m")
    } else {
        s.to_string()
    }
}
fn green(s: &str) -> String {
    c("\x1b[32m", s)
}
fn red(s: &str) -> String {
    c("\x1b[31m", s)
}
fn yellow(s: &str) -> String {
    c("\x1b[33m", s)
}
fn dim(s: &str) -> String {
    c("\x1b[2m", s)
}
fn bold(s: &str) -> String {
    c("\x1b[1m", s)
}

fn ok_n(n: usize) -> String {
    if n > 0 {
        green(&format!("{n} ok"))
    } else {
        dim("0 ok")
    }
}
fn bad_n(n: usize, label: &str) -> String {
    if n > 0 {
        red(&format!("{n} {label}"))
    } else {
        dim(&format!("0 {label}"))
    }
}
fn warn_n(n: usize, label: &str) -> String {
    if n > 0 {
        yellow(&format!("{n} {label}"))
    } else {
        dim(&format!("0 {label}"))
    }
}

// ── JSON contract ──

#[derive(Serialize, Deserialize)]
pub struct CheckOutput {
    pub path: String,
    pub year: String,
    pub target: Option<String>,
    /// Input was a single bare puzzle blob (no date/level context) rather than a
    /// year map — render the detailed single view and suppress date-based URLs.
    #[serde(default)]
    pub single: bool,
    pub puzzles: Vec<PuzzleCheckResult>,
}

#[derive(Serialize, Deserialize)]
pub struct PuzzleCheckResult {
    pub key: String,
    pub n: usize,
    pub option_count: usize,
    pub questions: Vec<QuestionInfo>,
    pub form_warnings: Vec<String>,
    pub form_errors: Vec<String>,
    pub solve_ok: bool,
    pub solve_answered: usize,
    pub solve_steps: Vec<String>,
    /// Shareable link opened on the solver's resolved cells. A date route for the
    /// served corpus; a self-contained `/playground` link (blob embedded) otherwise.
    pub solve_link: String,
    /// Solve from the start under the puzzle's own generation accept-gate engine
    /// (`generation(recipe_depth)`). `None` for keyless (playground) puzzles.
    /// `Some(false)` means it no longer solves under that engine — a corpus-drift /
    /// stale-bake signal, since generation only admits puzzles this engine solves.
    pub recipe_depth: Option<usize>,
    pub recipe_solve_ok: Option<bool>,
    pub recipe_solve_answered: Option<usize>,
    pub brute_count: usize,
    pub brute_solutions: Vec<String>,
    /// One link per `brute_solutions` entry, same shape as `solve_link`.
    pub brute_links: Vec<String>,
    pub hint_brute_match: bool,
    /// Questions without a unique answer for the key — `check_well_posed_given_key`
    /// (histogram/structural) and `check_well_posed_given_options` (SameAs/SameAsWhich/TrueStmt).
    pub ambiguous: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct QuestionInfo {
    pub type_tag: String,
    pub options: Vec<Option<i64>>,
    pub claims: Option<Vec<ClaimInfo>>,
}

#[derive(Serialize, Deserialize)]
pub struct ClaimInfo {
    pub label: String,
    pub text: String,
}

// ── Helpers ──

fn extract_year(path: &str) -> String {
    let s = path.strip_suffix(".json").unwrap_or(path);
    s[s.len().saturating_sub(4)..].to_string()
}

/// Dev server the generated links point at (date routes and `/playground` alike).
const ORIGIN: &str = "http://localhost:5173";

fn make_url(year: &str, day: &str, lvl: &str, steps: &[String]) -> String {
    let mm = day.get(..2).expect("day key must be MMDD");
    let dd = day.get(2..4).expect("day key must be MMDD");
    let hash = steps.join(".");
    format!("{ORIGIN}/{year}-{mm}-{dd}/{lvl}?debug#{hash}")
}

fn solution_str_to_steps(sol: &str) -> Vec<String> {
    sol.chars()
        .enumerate()
        .map(|(i, ch)| format!("{}{}", i + 1, ch))
        .collect()
}

// ── Compute ──

/// Puzzle layout for the `check` display; depends only on `fp`, so it's safe even
/// when the solve phases are skipped.
fn build_question_infos(fp: &FlatPuzzle) -> Vec<QuestionInfo> {
    let n = fp.n;
    let oc = fp.option_count;
    (0..n)
        .map(|qi| {
            let type_tag = format::format_type_tag(&fp.question_types[qi]);
            let options: Vec<Option<i64>> = (0..oc)
                .map(|oi| {
                    let ov = fp.options[qi][oi];
                    if matches!(fp.question_types[qi], QuestionType::TrueStmt) || !ov.is_num() {
                        None
                    } else {
                        Some(ov.value() as i64)
                    }
                })
                .collect();
            let claims = if matches!(fp.question_types[qi], QuestionType::TrueStmt) {
                Some(
                    (0..oc)
                        .map(|oi| {
                            let label = ['A', 'B', 'C', 'D', 'E'][oi].to_string();
                            let text = match fp.claim_at(qi, oi) {
                                Some(c) => {
                                    let tag = format::format_type_tag(&c.question_type);
                                    let v = if c.value.is_none() {
                                        "null".into()
                                    } else {
                                        c.value.value().to_string()
                                    };
                                    format!("{tag} = {v}")
                                }
                                None => "none".into(),
                            };
                            ClaimInfo { label, text }
                        })
                        .collect(),
                )
            } else {
                None
            };
            QuestionInfo {
                type_tag,
                options,
                claims,
            }
        })
        .collect()
}

/// Re-solve from the start under the puzzle's own generation recipe depth — the
/// exact accept-gate engine (`EngineConfig::generation(depth)`, `n*15` iters). The
/// level is parsed from the `MMDD-L` key; `None` for keyless (playground) puzzles.
/// Returns `(depth, solved, answered)`.
fn recipe_depth_solve(fp: &FlatPuzzle, key: &str) -> Option<(usize, bool, usize)> {
    let level: usize = key.rsplit_once('-')?.1.parse().ok()?;
    if !(1..=6).contains(&level) {
        return None;
    }
    let depth = construct::RECIPES[level - 1].lookahead_deduce_until;
    let out = solve_deduce::run_engine(
        fp,
        fp.initial_state,
        solve_deduce::EngineConfig::generation(depth),
        fp.n * 15,
        &mut solve_deduce::NoSteps,
    );
    let answered = out.state.answers[..fp.n]
        .iter()
        .filter(|a| a.is_some())
        .count();
    Some((depth, out.solved, answered))
}

/// `year = Some(y)` (always non-empty) renders date-route links (served corpus,
/// key is MMDD-L); `year = None` renders self-contained `/playground` links with
/// the puzzle blob embedded (single puzzles, stdin, files outside the corpus).
fn check_one_puzzle(fp: &FlatPuzzle, key: &str, year: Option<&str>) -> PuzzleCheckResult {
    let n = fp.n;
    let oc = fp.option_count;

    let fe = check_form::check_form(fp);
    let form_warnings: Vec<String> = fe
        .iter()
        .filter(|e| matches!(e.severity, check_form::Severity::Warning))
        .map(|e| format!("Q{}: {}", e.qi + 1, e.message))
        .collect();
    let form_errors: Vec<String> = fe
        .iter()
        .filter(|e| matches!(e.severity, check_form::Severity::Error))
        .map(|e| format!("Q{}: {}", e.qi + 1, e.message))
        .collect();

    let questions = build_question_infos(fp);

    // Malformed input can panic the solve phases; on fatal form errors, skip them.
    // The sentinels below read as "not evaluated" so the verdict is FORM ERRORS.
    if !form_errors.is_empty() {
        return PuzzleCheckResult {
            key: key.to_string(),
            n,
            option_count: oc,
            questions,
            form_warnings,
            form_errors,
            solve_ok: true,
            solve_answered: 0,
            solve_steps: Vec::new(),
            solve_link: String::new(),
            recipe_depth: None,
            recipe_solve_ok: None,
            recipe_solve_answered: None,
            brute_count: 1,
            brute_solutions: Vec::new(),
            brute_links: Vec::new(),
            hint_brute_match: true,
            ambiguous: Vec::new(),
        };
    }

    // Link renderer: a date route the dev server resolves for the served corpus, or
    // a self-contained `/playground` link (blob + resolved cells) for everything
    // else. `steps` feeds the date hash; `state` feeds the playground history.
    let make_link = |state: &State, steps: &[String]| -> String {
        match year {
            Some(year) => {
                let (day, lvl) = key.split_once('-').expect("dated key must be MMDD-L");
                make_url(year, day, lvl, steps)
            }
            None => serialize::playground_link(ORIGIN, fp, state),
        }
    };

    let cr = run_check(fp, key);
    let answered = cr.answers[..n].iter().filter(|a| a.is_some()).count();
    let recipe = recipe_depth_solve(fp, key);

    let solutions = solve_brute::solve(fp, 10);

    let unique_solution = if solutions.len() == 1 {
        Some(solutions[0][..n].as_ref())
    } else {
        None
    };
    let brute_count = solutions.len();
    let brute_solutions: Vec<String> = solutions
        .iter()
        .map(|sol| sol.iter().take(n).map(|a| a.as_char()).collect())
        .collect();
    let brute_links: Vec<String> = solutions
        .iter()
        .zip(&brute_solutions)
        .map(|(sol, sol_str)| {
            let state = State {
                answers: std::array::from_fn(|i| (i < n).then(|| sol[i])),
                eliminated: [0; MAX_N],
            };
            make_link(&state, &solution_str_to_steps(sol_str))
        })
        .collect();

    let hint_brute_match = if cr.ok && solutions.len() == 1 {
        (0..n).all(|i| cr.answers[i] == Some(solutions[0][i]))
    } else {
        true
    };

    let state = State {
        answers: cr.answers,
        eliminated: cr.eliminated,
    };
    let solve_link = make_link(&state, &cr.steps);

    // Well-posedness: with the key known (unique brute solution), no question may
    // have a second valid answer.
    let ambiguous: Vec<String> = unique_solution
        .map(|sol| {
            (0..n)
                .filter_map(|qi| {
                    check_well_posed::check_well_posed_given_key(
                        n,
                        oc,
                        sol,
                        qi,
                        fp.question_types[qi],
                    )
                    .or_else(|| check_well_posed::check_well_posed_given_options(fp, sol, qi))
                    .map(|msg| format!("Q{}: {}", qi + 1, msg))
                })
                .collect()
        })
        .unwrap_or_default();

    PuzzleCheckResult {
        key: key.to_string(),
        n,
        option_count: oc,
        questions,
        form_warnings,
        form_errors,
        solve_ok: cr.ok,
        solve_answered: answered,
        solve_steps: cr.steps,
        solve_link,
        recipe_depth: recipe.map(|(d, _, _)| d),
        recipe_solve_ok: recipe.map(|(_, ok, _)| ok),
        recipe_solve_answered: recipe.map(|(_, _, a)| a),
        brute_count,
        brute_solutions,
        brute_links,
        hint_brute_match,
        ambiguous,
    }
}

/// Read the check input, treating `-` as stdin (mirrors `-o -` on `gen`).
fn read_input(path: &str) -> String {
    if path == "-" {
        std::io::read_to_string(std::io::stdin()).expect("failed to read stdin")
    } else {
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("can't read {path}: {e}"))
    }
}

fn compute_check_output(path: &str, target: Option<&str>) -> CheckOutput {
    let data: Value = serde_json::from_str(&read_input(path)).expect("invalid JSON");

    // A bare single-puzzle blob carries "o"/"q" at the top level; a year map's
    // top-level keys are MMDD dates. Check the blob directly — it has no date/level
    // context, so it gets a synthetic key and `/playground` links (year = None).
    if data.get("o").is_some() && data.get("q").is_some() {
        let fp = serialize::parse_puzzle(&data).unwrap_or_else(|| {
            eprintln!("Error: input is not a valid puzzle");
            std::process::exit(1);
        });
        return CheckOutput {
            path: path.to_string(),
            year: String::new(),
            target: None,
            single: true,
            puzzles: vec![check_one_puzzle(&fp, "puzzle", None)],
        };
    }

    let obj = data.as_object().expect("top-level must be object");
    let year = extract_year(path);
    // Date routes only resolve for the served corpus; puzzles read from anywhere
    // else (including stdin) get self-contained `/playground` links instead. An
    // empty year (unnamed path) folds to None so `make_link` sees one signal.
    let link_year = path
        .contains("puzzles/daily")
        .then_some(year.as_str())
        .filter(|y| !y.is_empty());

    let mut puzzles = Vec::new();

    for (day, levels) in obj {
        let levels = levels
            .as_object()
            .unwrap_or_else(|| panic!("day {day:?}: levels must be an object"));
        for (lvl, puzzle) in levels {
            let key = format!("{day}-{lvl}");
            if let Some(t) = target
                && key != t
            {
                continue;
            }
            let fp = match serialize::parse_puzzle(puzzle) {
                Some(fp) => fp,
                None => {
                    eprintln!("  SKIP {key}: parse failed");
                    continue;
                }
            };
            puzzles.push(check_one_puzzle(&fp, &key, link_year));
        }
    }

    CheckOutput {
        path: path.to_string(),
        year,
        target: target.map(|s| s.to_string()),
        single: false,
        puzzles,
    }
}

// ── Format ──

fn format_single(w: &mut impl Write, r: &PuzzleCheckResult) -> bool {
    let n = r.n;

    let has_form_warns = !r.form_warnings.is_empty();
    let has_errors = !r.solve_ok
        || r.brute_count != 1
        || !r.hint_brute_match
        || !r.ambiguous.is_empty()
        || !r.form_errors.is_empty();

    let verdict = if !r.hint_brute_match {
        red("MISMATCH")
    } else if !r.solve_ok && r.solve_answered == n {
        red("CONTRADICTION")
    } else if !r.solve_ok {
        red("STUCK")
    } else if r.brute_count == 0 {
        red("UNSOLVABLE")
    } else if r.brute_count != 1 || !r.ambiguous.is_empty() {
        red("AMBIGUOUS")
    } else if !r.form_errors.is_empty() {
        red("FORM ERRORS")
    } else if has_form_warns {
        yellow("ok (with warnings)")
    } else {
        green("ok")
    };

    writeln!(w, "\n{}: {verdict}", bold(&r.key)).unwrap();
    writeln!(w).unwrap();

    // Puzzle layout
    let max_tag_len = r
        .questions
        .iter()
        .map(|q| q.type_tag.len())
        .max()
        .unwrap_or(0);
    // `.first()` not `[0]`: a form-error sentinel has brute_count == 1 but no solution.
    let sol_chars: Vec<char> = if r.brute_count == 1 {
        r.brute_solutions
            .first()
            .map(|s| s.chars().collect())
            .unwrap_or_default()
    } else {
        vec![]
    };

    for (qi, q) in r.questions.iter().enumerate() {
        let answer_oi = sol_chars.get(qi).map(|&ch| (ch as u8 - b'A') as usize);
        let vals: Vec<String> = q
            .options
            .iter()
            .enumerate()
            .map(|(oi, v)| {
                let s = match v {
                    Some(n) => n.to_string(),
                    None => "none".into(),
                };
                if Some(oi) == answer_oi {
                    format!(" {} ", bold(&green(&s)))
                } else {
                    s
                }
            })
            .collect();
        writeln!(
            w,
            "  {} {:<width$} [{}]",
            dim(&format!("Q{:<2}", qi + 1)),
            q.type_tag,
            vals.join(","),
            width = max_tag_len
        )
        .unwrap();
        if let Some(claims) = &q.claims {
            for cl in claims {
                writeln!(w, "      {}  {}", dim(&cl.label), cl.text).unwrap();
            }
        }
    }

    writeln!(w).unwrap();

    // Form
    let form_label = if r.form_warnings.is_empty() && r.form_errors.is_empty() {
        green("ok")
    } else {
        format!(
            "{}, {}",
            warn_n(r.form_warnings.len(), "warnings"),
            bad_n(r.form_errors.len(), "errors")
        )
    };
    writeln!(w, "  {:<28} {form_label}", "Form").unwrap();
    for msg in r.form_warnings.iter().chain(r.form_errors.iter()) {
        writeln!(w, "    {}", dim(msg)).unwrap();
    }

    // Form-error result: solve was skipped, so the sections below are sentinels.
    if !r.form_errors.is_empty() {
        return has_errors;
    }

    // Answerable (unique answer per question)
    let answerable_label = if r.ambiguous.is_empty() {
        green("ok")
    } else {
        bad_n(r.ambiguous.len(), "ambiguous")
    };
    writeln!(w, "  {:<28} {answerable_label}", "Answerable").unwrap();
    for msg in &r.ambiguous {
        writeln!(w, "    {}", dim(msg)).unwrap();
    }

    // Solve
    let solve_label = if r.solve_ok {
        green(&format!("solved {}/{n}", r.solve_answered))
    } else {
        red(&format!("stuck {}/{n}", r.solve_answered))
    };
    writeln!(w, "  {:<28} {solve_label}", "Deduce+lookahead (full)").unwrap();
    writeln!(w, "    {}", dim(&r.solve_link)).unwrap();

    // Same engine at this level's accept-gate (recipe) depth — a drift signal if it
    // no longer solves.
    if let Some(ok) = r.recipe_solve_ok {
        let d = r.recipe_depth.unwrap_or(0);
        let ans = r.recipe_solve_answered.unwrap_or(0);
        let label = if ok {
            green(&format!("solved {ans}/{n} (d={d})"))
        } else {
            yellow(&format!("stuck {ans}/{n} (d={d})"))
        };
        writeln!(w, "  {:<28} {label}", "Deduce+lookahead (recipe)").unwrap();
    }

    // Brute
    if r.brute_count == 1 {
        writeln!(
            w,
            "  {:<28} {}",
            "Brute",
            green(&format!("1 solution ({})", r.brute_solutions[0]))
        )
        .unwrap();
        writeln!(w, "    {}", dim(&r.brute_links[0])).unwrap();
    } else {
        writeln!(
            w,
            "  {:<28} {}",
            "Brute",
            red(&format!("{} solutions", r.brute_count))
        )
        .unwrap();
        for (i, sol) in r.brute_solutions.iter().enumerate() {
            writeln!(w, "    #{} {sol}", i + 1).unwrap();
            writeln!(w, "      {}", dim(&r.brute_links[i])).unwrap();
        }
    }

    // Match
    let match_label = if r.hint_brute_match {
        green("ok")
    } else {
        red("MISMATCH")
    };
    writeln!(w, "  {:<28} {match_label}", "Deduce+lookahead vs brute").unwrap();

    has_errors
}

fn format_full(w: &mut impl Write, results: &[PuzzleCheckResult], path: &str) -> bool {
    let total = results.len();
    let basename = std::path::Path::new(path)
        .file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap_or(path);

    let mut form_warnings: Vec<&str> = Vec::new();
    let mut form_errors: Vec<&str> = Vec::new();
    let mut stuck: Vec<&str> = Vec::new();
    let mut contradictions: Vec<&str> = Vec::new();
    let mut ambiguous: Vec<&str> = Vec::new();
    let mut mismatches: Vec<&str> = Vec::new();
    let mut not_answerable: Vec<&str> = Vec::new();
    let mut recipe_stuck: Vec<&str> = Vec::new();

    for r in results {
        if !r.form_warnings.is_empty() {
            form_warnings.push(&r.key);
        }
        if !r.form_errors.is_empty() {
            form_errors.push(&r.key);
        }
        if !r.solve_ok && r.solve_answered == r.n {
            contradictions.push(&r.key);
        } else if !r.solve_ok {
            stuck.push(&r.key);
        }
        if r.brute_count != 1 {
            ambiguous.push(&r.key);
        }
        if !r.hint_brute_match {
            mismatches.push(&r.key);
        }
        if !r.ambiguous.is_empty() {
            not_answerable.push(&r.key);
        }
        if r.recipe_solve_ok == Some(false) {
            recipe_stuck.push(&r.key);
        }
    }

    let mut failed_set: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for v in [
        &stuck,
        &contradictions,
        &ambiguous,
        &mismatches,
        &not_answerable,
        &form_errors,
    ] {
        for k in v {
            failed_set.insert(k);
        }
    }
    let passed = total - failed_set.len();
    let has_warnings = !form_warnings.is_empty();
    let has_errors = !form_errors.is_empty()
        || !stuck.is_empty()
        || !contradictions.is_empty()
        || !ambiguous.is_empty()
        || !mismatches.is_empty()
        || !not_answerable.is_empty();

    let verdict = if has_errors {
        red(&format!("{passed}/{total} passed"))
    } else if has_warnings {
        yellow(&format!("{passed}/{total} passed (with warnings)"))
    } else {
        green(&format!("{passed}/{total} passed"))
    };
    writeln!(w, "\n{basename}: {verdict}").unwrap();
    let form_problem_count: usize = {
        let mut all: Vec<&str> = form_warnings
            .iter()
            .chain(form_errors.iter())
            .copied()
            .collect();
        all.sort();
        all.dedup();
        all.len()
    };
    let n_form_ok = total - form_problem_count;
    writeln!(
        w,
        "  Form                  {}, {}, {}",
        ok_n(n_form_ok),
        warn_n(form_warnings.len(), "warnings"),
        bad_n(form_errors.len(), "errors")
    )
    .unwrap();
    let n_solve_ok = total - stuck.len() - contradictions.len();
    writeln!(w, "  {}", bold("Solve methods")).unwrap();
    writeln!(
        w,
        "    {:<28}{}, {}, {}",
        "deduce+lookahead (full)",
        ok_n(n_solve_ok),
        bad_n(stuck.len(), "stuck"),
        bad_n(contradictions.len(), "contradiction")
    )
    .unwrap();
    // Same deduce+lookahead engine at each level's accept-gate (recipe) depth —
    // informational (the full run + brute are the pass/fail gates). "stuck" flags
    // corpus drift: admitted puzzles that no longer solve at their recipe depth.
    let n_recipe_graded = results
        .iter()
        .filter(|r| r.recipe_solve_ok.is_some())
        .count();
    writeln!(
        w,
        "    {:<28}{}, {}",
        "deduce+lookahead (recipe)",
        ok_n(n_recipe_graded - recipe_stuck.len()),
        warn_n(recipe_stuck.len(), "stuck")
    )
    .unwrap();
    writeln!(
        w,
        "    {:<28}{}, {}",
        "brute",
        ok_n(total - ambiguous.len()),
        bad_n(ambiguous.len(), "ambiguous")
    )
    .unwrap();
    writeln!(w, "  {}", bold("Solution checks")).unwrap();
    writeln!(
        w,
        "    deduce vs brute     {}, {}",
        ok_n(total - mismatches.len()),
        bad_n(mismatches.len(), "mismatch")
    )
    .unwrap();
    writeln!(
        w,
        "    unique answer       {}, {}",
        ok_n(total - not_answerable.len()),
        bad_n(not_answerable.len(), "ambiguous")
    )
    .unwrap();

    if !form_warnings.is_empty() {
        writeln!(w, "\nWarnings:").unwrap();
        writeln!(
            w,
            "  Form ({}): {}",
            form_warnings.len(),
            form_warnings.join(" ")
        )
        .unwrap();
    }

    if !recipe_stuck.is_empty() {
        writeln!(
            w,
            "\nStuck at recipe depth ({}): {}",
            recipe_stuck.len(),
            recipe_stuck.join(" ")
        )
        .unwrap();
    }

    if has_errors {
        writeln!(w, "\nErrors:").unwrap();
        if !form_errors.is_empty() {
            writeln!(
                w,
                "  Form ({}): {}",
                form_errors.len(),
                form_errors.join(" ")
            )
            .unwrap();
        }
        if !stuck.is_empty() {
            writeln!(w, "  Stuck ({}): {}", stuck.len(), stuck.join(" ")).unwrap();
        }
        if !contradictions.is_empty() {
            writeln!(
                w,
                "  Contradiction ({}): {}",
                contradictions.len(),
                contradictions.join(" ")
            )
            .unwrap();
        }
        if !ambiguous.is_empty() {
            writeln!(
                w,
                "  Ambiguous ({}): {}",
                ambiguous.len(),
                ambiguous.join(" ")
            )
            .unwrap();
        }
        if !mismatches.is_empty() {
            writeln!(
                w,
                "  Mismatch ({}): {}",
                mismatches.len(),
                mismatches.join(" ")
            )
            .unwrap();
        }
        if !not_answerable.is_empty() {
            writeln!(
                w,
                "  Ambiguous-answer ({}): {}",
                not_answerable.len(),
                not_answerable.join(" ")
            )
            .unwrap();
        }
    }

    has_errors
}

// ── Entry points ──

/// Render a computed `CheckOutput` and return whether it contained errors. A
/// single puzzle or a targeted lookup gets the detailed per-puzzle view; a whole
/// year map gets the summary.
fn render_check_output(w: &mut impl Write, output: &CheckOutput) -> bool {
    if (output.single || output.target.is_some()) && !output.puzzles.is_empty() {
        format_single(w, &output.puzzles[0])
    } else {
        format_full(w, &output.puzzles, &output.path)
    }
}

pub fn check_command(path: &str, target: Option<&str>, json_output: bool) {
    let output = compute_check_output(path, target);

    if json_output {
        println!("{}", serde_json::to_string(&output).unwrap());
    } else {
        init_color(std::io::IsTerminal::is_terminal(&std::io::stderr()));
        let mut w = std::io::stderr().lock();
        if render_check_output(&mut w, &output) {
            std::process::exit(1);
        }
    }
}

pub fn format_check_stdin() {
    init_color(std::io::IsTerminal::is_terminal(&std::io::stdout()));
    let input = std::io::read_to_string(std::io::stdin()).expect("failed to read stdin");
    let output: CheckOutput = serde_json::from_str(&input).expect("invalid JSON");
    let mut w = std::io::stdout().lock();
    if render_check_output(&mut w, &output) {
        std::process::exit(1);
    }
}

// ── Conflict detection (used by run_check) ──

struct IncorrectActionReport {
    index: usize,
    summary: String,
    details: Vec<String>,
}

fn format_deduce_action(action: &deduce::DeduceAction) -> String {
    match *action {
        deduce::DeduceAction::Force { qi, answer } => {
            format!("force Q{}={}", qi + 1, answer.as_char())
        }
        deduce::DeduceAction::Eliminate { qi, oi } => {
            format!("eliminate Q{}{}", qi + 1, Answer::from(oi as u8).as_char())
        }
        deduce::DeduceAction::EliminateMulti {
            question_mask,
            option_mask,
        } => {
            let qs: Vec<String> = (0..MAX_N)
                .filter(|&qi| (question_mask >> qi) & 1 == 1)
                .map(|qi| format!("Q{}", qi + 1))
                .collect();
            let opts: String = (0..5usize)
                .filter(|&oi| (option_mask >> oi) & 1 == 1)
                .map(|oi| Answer::from(oi as u8).as_char())
                .collect();
            format!("eliminate-multi [{}] options [{}]", qs.join(", "), opts)
        }
    }
}

fn first_incorrect_action(
    steps: &[solve_deduce::SolveStep],
    solution: &[Answer; MAX_N],
    n: usize,
) -> Option<IncorrectActionReport> {
    for (idx, step) in steps.iter().enumerate() {
        match step {
            solve_deduce::SolveStep::Deduce(dr) => match dr.action {
                deduce::DeduceAction::Force { qi, answer } => {
                    if solution[qi] != answer {
                        return Some(IncorrectActionReport {
                            index: idx + 1,
                            summary: format!(
                                "force Q{}={} by {} (expected {})",
                                qi + 1,
                                answer.as_char(),
                                dr.rule.to_str(),
                                solution[qi].as_char(),
                            ),
                            details: Vec::new(),
                        });
                    }
                }
                deduce::DeduceAction::Eliminate { qi, oi } => {
                    if solution[qi] == Answer::from(oi as u8) {
                        return Some(IncorrectActionReport {
                            index: idx + 1,
                            summary: format!(
                                "eliminate Q{}{} by {} (eliminates true answer)",
                                qi + 1,
                                Answer::from(oi as u8).as_char(),
                                dr.rule.to_str(),
                            ),
                            details: Vec::new(),
                        });
                    }
                }
                deduce::DeduceAction::EliminateMulti {
                    question_mask,
                    option_mask,
                } => {
                    for qi in 0..n {
                        if (question_mask >> qi) & 1 == 0 {
                            continue;
                        }
                        let sol_oi = solution[qi].idx();
                        if (option_mask >> sol_oi) & 1 == 1 {
                            return Some(IncorrectActionReport {
                                index: idx + 1,
                                summary: format!(
                                    "eliminate-multi by {} removes Q{}{} (true answer)",
                                    dr.rule.to_str(),
                                    qi + 1,
                                    solution[qi].as_char(),
                                ),
                                details: Vec::new(),
                            });
                        }
                    }
                }
            },
            solve_deduce::SolveStep::Lookahead(lr) => {
                if solution[lr.eliminate_qi] == Answer::from(lr.eliminate_oi as u8) {
                    let mut details = vec![
                        format!(
                            "assumption: Q{}={}",
                            lr.assumption_qi + 1,
                            lr.assumption_answer.as_char()
                        ),
                        format!("contradiction at Q{}", lr.contradiction_qi + 1),
                    ];
                    if lr.chain.is_empty() {
                        details.push("deduction chain: (empty)".to_string());
                    } else {
                        details.push("deduction chain:".to_string());
                        for (i, dr) in lr.chain.iter().enumerate() {
                            details.push(format!(
                                "  {}. {} via {}",
                                i + 1,
                                format_deduce_action(&dr.action),
                                dr.rule.to_str()
                            ));
                        }
                    }

                    return Some(IncorrectActionReport {
                        index: idx + 1,
                        summary: format!(
                            "lookahead eliminate Q{}{} (eliminates true answer)",
                            lr.eliminate_qi + 1,
                            Answer::from(lr.eliminate_oi as u8).as_char(),
                        ),
                        details,
                    });
                }
            }
        }
    }
    None
}

// ── Solve engine ──

pub struct CheckResult {
    pub ok: bool,
    pub steps: Vec<String>,
    pub answers: [Option<Answer>; MAX_N],
    pub eliminated: [u8; MAX_N],
}

pub fn run_check(fp: &FlatPuzzle, key: &str) -> CheckResult {
    let mut log = solve_deduce::StepLog::default();
    let out = solve_deduce::run_engine(
        fp,
        fp.initial_state,
        solve_deduce::EngineConfig::verify(),
        fp.n * solve_deduce::VERIFY_ITERS_PER_QUESTION,
        &mut log,
    );
    // `run_engine` flags a self-contradiction (an unsound rule forcing a cell two
    // ways). It never fires for a sound engine on a well-posed puzzle, but if it
    // does, report the first action that deviated from the brute solution.
    if out.contradiction.is_some() {
        report_conflict(fp, key, &log.0);
    }
    CheckResult {
        ok: out.solved,
        steps: solve_deduce::format_steps(&log.0),
        answers: out.state.answers,
        eliminated: out.state.eliminated,
    }
}

/// Using brute as ground truth, print the first recorded solve step that
/// removed/contradicted the true answer. Only called when `run_engine`
/// reported a self-contradiction.
fn report_conflict(fp: &FlatPuzzle, key: &str, steps: &[solve_deduce::SolveStep]) {
    eprintln!("CONFLICT [{key}]: hint engine forced a cell two ways — an unsound rule");
    let solutions = solve_brute::solve(fp, 2);
    match solutions.len() {
        0 => {
            eprintln!(
                "CONFLICT [{key}]: brute-force solver found no solutions; cannot locate first incorrect action"
            );
        }
        1 => {
            if let Some(report) = first_incorrect_action(steps, &solutions[0], fp.n) {
                eprintln!(
                    "CONFLICT [{key}]: first incorrect action #{}: {}",
                    report.index, report.summary
                );
                for line in report.details {
                    eprintln!("CONFLICT [{key}]:   {line}");
                }
            } else {
                eprintln!(
                    "CONFLICT [{key}]: no incorrect force/elimination found before conflict against unique solution"
                );
            }
        }
        m => {
            eprintln!(
                "CONFLICT [{key}]: brute-force solver found {m} solutions; first incorrect action is ambiguous"
            );
        }
    }
}
