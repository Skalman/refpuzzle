#![allow(clippy::needless_range_loop)]

mod build;
mod check_validity;
mod construct;
mod deduce;
mod difficulty;
mod evaluate;
mod lookahead;
mod rng;
mod solve_brute;
#[allow(dead_code)]
mod solve_deduce;
mod types;

use build::GenerateResult;
use difficulty::PROFILES;
use rng::Rng;
use serde_json::{Value, json};
use std::time::Instant;
use types::*;

struct DateRange {
    year: u32,
    start_mm: u32,
    start_dd: u32,
    end_mm: u32,
    end_dd: u32,
}

fn parse_date_range(input: &str) -> DateRange {
    let (start_str, end_str) = if let Some((a, b)) = input.split_once("..") {
        (a, Some(b))
    } else {
        (input, None)
    };

    let parse_part = |s: &str| -> (u32, Option<u32>, Option<u32>) {
        let parts: Vec<&str> = s.split('-').collect();
        match parts.len() {
            1 => (parts[0].parse().expect("invalid year"), None, None),
            2 => (
                parts[0].parse().expect("invalid year"),
                Some(parts[1].parse().expect("invalid month")),
                None,
            ),
            3 => (
                parts[0].parse().expect("invalid year"),
                Some(parts[1].parse().expect("invalid month")),
                Some(parts[2].parse().expect("invalid day")),
            ),
            _ => {
                eprintln!("Invalid date format: {s}");
                std::process::exit(1);
            }
        }
    };

    fn last_day(year: u32, month: u32) -> u32 {
        match month {
            2 => {
                if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
                {
                    29
                } else {
                    28
                }
            }
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        }
    }

    let (sy, sm, sd) = parse_part(start_str);
    let start_mm = sm.unwrap_or(if sy == 2026 { 4 } else { 1 });
    let start_dd = sd.unwrap_or(if sy == 2026 && sm.is_none() { 19 } else { 1 });

    let (ey, em, ed) = if let Some(e) = end_str {
        parse_part(e)
    } else {
        (sy, sm, sd)
    };

    if sy != ey {
        eprintln!("Date range must not cross year boundaries: {input}");
        std::process::exit(1);
    }

    let end_mm = em.unwrap_or(12);
    let end_dd = ed.unwrap_or_else(|| last_day(ey, end_mm));

    let launch = 20260419u32;
    let mut start_mm = start_mm;
    let mut start_dd = start_dd;
    let start_val = sy * 10000 + start_mm * 100 + start_dd;
    if sy == 2026 && start_val < launch {
        start_mm = 4;
        start_dd = 19;
    }
    let start_val = sy * 10000 + start_mm * 100 + start_dd;
    if start_val < launch {
        eprintln!("Date range must not start before 2026-04-19: {input}");
        std::process::exit(1);
    }
    let end_val = ey * 10000 + end_mm * 100 + end_dd;
    if start_val > end_val {
        eprintln!(
            "Start date is after end date: {}-{:02}-{:02}..{}-{:02}-{:02}",
            sy, start_mm, start_dd, ey, end_mm, end_dd
        );
        std::process::exit(1);
    }

    DateRange {
        year: sy,
        start_mm,
        start_dd,
        end_mm,
        end_dd,
    }
}

