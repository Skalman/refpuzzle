#![allow(clippy::needless_range_loop)]

mod build;
mod check;
mod check_answer;
mod check_form;
mod construct;
mod deduce;
mod difficulty;
mod format;
mod lookahead;
mod rng;
mod serialize;
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
    eprintln!("       refpuzzle check <file.json> [MMDD-level] [--json]");
    eprintln!("       refpuzzle format-check  (reads JSON from stdin)");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -o FILE       output file (required, - for stdout)");
    eprintln!("  -m            merge into existing file");
    eprintln!("  -l, --level   generate only this level (1-6)");
    eprintln!("  -a, --attempts  max attempts per seed (default 100)");
    eprintln!("  --stats       show generation statistics");
    eprintln!("  --trace       show trace output");
    eprintln!("  --json        output check results as JSON");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  refpuzzle gen 2051 -o out.json");
    eprintln!("  refpuzzle gen 2051-03 -o out.json -l 4");
    eprintln!("  refpuzzle gen 2051-01..2051-06 -o out.json -m");
    eprintln!("  refpuzzle check puzzles/daily/2051.json");
    eprintln!("  refpuzzle check puzzles/daily/2051.json 0315-4");
    eprintln!("  refpuzzle check puzzles/daily/2051.json --json | refpuzzle format-check");
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
            let mut file = None;
            let mut target = None;
            let mut json_output = false;
            for arg in &args[2..] {
                match arg.as_str() {
                    "--json" => json_output = true,
                    s if file.is_none() => file = Some(s.to_string()),
                    s if target.is_none() => target = Some(s.to_string()),
                    other => {
                        eprintln!("Unknown option: {other}");
                        std::process::exit(1);
                    }
                }
            }
            if file.is_none() {
                eprintln!("Usage: refpuzzle check <file.json> [MMDD-level] [--json]");
                std::process::exit(1);
            }
            check::check_command(file.as_ref().unwrap(), target.as_deref(), json_output);
            return;
        }
        "format-check" => {
            check::format_check_stdin();
            return;
        }
        _ => {
            eprintln!(
                "Unknown subcommand: {}. Use 'gen', 'check', or 'format-check'.",
                args[1]
            );
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
                    .map(|oi| serialize::claim_to_json(&result.fp.option_claims[qi][oi]))
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
                    if let Some(fp) = serialize::parse_puzzle(puzzle) {
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
            let cr = check::run_check(fp, key);
            let ok = cr.ok;
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
                eprintln!("  cargo run --release -- check ./public/puzzles/daily/{file} {key}");
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
                if !check_answer::check_answer(
                    fp,
                    State {
                        answers,
                        eliminated: [0u8; MAX_N],
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
}
