use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::check_answer;
use crate::check_form;
use crate::check_well_posed;
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
    pub brute_count: usize,
    pub brute_solutions: Vec<String>,
    pub hint_brute_match: bool,
    pub validity_ok: bool,
    pub validity_per_question: Vec<String>,
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

fn make_url(year: &str, day: &str, lvl: &str, steps: &[String]) -> String {
    let mm = day.get(..2).expect("day key must be MMDD");
    let dd = day.get(2..4).expect("day key must be MMDD");
    let hash = steps.join(".");
    format!("http://localhost:5173/{year}-{mm}-{dd}/{lvl}?debug#{hash}")
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

fn check_one_puzzle(fp: &FlatPuzzle, key: &str) -> PuzzleCheckResult {
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
            brute_count: 1,
            brute_solutions: Vec::new(),
            hint_brute_match: true,
            validity_ok: true,
            validity_per_question: vec!["n/a".to_string(); n],
            ambiguous: Vec::new(),
        };
    }

    let cr = run_check(fp, key);
    let answered = cr.answers[..n].iter().filter(|a| a.is_some()).count();

    let solutions = solve_brute::solve(fp, None, 10);

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

    let hint_brute_match = if cr.ok && solutions.len() == 1 {
        (0..n).all(|i| cr.answers[i] == Some(solutions[0][i]))
    } else {
        true
    };

    let state = State {
        answers: cr.answers,
        eliminated: cr.eliminated,
    };
    let validity_ok = if cr.ok {
        (0..n).all(|i| {
            let v = check_answer::check_answer(fp, state, i);
            v.is_valid() || v == check_answer::Validity::Pending
        })
    } else {
        true
    };
    let validity_per_question: Vec<String> = (0..n)
        .map(|i| {
            if cr.ok {
                match check_answer::check_answer(fp, state, i) {
                    check_answer::Validity::Valid => "valid",
                    check_answer::Validity::Consistent => "consistent",
                    check_answer::Validity::Invalid => "invalid",
                    check_answer::Validity::Pending => "pending",
                    check_answer::Validity::Neutral => "neutral",
                }
                .into()
            } else {
                "n/a".into()
            }
        })
        .collect();

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
        brute_count,
        brute_solutions,
        hint_brute_match,
        validity_ok,
        validity_per_question,
        ambiguous,
    }
}

fn compute_check_output(path: &str, target: Option<&str>) -> CheckOutput {
    let data: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("can't read file"))
            .expect("invalid JSON");
    let obj = data.as_object().expect("top-level must be object");
    let year = extract_year(path);

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
            puzzles.push(check_one_puzzle(&fp, &key));
        }
    }

    CheckOutput {
        path: path.to_string(),
        year,
        target: target.map(|s| s.to_string()),
        puzzles,
    }
}

// ── Format ──

