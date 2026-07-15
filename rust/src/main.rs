#![allow(clippy::needless_range_loop)]

mod check_answer;
mod check_form;
mod check_well_posed;
mod cli;
mod construct;
mod deduce;
mod difficulty;
mod fill;
// Consumers (wasm exposure, flip of the TS explain.ts) arrive in later increments.
#[allow(dead_code)]
mod explain;
mod format;
mod lookahead;
// Consumers (wasm exposure, explain) arrive in later migration increments.
#[allow(dead_code)]
mod render;
mod rng;
mod serialize;
mod solve_brute;
#[allow(dead_code)]
mod solve_deduce;
mod stats;
#[cfg(test)]
mod test_symmetry;
#[cfg(test)]
mod test_util;
mod time;
mod types;

use construct::GenerateResult;
use difficulty::PROFILES;
use rng::Rng;
use serde_json::Value;
use std::time::Instant;
#[cfg(test)]
use types::*;

// RefPuzzle's public launch date. No puzzles exist before it, so a 2026 range
// with no explicit start defaults here, and an explicit pre-launch start in 2026
// is clamped up to it; any other pre-launch start is rejected outright.
const LAUNCH_YEAR: u32 = 2026;
const LAUNCH_MM: u32 = 4;
const LAUNCH_DD: u32 = 19;
const LAUNCH_YYYYMMDD: u32 = LAUNCH_YEAR * 10000 + LAUNCH_MM * 100 + LAUNCH_DD;

struct DateRange {
    year: u32,
    start_mm: u32,
    start_dd: u32,
    end_mm: u32,
    end_dd: u32,
}

/// Days in a Gregorian month. Only ever called with a validated month (1..=12);
/// invalid months yield 0 rather than a guess.
fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
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

    // Reject impossible calendar dates up front. Left unchecked, a bad month/day
    // collapses to an empty day set, which divides by zero in the summary and
    // writes `{}` over the output file. Only explicitly-supplied parts can be out
    // of range; the defaults below are in range by construction.
    let validate = |y: u32, m: Option<u32>, d: Option<u32>| {
        if let Some(m) = m {
            if !(1..=12).contains(&m) {
                eprintln!("Invalid month in date range {input}: {m}");
                std::process::exit(1);
            }
            if let Some(d) = d
                && (d == 0 || d > days_in_month(y, m))
            {
                eprintln!("Invalid day in date range {input}: {y}-{m:02}-{d:02}");
                std::process::exit(1);
            }
        }
    };

    let (sy, sm, sd) = parse_part(start_str);
    validate(sy, sm, sd);
    let start_mm = sm.unwrap_or(if sy == LAUNCH_YEAR { LAUNCH_MM } else { 1 });
    let start_dd = sd.unwrap_or(if sy == LAUNCH_YEAR && sm.is_none() {
        LAUNCH_DD
    } else {
        1
    });

    let (ey, em, ed) = if let Some(e) = end_str {
        parse_part(e)
    } else {
        (sy, sm, sd)
    };
    validate(ey, em, ed);

    if sy != ey {
        eprintln!("Date range must not cross year boundaries: {input}");
        std::process::exit(1);
    }

    let end_mm = em.unwrap_or(12);
    let end_dd = ed.unwrap_or_else(|| days_in_month(ey, end_mm));

    let mut start_mm = start_mm;
    let mut start_dd = start_dd;
    let start_val = sy * 10000 + start_mm * 100 + start_dd;
    if sy == LAUNCH_YEAR && start_val < LAUNCH_YYYYMMDD {
        start_mm = LAUNCH_MM;
        start_dd = LAUNCH_DD;
    }
    let start_val = sy * 10000 + start_mm * 100 + start_dd;
    if start_val < LAUNCH_YYYYMMDD {
        eprintln!(
            "Date range must not start before {LAUNCH_YEAR}-{LAUNCH_MM:02}-{LAUNCH_DD:02}: {input}"
        );
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
    eprintln!("       refpuzzle check -   (reads a year map or single puzzle from stdin)");
    eprintln!("       refpuzzle format-check  (reads JSON from stdin)");
    eprintln!("       refpuzzle type-stats -o FILE [--attempts N] [--seed S]");
    eprintln!(
        "       refpuzzle gen-stats [-a N] [-n N] [-l 1-6] [--seed S] [--origin URL]   (gen quality: histogram + links)"
    );
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -o FILE       output file (required, - for stdout)");
    eprintln!("  -m, --merge   merge into existing file");
    eprintln!("  --overwrite   overwrite existing output file");
    eprintln!("  -l, --level   generate only this level (1-6)");
    eprintln!("  -a, --attempts  regeneration budget per puzzle (default 100)");
    eprintln!("  -t, --threads N  worker threads (default: all cores)");
    eprintln!("  --stats       show generation statistics");
    eprintln!("  --json        output check results as JSON");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  refpuzzle gen 2051 -o out.json");
    eprintln!("  refpuzzle gen 2051-03 -o out.json -l 4");
    eprintln!("  refpuzzle gen 2051-01..2051-06 -o out.json -m");
    eprintln!("  refpuzzle check puzzles/daily/2051.json");
    eprintln!("  refpuzzle check puzzles/daily/2051.json 0315-4");
    eprintln!("  refpuzzle check puzzles/daily/2051.json --json | refpuzzle format-check");
    eprintln!("  echo '{{\"o\":[...],\"q\":[...]}}' | refpuzzle check -");
}