fn print_help() {
    eprintln!("Usage: refpuzzle gen <date-range> -o FILE [options]");
    eprintln!("       refpuzzle check <file.json> [MMDD-level]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -o FILE       output file (required, - for stdout)");
    eprintln!("  -m            merge into existing file");
    eprintln!("  -l, --level   generate only this level (1-6)");
    eprintln!("  -a, --attempts  max attempts per seed (default 100)");
    eprintln!("  --stats       show generation statistics");
    eprintln!("  --trace       show trace output");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  refpuzzle gen 2051 -o out.json");
    eprintln!("  refpuzzle gen 2051-03 -o out.json -l 4");
    eprintln!("  refpuzzle gen 2051-01..2051-06 -o out.json -m");
    eprintln!("  refpuzzle check puzzles/daily/2051.json");
    eprintln!("  refpuzzle check puzzles/daily/2051.json 0315-4");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 || args[1] == "--help" || args[1] == "-h" {
        print_help();
        if args.len() >= 2 {
            return;
        }
        std::process::exit(1);
    }

    match args[1].as_str() {
        "gen" => {}
        "check" => {
            if args.len() < 3 {
                eprintln!("Usage: refpuzzle check <file.json> [MMDD-level]");
                std::process::exit(1);
            }
            let file = &args[2];
            let target = args.get(3).cloned();
            check_json(file, target.as_deref());
            return;
        }
        _ => {
            eprintln!("Unknown subcommand: {}. Use 'gen' or 'check'.", args[1]);
            std::process::exit(1);
        }
    }

    // Parse gen subcommand args
    let mut date_range_str: Option<String> = None;
    let mut max_attempts: usize = 100;
    let mut level_filter: Option<u8> = None;
    let mut show_stats = false;
    let mut trace = false;
    let mut output_path: Option<String> = None;
    let mut merge = false;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--attempts" | "-a" => {
                i += 1;
                max_attempts = args[i].parse().expect("invalid attempts");
            }
            "--level" | "-l" => {
                i += 1;
                let l: u8 = args[i].parse().expect("invalid level");
                assert!((1..=6).contains(&l), "level must be 1-6");
                level_filter = Some(l);
            }
            "--output" | "-o" => {
                i += 1;
                output_path = Some(args[i].clone());
            }
            "--merge" | "-m" => {
                merge = true;
            }
            "--stats" => {
                show_stats = true;
            }
            "--trace" => {
                trace = true;
            }
            "--help" | "-h" => {
                print_help();
                return;
            }
            other => {
                if other.starts_with('-') {
                    eprintln!("Unknown option: {other}");
                    std::process::exit(1);
                }
                date_range_str = Some(other.to_string());
            }
        }
        i += 1;
    }

    let date_range_str = date_range_str.unwrap_or_else(|| {
        eprintln!("Error: date range is required. Example: refpuzzle gen 2051 -o out.json");
        std::process::exit(1);
    });
    let dr = parse_date_range(&date_range_str);
    let year = dr.year;
    let start = format!("{}-{:02}-{:02}", year, dr.start_mm, dr.start_dd);
    let end = format!("{}-{:02}-{:02}", year, dr.end_mm, dr.end_dd);
    let start_mm = dr.start_mm;
    let start_dd = dr.start_dd;
    let end_mm = dr.end_mm;
    let end_dd = dr.end_dd;

    let output_path = output_path.unwrap_or_else(|| {
        eprintln!("Error: -o/--output is required (use -o - for stdout)");
        std::process::exit(1);
    });

    let days = dates_in_year(year, start_mm, start_dd, end_mm, end_dd);
    let day_count = days.len();

    eprintln!(
        "Generating {} days for year {} ({}..{})...",
        day_count, year, start, end,
    );

    let start_time = Instant::now();
    let levels: Vec<u8> = match level_filter {
        Some(l) => vec![l],
        None => vec![1, 2, 3, 4, 5, 6],
    };

    // Generate all (day, level) pairs in parallel
    let tasks: Vec<(usize, u8)> = (0..day_count)
        .flat_map(|d| levels.iter().map(move |&l| (d, l)))
        .collect();

    // Derive per-task seeds deterministically from (year, mmdd, level).
    // Each retry gets a different seed by mixing in the retry index.
    // This means the same date always produces the same puzzle regardless of --start.
    let task_seeds: Vec<Vec<u32>> = tasks
        .iter()
        .map(|&(day_idx, level)| {
            let (mm, dd) = days[day_idx];
            let date_key = year * 10000 + mm * 100 + dd;
            (0..100u32)
                .map(|retry| {
                    date_key
                        .wrapping_mul(31)
                        .wrapping_add(level as u32)
                        .wrapping_mul(17)
                        .wrapping_add(retry.wrapping_mul(0x9e3779b9))
                })
                .collect()
        })
        .collect();

    use rayon::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let done_by_level: [AtomicUsize; 6] = std::array::from_fn(|_| AtomicUsize::new(0));
    let last_report = std::sync::Mutex::new(Instant::now());
    let total = tasks.len();

    let results: Vec<((usize, u8), Option<GenerateResult>, build::Stats)> = tasks
        .par_iter()
        .zip(task_seeds.par_iter())
        .map(|(&(day_idx, level), seeds)| {
            let profile = &PROFILES[level as usize - 1];
            let mut result = None;
            let mut stats = build::Stats::default();
            for &s in seeds {
                let mut rng = Rng::new(s);
                if let Some(r) =
                    construct::generate(profile, &mut rng, max_attempts, &mut stats, trace)
                {
                    result = Some(r);
                    break;
                }
            }
            done_by_level[level as usize - 1].fetch_add(1, Ordering::Relaxed);
            if let Ok(mut last) = last_report.try_lock()
                && last.elapsed().as_secs() >= 15
            {
                let counts: Vec<usize> = (0..6)
                    .map(|i| done_by_level[i].load(Ordering::Relaxed))
                    .collect();
                let done_total: usize = counts.iter().sum();
                eprintln!(
                    "  {done_total}/{total}: L1={} L2={} L3={} L4={} L5={} L6={}",
                    counts[0], counts[1], counts[2], counts[3], counts[4], counts[5],
                );
                *last = Instant::now();
            }
            ((day_idx, level), result, stats)
        })
        .collect();

    let mut stats = build::Stats::default();
    for (_, _, s) in &results {
        stats.merge(s);
    }

    // Assemble into { "_seed": N, "MMDD": { "level-1": ..., ... }, ... }
    let mut year_map = serde_json::Map::new();
    let mut ok_count = 0;
    let mut fail_count = 0;

    for &(mm, dd) in &days {
        year_map.insert(
            format!("{mm:02}{dd:02}"),
            Value::Object(serde_json::Map::new()),
        );
    }

    for ((day_idx, level), result, _) in &results {
        let (mm, dd) = days[*day_idx];
        let key = format!("{mm:02}{dd:02}");
        match result {
            Some(r) => {
                ok_count += 1;
                let puzzle_json = puzzle_to_json(r, *level as usize);
                if let Some(Value::Object(day)) = year_map.get_mut(&key) {
                    day.insert(format!("{level}"), puzzle_json);
                }
            }
            None => {
                fail_count += 1;
                eprintln!("  FAILED: {} L{}", key, level);
            }
        }
    }

    let elapsed = start_time.elapsed();

    eprintln!();
    eprintln!("=== Summary ===");
    eprintln!("  Year:    {year}");
    eprintln!("  Start:   {start}");
    eprintln!("  Days:    {day_count}");
    eprintln!(
        "  Puzzles: {ok_count}/{} ({fail_count} failed)",
        tasks.len()
    );
    eprintln!(
        "  Time:    {:.1}s ({:.1}ms per day)",
        elapsed.as_secs_f64(),
        elapsed.as_secs_f64() * 1000.0 / day_count as f64
    );
    eprintln!("  Output:  {}", output_path);
    if show_stats {
        stats.print();
    }

    if merge {
        assert!(
            output_path != "-",
            "--merge requires -o FILE (cannot merge to stdout)"
        );
        let existing = std::fs::read_to_string(&output_path).unwrap_or_else(|_| "{}".into());
        let mut existing: serde_json::Map<String, Value> =
            serde_json::from_str(&existing).expect("invalid JSON in output file");
        for (date, levels) in year_map {
            let entry = existing
                .entry(date)
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            if let (Value::Object(existing_day), Value::Object(new_levels)) = (entry, levels) {
                for (lvl, puzzle) in new_levels {
                    existing_day.insert(lvl, puzzle);
                }
            }
        }
        let out = serde_json::to_string(&Value::Object(existing)).unwrap();
        std::fs::write(&output_path, out).expect("failed to write output file");
    } else {
        let out = serde_json::to_string(&Value::Object(year_map)).unwrap();
        if output_path == "-" {
            println!("{out}");
        } else {
            std::fs::write(&output_path, out).expect("failed to write output file");
        }
    }
}