fn format_single(w: &mut impl Write, r: &PuzzleCheckResult, year: &str) -> bool {
    let n = r.n;
    let (day, lvl) = r.key.split_once('-').expect("check key must be MMDD-L");

    let has_form_warns = !r.form_warnings.is_empty();
    let has_errors = !r.solve_ok
        || r.brute_count != 1
        || !r.hint_brute_match
        || !r.validity_ok
        || !r.ambiguous.is_empty()
        || !r.form_errors.is_empty();

    let verdict = if !r.hint_brute_match {
        red("MISMATCH")
    } else if !r.validity_ok {
        red("INVALID")
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
    writeln!(w, "  {:<28} {solve_label}", "Deduce+lookahead").unwrap();
    writeln!(w, "    {}", dim(&make_url(year, day, lvl, &r.solve_steps))).unwrap();

    // Brute
    if r.brute_count == 1 {
        writeln!(
            w,
            "  {:<28} {}",
            "Brute",
            green(&format!("1 solution ({})", r.brute_solutions[0]))
        )
        .unwrap();
        let steps = solution_str_to_steps(&r.brute_solutions[0]);
        writeln!(w, "    {}", dim(&make_url(year, day, lvl, &steps))).unwrap();
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
            let steps = solution_str_to_steps(sol);
            writeln!(w, "      {}", dim(&make_url(year, day, lvl, &steps))).unwrap();
        }
    }

    // Match
    let match_label = if r.hint_brute_match {
        green("ok")
    } else {
        red("MISMATCH")
    };
    writeln!(w, "  {:<28} {match_label}", "Deduce+lookahead vs brute").unwrap();

    // Validity
    let validity_label = if r.validity_ok {
        green("ok")
    } else {
        red("INVALID")
    };
    writeln!(w, "  {:<28} {validity_label}", "Answer validity").unwrap();
    if !r.validity_ok {
        for (i, v) in r.validity_per_question.iter().enumerate() {
            writeln!(w, "    {}", dim(&format!("Q{}: {v}", i + 1))).unwrap();
        }
    }

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
    let mut validity_fails: Vec<&str> = Vec::new();
    let mut not_answerable: Vec<&str> = Vec::new();

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
        if !r.validity_ok {
            validity_fails.push(&r.key);
        }
        if !r.ambiguous.is_empty() {
            not_answerable.push(&r.key);
        }
    }

    let mut failed_set: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for v in [
        &stuck,
        &contradictions,
        &ambiguous,
        &mismatches,
        &validity_fails,
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
        || !validity_fails.is_empty()
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
        "    deduce+lookahead    {}, {}, {}",
        ok_n(n_solve_ok),
        bad_n(stuck.len(), "stuck"),
        bad_n(contradictions.len(), "contradiction")
    )
    .unwrap();
    writeln!(
        w,
        "    brute               {}, {}",
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
    let validity_label = if stuck.is_empty() && contradictions.is_empty() && ambiguous.is_empty() {
        format!(
            "{}, {}",
            ok_n(total - validity_fails.len()),
            bad_n(validity_fails.len(), "invalid")
        )
    } else {
        let applicable = total - stuck.len() - contradictions.len();
        format!(
            "{}, {} ({})",
            ok_n(applicable - validity_fails.len()),
            bad_n(validity_fails.len(), "invalid"),
            dim(&format!("{} n/a", stuck.len() + contradictions.len()))
        )
    };
    writeln!(w, "    answer validity     {validity_label}").unwrap();
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
        if !validity_fails.is_empty() {
            writeln!(
                w,
                "  Validity ({}): {}",
                validity_fails.len(),
                validity_fails.join(" ")
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

pub fn check_command(path: &str, target: Option<&str>, json_output: bool) {
    let output = compute_check_output(path, target);

    if json_output {
        println!("{}", serde_json::to_string(&output).unwrap());
    } else {
        init_color(std::io::IsTerminal::is_terminal(&std::io::stderr()));
        let mut w = std::io::stderr().lock();
        let has_errors = if output.target.is_some() && !output.puzzles.is_empty() {
            format_single(&mut w, &output.puzzles[0], &output.year)
        } else {
            format_full(&mut w, &output.puzzles, &output.path)
        };
        if has_errors {
            std::process::exit(1);
        }
    }
}

pub fn format_check_stdin() {
    init_color(std::io::IsTerminal::is_terminal(&std::io::stdout()));
    let input = std::io::read_to_string(std::io::stdin()).expect("failed to read stdin");
    let output: CheckOutput = serde_json::from_str(&input).expect("invalid JSON");
    let mut w = std::io::stdout().lock();
    let has_errors = if output.target.is_some() && !output.puzzles.is_empty() {
        format_single(&mut w, &output.puzzles[0], &output.year)
    } else {
        format_full(&mut w, &output.puzzles, &output.path)
    };
    if has_errors {
        std::process::exit(1);
    }
}

// ── Conflict detection (used by run_check) ──

#[derive(Clone)]
struct LookaheadTrace {
    eliminate_qi: usize,
    eliminate_oi: usize,
    assumption_qi: usize,
    assumption_answer: Answer,
    contradiction_qi: usize,
    chain: Vec<deduce::DeduceResult>,
}

#[derive(Clone)]
enum CheckAction {
    Force {
        qi: usize,
        answer: Answer,
        rule: deduce::DeduceRule,
    },
    Eliminate {
        qi: usize,
        oi: usize,
        rule: deduce::DeduceRule,
    },
    EliminateMulti {
        question_mask: u16,
        option_mask: u8,
        rule: deduce::DeduceRule,
    },
    LookaheadEliminate {
        trace: LookaheadTrace,
    },
}

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
    actions: &[CheckAction],
    solution: &[Answer; MAX_N],
    n: usize,
) -> Option<IncorrectActionReport> {
    for (idx, action) in actions.iter().enumerate() {
        match action {
            CheckAction::Force { qi, answer, rule } => {
                if solution[*qi] != *answer {
                    return Some(IncorrectActionReport {
                        index: idx + 1,
                        summary: format!(
                            "force Q{}={} by {} (expected {})",
                            *qi + 1,
                            answer.as_char(),
                            rule.to_str(),
                            solution[*qi].as_char(),
                        ),
                        details: Vec::new(),
                    });
                }
            }
            CheckAction::Eliminate { qi, oi, rule } => {
                if solution[*qi] == Answer::from(*oi as u8) {
                    return Some(IncorrectActionReport {
                        index: idx + 1,
                        summary: format!(
                            "eliminate Q{}{} by {} (eliminates true answer)",
                            *qi + 1,
                            Answer::from(*oi as u8).as_char(),
                            rule.to_str(),
                        ),
                        details: Vec::new(),
                    });
                }
            }
            CheckAction::EliminateMulti {
                question_mask,
                option_mask,
                rule,
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
                                rule.to_str(),
                                qi + 1,
                                solution[qi].as_char(),
                            ),
                            details: Vec::new(),
                        });
                    }
                }
            }
            CheckAction::LookaheadEliminate { trace } => {
                if solution[trace.eliminate_qi] == Answer::from(trace.eliminate_oi as u8) {
                    let mut details = vec![
                        format!(
                            "assumption: Q{}={}",
                            trace.assumption_qi + 1,
                            trace.assumption_answer.as_char()
                        ),
                        format!("contradiction at Q{}", trace.contradiction_qi + 1),
                    ];
                    if trace.chain.is_empty() {
                        details.push("deduction chain: (empty)".to_string());
                    } else {
                        details.push("deduction chain:".to_string());
                        for (i, dr) in trace.chain.iter().enumerate() {
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
                            trace.eliminate_qi + 1,
                            Answer::from(trace.eliminate_oi as u8).as_char(),
                        ),
                        details,
                    });
                }
            }
        }
    }
    None
}