/// Consume the value after a value-taking flag `name` (currently at `args[*i]`),
/// advancing `*i` onto it. Exits with a message on the two user errors the bare
/// `i += 1; args[i]` mishandled: a trailing flag (which index-panicked) and a
/// value that is really the next flag (e.g. `-o -l`). `-` passes through as the
/// stdout sentinel for `-o`/`--output`.
fn flag_value<'a>(args: &'a [String], i: &mut usize, name: &str) -> &'a str {
    *i += 1;
    let Some(value) = args.get(*i) else {
        eprintln!("Error: {name} requires a value");
        std::process::exit(1);
    };
    if value.starts_with('-') && value != "-" {
        eprintln!("Error: {name} requires a value, but found flag '{value}'");
        std::process::exit(1);
    }
    value
}

/// [`flag_value`] plus a parse to `T`, exiting cleanly (not panicking) when the
/// value is malformed.
fn flag_parse<T: std::str::FromStr>(args: &[String], i: &mut usize, name: &str) -> T {
    let value = flag_value(args, i, name);
    value.parse().unwrap_or_else(|_| {
        eprintln!("Error: {name} expects a valid value, got '{value}'");
        std::process::exit(1);
    })
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
                    // Reject unknown flags rather than swallowing them as the
                    // file path / target.
                    s if s.starts_with('-') && s != "-" => {
                        eprintln!("Unknown option: {s}");
                        std::process::exit(1);
                    }
                    s if file.is_none() => file = Some(s.to_string()),
                    s if target.is_none() => target = Some(s.to_string()),
                    other => {
                        eprintln!("Unexpected argument: {other}");
                        std::process::exit(1);
                    }
                }
            }
            if file.is_none() {
                eprintln!("Usage: refpuzzle check <file.json> [MMDD-level] [--json]");
                std::process::exit(1);
            }
            cli::check::check_command(file.as_ref().unwrap(), target.as_deref(), json_output);
            return;
        }
        "format-check" => {
            cli::check::format_check_stdin();
            return;
        }
        "type-stats" => {
            let mut attempts: u32 = 10000;
            let mut seed: u32 = 1;
            let mut output: Option<String> = None;
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--attempts" | "-a" => attempts = flag_parse(&args, &mut i, "--attempts"),
                    "--seed" => seed = flag_parse(&args, &mut i, "--seed"),
                    "--output" | "-o" => {
                        output = Some(flag_value(&args, &mut i, "--output").to_string())
                    }
                    other => {
                        eprintln!("Unknown option: {other}");
                        std::process::exit(1);
                    }
                }
                i += 1;
            }
            let Some(output) = output else {
                eprintln!("Usage: refpuzzle type-stats -o FILE [--attempts N] [--seed N]");
                eprintln!("  -o FILE   output file (required, - for stdout)");
                std::process::exit(1);
            };
            cli::type_stats::type_stats(attempts, seed, &output);
            return;
        }
        // generation quality diagnostic (see src/cli/diagnose.rs).
        "gen-stats" => {
            let mut seed: u32 = 1;
            let mut attempts: usize = 200;
            let mut count: usize = 5;
            let mut level: Option<usize> = None;
            let mut origin = "http://localhost:5173".to_string();
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--seed" => seed = flag_parse(&args, &mut i, "--seed"),
                    "--attempts" | "-a" => attempts = flag_parse(&args, &mut i, "--attempts"),
                    "--count" | "-n" => count = flag_parse(&args, &mut i, "--count"),
                    "--level" | "-l" => {
                        let l: usize = flag_parse(&args, &mut i, "--level");
                        if !(1..=6).contains(&l) {
                            eprintln!("Error: level must be 1-6");
                            std::process::exit(1);
                        }
                        level = Some(l);
                    }
                    "--origin" => origin = flag_value(&args, &mut i, "--origin").to_string(),
                    other => {
                        eprintln!("Unknown option: {other}");
                        std::process::exit(1);
                    }
                }
                i += 1;
            }
            cli::diagnose::gen_stats(seed, attempts, count, level, &origin);
            return;
        }
        _ => {
            eprintln!(
                "Unknown subcommand: {}. Use 'gen', 'check', 'format-check', 'type-stats', or 'gen-stats'.",
                args[1]
            );
            std::process::exit(1);
        }
    }

    // Parse gen subcommand args
    let mut date_range_str: Option<String> = None;
    let mut max_attempts: usize = construct::DEFAULT_MAX_REGENERATIONS;
    let mut level_filter: Option<u8> = None;
    let mut show_stats = false;
    let mut output_path: Option<String> = None;
    let mut merge = false;
    let mut overwrite = false;
    let mut threads: Option<usize> = None;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--attempts" | "-a" => max_attempts = flag_parse(&args, &mut i, "--attempts"),
            "--level" | "-l" => {
                let l: u8 = flag_parse(&args, &mut i, "--level");
                if !(1..=6).contains(&l) {
                    eprintln!("Error: level must be 1-6");
                    std::process::exit(1);
                }
                level_filter = Some(l);
            }
            "--output" | "-o" => {
                output_path = Some(flag_value(&args, &mut i, "--output").to_string())
            }
            "--merge" | "-m" => {
                merge = true;
            }
            "--overwrite" => {
                overwrite = true;
            }
            "--threads" | "-t" => threads = Some(flag_parse(&args, &mut i, "--threads")),
            "--stats" => {
                show_stats = true;
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
                if date_range_str.is_some() {
                    eprintln!(
                        "Error: unexpected extra argument '{other}' (date range already set)"
                    );
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

    if merge && overwrite {
        eprintln!("Error: --merge and --overwrite are mutually exclusive");
        std::process::exit(1);
    }
    if merge && output_path == "-" {
        eprintln!("Error: --merge requires -o FILE (cannot merge to stdout)");
        std::process::exit(1);
    }
    if output_path != "-" && !merge && !overwrite && std::path::Path::new(&output_path).is_file() {
        eprintln!(
            "Error: output file {output_path} already exists. Pass --merge to add to it, or --overwrite to replace it."
        );
        std::process::exit(1);
    }

    if let Some(t) = threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(t)
            .build_global()
            .expect("failed to configure thread pool");
    }

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

    // One seed per task, derived deterministically from (year, mmdd, level) so the
    // same date always produces the same puzzle. The generator retries internally
    // (re-rolling questions against the fixed key), so no per-retry seed is needed.
    let task_seeds: Vec<u32> = tasks
        .iter()
        .map(|&(day_idx, level)| {
            let (mm, dd) = days[day_idx];
            let date_key = year * 10000 + mm * 100 + dd;
            date_key
                .wrapping_mul(31)
                .wrapping_add(level as u32)
                .wrapping_mul(17)
        })
        .collect();

    use rayon::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let done_by_level: [AtomicUsize; 6] = std::array::from_fn(|_| AtomicUsize::new(0));
    let last_report = std::sync::Mutex::new(Instant::now());
    let total = tasks.len();

    let results: Vec<((usize, u8), Option<GenerateResult>, stats::Stats)> = tasks
        .par_iter()
        .zip(task_seeds.par_iter())
        .map(|(&(day_idx, level), &seed)| {
            let (mm, dd) = days[day_idx];
            let label = format!("{mm:02}{dd:02}-{level}");
            let profile = &PROFILES[level as usize - 1];
            let mut stats = stats::Stats::default();
            // The generator fixes the answer key on the first skeleton and only
            // re-rolls the questions, so one seed suffices. A `None` means the key
            // admitted no unique puzzle within the budget, which shouldn't happen;
            // surface it loudly.
            let mut rng = Rng::new(seed);
            let result = Some(
                construct::generate(
                    &construct::RECIPES[level as usize - 1],
                    profile.question_count,
                    profile.option_count,
                    &mut rng,
                    max_attempts,
                    &mut stats,
                    &label,
                )
                .unwrap_or_else(|| {
                    panic!(
                        "{label}: no unique puzzle within {max_attempts} regenerations (seed {seed})",
                    )
                }),
            );
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

    let mut stats = stats::Stats::default();
    for (_, _, s) in &results {
        stats.merge(s);
    }

    // Assemble into { "MMDD": { "1": ..., ..., "6": ... }, ... }
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
                let puzzle_json = puzzle_to_json(r);
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
        "  Time:    {:.2}s ({:.3}ms per day)",
        elapsed.as_secs_f64(),
        elapsed.as_secs_f64() * 1000.0 / day_count as f64
    );
    eprintln!("  Output:  {}", output_path);
    if show_stats {
        stats.print();
    }

    if merge {
        let existing = std::fs::read_to_string(&output_path).unwrap_or_else(|_| "{}".into());
        let mut existing: serde_json::Map<String, Value> =
            serde_json::from_str(&existing).expect("invalid JSON in output file");
        for (date, levels) in year_map {
            let entry = existing
                .entry(date)
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            if let (Value::Object(existing_day), Value::Object(new_levels)) = (entry, levels) {
                for (level_key, puzzle) in new_levels {
                    existing_day.insert(level_key, puzzle);
                }
            }
        }
        let out = format_year(&existing);
        std::fs::write(&output_path, out).expect("failed to write output file");
    } else {
        let out = format_year(&year_map);
        if output_path == "-" {
            print!("{out}");
        } else {
            std::fs::write(&output_path, out).expect("failed to write output file");
        }
    }
}

/// Indent the date and level keys for readable git diffs; keep each puzzle on
/// one compact line. Iteration order is sorted (MMDD dates → chronological).
fn format_year(year: &serde_json::Map<String, Value>) -> String {
    let mut out = String::from("{\n");
    let n = year.len();
    for (i, (date, levels)) in year.iter().enumerate() {
        out.push_str("  ");
        out.push_str(&serde_json::to_string(date).unwrap());
        out.push_str(": {\n");
        let levels_obj = levels.as_object().expect("levels must be object");
        let m = levels_obj.len();
        for (j, (lvl, puzzle)) in levels_obj.iter().enumerate() {
            out.push_str("    ");
            out.push_str(&serde_json::to_string(lvl).unwrap());
            out.push_str(": ");
            out.push_str(&serde_json::to_string(puzzle).unwrap());
            out.push_str(if j + 1 < m { ",\n" } else { "\n" });
        }
        out.push_str("  }");
        out.push_str(if i + 1 < n { ",\n" } else { "\n" });
    }
    out.push_str("}\n");
    out
}

fn puzzle_to_json(result: &GenerateResult) -> Value {
    serialize::puzzle_to_compact_value(&result.question_types, &result.fp)
}

fn dates_in_year(
    year: u32,
    start_mm: u32,
    start_dd: u32,
    end_mm: u32,
    end_dd: u32,
) -> Vec<(u32, u32)> {
    let mut result = Vec::new();
    let mut mm = start_mm;
    let mut dd = start_dd;
    while mm <= 12 {
        while dd <= days_in_month(year, mm) {
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

/// Every shipped daily puzzle as `(label, FlatPuzzle)`, read from the daily dir
/// (one file per year). Shared by the generator fuzz tests and the symmetry sweep.
#[cfg(test)]
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
    use super::*;
    use crate::test_util::slow_test_duration;

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
            let cr = cli::check::run_check(fp, key);
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
                let is_warning = matches!(e.severity, check_form::Severity::Warning);
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