fn option_value_json(qt: &QuestionType, qi: usize, oi: usize, fp: &FlatPuzzle) -> Value {
    // Letter-type question types store values in option_answers as letter indices (already 0-based)
    if matches!(
        qt,
        QuestionType::AnswerOf { .. } | QuestionType::LeastCommon | QuestionType::MostCommon
    ) {
        let a = fp.option_answers[qi][oi];
        if a > 4 {
            return Value::Null;
        }
        return json!(a);
    }
    // Identity-option types: option A=0, B=1, C=2, D=3, E=4
    if qt.has_identity_options() {
        return json!(oi);
    }
    let v = fp.option_nums[qi][oi];
    if v == NONE_VAL || v == NAN_VAL {
        return Value::Null;
    }
    json!(v)
}

fn puzzle_to_json(result: &GenerateResult, _level: usize) -> Value {
    let n = result.n;
    let oc = result.fp.option_count;
    let questions: Vec<Value> = (0..n)
        .map(|qi| {
            let qt = &result.question_types[qi];
            let mut q = serde_json::Map::new();
            if let QuestionType::TrueStmt = qt {
                let claims: Vec<Value> = (0..oc)
                    .map(|oi| claim_to_json(&result.fp.option_claims[qi][oi]))
                    .collect();
                q.insert("c".into(), json!(claims));
            } else {
                let options: Vec<Value> = (0..oc)
                    .map(|oi| option_value_json(qt, qi, oi, &result.fp))
                    .collect();
                q.insert("o".into(), json!(options));
            }
            q.insert("t".into(), question_type_to_json(qt));
            Value::Object(q)
        })
        .collect();

    json!({ "q": questions })
}