fn report_first_incorrect_if_needed(
    key: &str,
    fp: &FlatPuzzle,
    actions: &[CheckAction],
    n: usize,
    conflict_reported: &mut bool,
    brute_solutions: &mut Option<Vec<[Answer; MAX_N]>>,
) {
    if *conflict_reported {
        return;
    }
    *conflict_reported = true;

    let solutions = brute_solutions.get_or_insert_with(|| solve_brute::solve(fp, None, 2));
    match solutions.len() {
        0 => {
            eprintln!(
                "CONFLICT [{key}]: brute-force solver found no solutions; cannot locate first incorrect action"
            );
        }
        1 => {
            if let Some(report) = first_incorrect_action(actions, &solutions[0], n) {
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

/// Replay the recorded solve trace as a `CheckAction` log and, using brute as
/// ground truth, print the first action that removed/contradicted the true
/// answer. Only called when `run_engine` reported a self-contradiction.
fn report_conflict(fp: &FlatPuzzle, key: &str, steps: &[solve_deduce::SolveStep]) {
    let actions: Vec<CheckAction> = steps.iter().map(step_to_action).collect();
    eprintln!("CONFLICT [{key}]: hint engine forced a cell two ways — an unsound rule");
    let mut conflict_reported = false;
    let mut brute_solutions = None;
    report_first_incorrect_if_needed(
        key,
        fp,
        &actions,
        fp.n,
        &mut conflict_reported,
        &mut brute_solutions,
    );
}

fn step_to_action(step: &solve_deduce::SolveStep) -> CheckAction {
    match step {
        solve_deduce::SolveStep::Deduce(dr) => match dr.action {
            deduce::DeduceAction::Force { qi, answer } => CheckAction::Force {
                qi,
                answer,
                rule: dr.rule,
            },
            deduce::DeduceAction::Eliminate { qi, oi } => CheckAction::Eliminate {
                qi,
                oi,
                rule: dr.rule,
            },
            deduce::DeduceAction::EliminateMulti {
                question_mask,
                option_mask,
            } => CheckAction::EliminateMulti {
                question_mask,
                option_mask,
                rule: dr.rule,
            },
        },
        solve_deduce::SolveStep::Lookahead(lr) => CheckAction::LookaheadEliminate {
            trace: LookaheadTrace {
                eliminate_qi: lr.eliminate_qi,
                eliminate_oi: lr.eliminate_oi,
                assumption_qi: lr.assumption_qi,
                assumption_answer: lr.assumption_answer,
                contradiction_qi: lr.contradiction_qi,
                chain: lr.chain.iter().copied().collect(),
            },
        },
    }
}