fn dates_in_year(
    year: u32,
    start_mm: u32,
    start_dd: u32,
    end_mm: u32,
    end_dd: u32,
) -> Vec<(u32, u32)> {
    let days_in_month = |m: u32| -> u32 {
        match m {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
                {
                    29
                } else {
                    28
                }
            }
            _ => 0,
        }
    };
    let mut result = Vec::new();
    let mut mm = start_mm;
    let mut dd = start_dd;
    while mm <= 12 {
        while dd <= days_in_month(mm) {
            if mm > end_mm || (mm == end_mm && dd > end_dd) {
                return result;
            }
            result.push((mm, dd));
            dd += 1;
        }
        mm += 1;
        dd = 1;
    }
    result
}

fn question_type_to_json(qt: &QuestionType) -> Value {
    serde_json::to_value(qt).unwrap()
}

fn check_json(path: &str, target: Option<&str>) {
    let data: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("can't read file"))
            .expect("invalid JSON");
    let obj = data.as_object().expect("top-level must be object");

    let mut total = 0;
    let mut solved = 0;
    let mut failures = Vec::new();

    for (day, levels) in obj {
        let levels = levels.as_object().unwrap();
        for (lvl, puzzle) in levels {
            let key = format!("{day}-{lvl}");
            if let Some(t) = target
                && key != t
            {
                continue;
            }
            let fp = match parse_puzzle(puzzle) {
                Some(fp) => fp,
                None => {
                    eprintln!("  SKIP {key}: parse failed");
                    continue;
                }
            };
            total += 1;
            let (ok, steps) = run_check(&fp, &key);
            if ok {
                solved += 1;
            } else {
                let mut answered_set = std::collections::HashSet::new();
                for s in &steps {
                    if s.chars().last().is_some_and(|c| c.is_uppercase()) {
                        let qi: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
                        answered_set.insert(qi);
                    }
                }
                let answered = answered_set.len();
                let year = &path
                    .replace(".json", "")
                    .chars()
                    .rev()
                    .take(4)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>();
                let mm = &day[..2];
                let dd = &day[2..4];
                let level = lvl;
                let hash = steps.join(".");
                let url = format!("http://localhost:5173/{year}-{mm}-{dd}/{level}?debug#{hash}");
                failures.push(format!("{key}: {answered}/{} — {url}", fp.n));
            }
            if target.is_some() {
                let mut answered_set = std::collections::HashSet::new();
                for s in &steps {
                    if s.chars().last().is_some_and(|c| c.is_uppercase()) {
                        let qi: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
                        answered_set.insert(qi);
                    }
                }
                let status = if ok {
                    "solved"
                } else if answered_set.len() == fp.n {
                    "INVALID (all answered but solution is wrong)"
                } else {
                    "STUCK"
                };
                eprintln!(
                    "Hint engine: {status} {}/{} answered",
                    answered_set.len(),
                    fp.n
                );
                eprintln!("  {}", steps.join("."));
                if !ok {
                    let year = &path
                        .replace(".json", "")
                        .chars()
                        .rev()
                        .take(4)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect::<String>();
                    let mm = &day[..2];
                    let dd = &day[2..4];
                    let hash = steps.join(".");
                    eprintln!("  http://localhost:5173/day/{year}-{mm}-{dd}?l={lvl}&debug#{hash}");
                }
                let solutions = solve_brute::solve(&fp, None, 10);
                eprintln!("Brute-force: {} solution(s)", solutions.len());
                for (i, sol) in solutions.iter().enumerate() {
                    let s: String = sol.iter().take(fp.n).map(|a| a.as_char()).collect();
                    eprintln!("  #{}: {}", i + 1, s);
                }
                if !ok || solutions.len() != 1 {
                    std::process::exit(1);
                }
                return;
            }
        }
    }

    println!("{solved}/{total} solved");
    if !failures.is_empty() {
        println!("\nFailed ({}):", failures.len());
        for f in &failures {
            println!("  {f}");
        }
        std::process::exit(1);
    }
}

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
            format!("eliminate Q{}{}", qi + 1, LETTERS[oi].as_char())
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
                .map(|oi| LETTERS[oi].as_char())
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
                if solution[*qi] == LETTERS[*oi] {
                    return Some(IncorrectActionReport {
                        index: idx + 1,
                        summary: format!(
                            "eliminate Q{}{} by {} (eliminates true answer)",
                            *qi + 1,
                            LETTERS[*oi].as_char(),
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
                if solution[trace.eliminate_qi] == LETTERS[trace.eliminate_oi] {
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
                            LETTERS[trace.eliminate_oi].as_char(),
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

fn run_check(fp: &FlatPuzzle, key: &str) -> (bool, Vec<String>) {
    let n = fp.n;
    let pm = build::phantom_mask(fp.option_count);
    let mut answers: [Option<Answer>; MAX_N] = [None; MAX_N];
    let mut eliminated = [pm; MAX_N];
    let mut forced_by: [Option<deduce::DeduceRule>; MAX_N] = [None; MAX_N];
    let mut action_log: Vec<CheckAction> = Vec::new();
    let mut conflict_reported = false;
    let mut brute_solutions: Option<Vec<[Answer; MAX_N]>> = None;
    let mut steps = Vec::new();
    let letters_lower = ['a', 'b', 'c', 'd', 'e'];

    for _ in 0..n * 30 {
        if (0..n).all(|i| answers[i].is_some()) {
            let valid = (0..n).all(|i| {
                check_validity::check_question_against_solution(
                    fp,
                    i,
                    answers[i].unwrap(),
                    &answers,
                )
            });
            return (valid, steps);
        }
        let drs = deduce::deduce(fp, &answers, &eliminated);
        if !drs.is_empty() {
            for dr in &drs {
                match dr.action {
                    deduce::DeduceAction::Force { qi, answer } => {
                        action_log.push(CheckAction::Force {
                            qi,
                            answer,
                            rule: dr.rule,
                        });
                        if let Some(existing) = answers[qi] {
                            if existing != answer {
                                let origin = forced_by[qi].map_or("unknown", |r| r.to_str());
                                eprintln!(
                                    "CONFLICT [{key}]: Q{} forced {} by {} but already {} (set by {})",
                                    qi + 1,
                                    answer.as_char(),
                                    dr.rule.to_str(),
                                    existing.as_char(),
                                    origin,
                                );
                                report_first_incorrect_if_needed(
                                    key,
                                    fp,
                                    &action_log,
                                    n,
                                    &mut conflict_reported,
                                    &mut brute_solutions,
                                );
                            }
                        } else {
                            forced_by[qi] = Some(dr.rule);
                        }
                        eliminated[qi] = 0b11111 ^ (1 << answer.idx());
                        answers[qi] = Some(answer);
                        steps.push(format!("{}{}", qi + 1, answer.as_char()));
                    }
                    deduce::DeduceAction::Eliminate { qi, oi } => {
                        action_log.push(CheckAction::Eliminate {
                            qi,
                            oi,
                            rule: dr.rule,
                        });
                        if answers[qi] == Some(LETTERS[oi]) {
                            let origin = forced_by[qi].map_or("unknown", |r| r.to_str());
                            eprintln!(
                                "CONFLICT [{key}]: Q{} eliminating {} by {} but already forced to it (set by {})",
                                qi + 1,
                                LETTERS[oi].as_char(),
                                dr.rule.to_str(),
                                origin,
                            );
                            report_first_incorrect_if_needed(
                                key,
                                fp,
                                &action_log,
                                n,
                                &mut conflict_reported,
                                &mut brute_solutions,
                            );
                        }
                        eliminated[qi] |= 1 << oi;
                        steps.push(format!("{}{}", qi + 1, letters_lower[oi]));
                    }
                    deduce::DeduceAction::EliminateMulti {
                        question_mask,
                        option_mask,
                    } => {
                        action_log.push(CheckAction::EliminateMulti {
                            question_mask,
                            option_mask,
                            rule: dr.rule,
                        });
                        for i in 0..n {
                            if (question_mask >> i) & 1 == 1 {
                                eliminated[i] |= option_mask;
                                for oi in 0..5usize {
                                    if (option_mask >> oi) & 1 == 1 {
                                        if answers[i] == Some(LETTERS[oi]) {
                                            let origin =
                                                forced_by[i].map_or("unknown", |r| r.to_str());
                                            eprintln!(
                                                "CONFLICT [{key}]: Q{} eliminating {} by {} (multi) but already forced to it (set by {})",
                                                i + 1,
                                                LETTERS[oi].as_char(),
                                                dr.rule.to_str(),
                                                origin,
                                            );
                                            report_first_incorrect_if_needed(
                                                key,
                                                fp,
                                                &action_log,
                                                n,
                                                &mut conflict_reported,
                                                &mut brute_solutions,
                                            );
                                        }
                                        steps.push(format!("{}{}", i + 1, letters_lower[oi]));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            continue;
        }
        if let Some(lr) = lookahead::lookahead(fp, &answers, &eliminated, usize::MAX, false) {
            action_log.push(CheckAction::LookaheadEliminate {
                trace: LookaheadTrace {
                    eliminate_qi: lr.eliminate_qi,
                    eliminate_oi: lr.eliminate_oi,
                    assumption_qi: lr.assumption_qi,
                    assumption_answer: lr.assumption_answer,
                    contradiction_qi: lr.contradiction_qi,
                    chain: lr.chain.iter().copied().collect(),
                },
            });
            eliminated[lr.eliminate_qi] |= 1 << lr.eliminate_oi;
            steps.push(format!(
                "{}{}",
                lr.eliminate_qi + 1,
                letters_lower[lr.eliminate_oi]
            ));
            continue;
        }
        break;
    }
    (false, steps)
}

pub fn parse_puzzle(v: &Value) -> Option<FlatPuzzle> {
    let qs = v.get("q")?.as_array()?;
    let n = qs.len();
    if n == 0 || n > MAX_N {
        return None;
    }

    let option_count = qs
        .first()
        .and_then(|q| q.get("o"))
        .and_then(|o| o.as_array())
        .map_or(5, |a| a.len());

    let mut question_types = [QuestionType::AnswerIsSelf; MAX_N];
    let mut option_nums = [[NAN_VAL; 5]; MAX_N];
    let mut option_answers = [[0xFFu8; 5]; MAX_N];
    let mut option_claims: [[Option<Claim>; 5]; MAX_N] = [[None; 5]; MAX_N];

    for (qi, q) in qs.iter().enumerate() {
        let t = q.get("t")?;
        question_types[qi] = serde_json::from_value(t.clone()).ok()?;

        if let Some(claims) = q.get("c") {
            let claims = claims.as_array()?;
            for (oi, c) in claims.iter().enumerate() {
                if c.is_null() {
                    continue;
                }
                option_claims[qi][oi] = Some(serde_json::from_value(c.clone()).ok()?);
                option_nums[qi][oi] = NAN_VAL;
            }
        } else if let Some(opts) = q.get("o") {
            let opts = opts.as_array()?;
            for (oi, o) in opts.iter().enumerate() {
                if o.is_null() {
                    option_nums[qi][oi] = NONE_VAL;
                } else {
                    option_nums[qi][oi] = o.as_i64()? as i16;
                }
            }
            if matches!(
                question_types[qi],
                QuestionType::AnswerOf { .. }
                    | QuestionType::LeastCommon
                    | QuestionType::MostCommon
            ) {
                for oi in 0..5 {
                    option_answers[qi][oi] = if option_nums[qi][oi] >= 0 && option_nums[qi][oi] <= 4
                    {
                        option_nums[qi][oi] as u8
                    } else {
                        0xFF
                    };
                    option_nums[qi][oi] = NAN_VAL;
                }
            }
            if question_types[qi].has_identity_options() {
                for oi in 0..5 {
                    option_answers[qi][oi] = oi as u8;
                    option_nums[qi][oi] = NAN_VAL;
                }
            }
        }
    }

    let (affected_by, global_indices) = FlatPuzzle::build_deps(&question_types, n);
    Some(FlatPuzzle {
        question_types,
        option_nums,
        option_answers,
        option_claims,
        affected_by,
        global_indices,
        n,
        option_count,
    })
}

fn claim_to_json(claim: &Option<Claim>) -> Value {
    match claim {
        None => Value::Null,
        Some(c) => serde_json::to_value(c).unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn puzzle_json_files() -> Vec<std::path::PathBuf> {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let daily_dir = manifest_dir.join("../public/puzzles/daily");
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&daily_dir)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", daily_dir.display()))
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();
        files.sort();
        files
    }

    fn all_puzzles() -> Vec<(String, FlatPuzzle)> {
        let mut puzzles = Vec::new();
        for path in &puzzle_json_files() {
            let filename = path.file_name().unwrap().to_str().unwrap();
            let text = std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
            let data: Value = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("invalid JSON in {}: {e}", path.display()));
            let obj = data.as_object().unwrap();
            for (day, levels) in obj {
                let levels = match levels.as_object() {
                    Some(l) => l,
                    None => continue,
                };
                for (lvl, puzzle) in levels {
                    let key = format!("{filename}/{day}-{lvl}");
                    if let Some(fp) = parse_puzzle(puzzle) {
                        puzzles.push((key, fp));
                    }
                }
            }
        }
        puzzles
    }

    #[test]
    fn generated_puzzles_hint_solvable() {
        assert!(
            cfg!(not(debug_assertions)),
            "too slow in debug mode — run `cargo test --release`"
        );
        let puzzles = all_puzzles();
        assert!(!puzzles.is_empty());
        let mut failures: Vec<String> = Vec::new();

        for (key, fp) in &puzzles {
            let (ok, _steps) = run_check(fp, key);
            if !ok {
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
                eprintln!("  cargo run --release -- --check ./public/puzzles/daily/{file} {key}");
            }
            panic!("{} hint-solve failure(s)", failures.len());
        }
    }

    #[test]
    fn generated_puzzles_unique_solution() {
        assert!(
            cfg!(not(debug_assertions)),
            "too slow in debug mode — run `cargo test --release`"
        );
        let puzzles = all_puzzles();
        assert!(!puzzles.is_empty());
        let mut failures: Vec<String> = Vec::new();

        for (key, fp) in &puzzles {
            let solutions = solve_brute::solve(fp, None, 2);
            if solutions.len() != 1 {
                failures.push(format!("{key}: found {} solutions", solutions.len()));
                continue;
            }

            let sol = &solutions[0];
            let answers: [Option<Answer>; MAX_N] =
                std::array::from_fn(|i| if i < fp.n { Some(sol[i]) } else { None });

            for qi in 0..fp.n {
                if !check_validity::check_question_against_solution(fp, qi, sol[qi], &answers) {
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
}
