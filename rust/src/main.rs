#![allow(clippy::needless_range_loop)]

mod check_validity;
mod construct_puzzle;
mod deduce;
mod difficulty;
mod evaluate;
mod gen_common;
mod lookahead;
mod rng;
#[allow(dead_code)]
mod solve;
mod solver;
mod types;

use difficulty::PROFILES;
use gen_common::GenerateResult;
use rng::Rng;
use serde_json::{Value, json};
use std::time::Instant;
use types::*;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut year: Option<u32> = None;
    let mut start_date: Option<String> = None;
    let mut max_attempts: usize = 100;
    let mut level_filter: Option<u8> = None;
    let mut show_stats = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--year" | "-y" => {
                i += 1;
                year = Some(args[i].parse().expect("invalid year"));
            }
            "--start" => {
                i += 1;
                start_date = Some(args[i].clone());
            }
            "--attempts" | "-a" => {
                i += 1;
                max_attempts = args[i].parse().expect("invalid attempts");
            }
            "--level" | "-l" => {
                i += 1;
                let l: u8 = args[i].parse().expect("invalid level");
                assert!((1..=5).contains(&l), "level must be 1-5");
                level_filter = Some(l);
            }
            "--stats" => {
                show_stats = true;
            }
            "--check" => {
                i += 1;
                let file = &args[i];
                let target = args.get(i + 1).cloned();
                check_json(file, target.as_deref());
                return;
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: logiquiz-gen --year YYYY [--start YYYY-MM-DD] [--level 1-5] [--attempts A] [--stats]"
                );
                eprintln!("       logiquiz-gen --check <file.json> [MMDD-level-N]");
                eprintln!("  Generates a year of daily puzzles.");
                eprintln!(
                    "  Seeds are derived from the date, so the same date always produces the same puzzle."
                );
                eprintln!("  --level  generate only this level (default: all 5)");
                eprintln!("  --start  defaults to YYYY-01-01 (or 2026-04-19 for 2026).");
                eprintln!("  --stats  show generation statistics");
                eprintln!("  --check  verify solvability of puzzles in a JSON file");
                return;
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let year = year.unwrap_or(2026);
    let start = start_date.unwrap_or_else(|| {
        if year == 2026 {
            "2026-04-19".into()
        } else {
            format!("{year}-01-01")
        }
    });

    let start_mm: u32 = start[5..7].parse().unwrap();
    let start_dd: u32 = start[8..10].parse().unwrap();

    let days = dates_in_year(year, start_mm, start_dd);
    let day_count = days.len();

    eprintln!(
        "Generating {} days for year {} (start={})...",
        day_count, year, start,
    );

    let start_time = Instant::now();
    let levels: Vec<u8> = match level_filter {
        Some(l) => vec![l],
        None => vec![1, 2, 3, 4, 5],
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
    let done_by_level: [AtomicUsize; 5] = std::array::from_fn(|_| AtomicUsize::new(0));
    let last_report = std::sync::Mutex::new(Instant::now());
    let total = tasks.len();

    let results: Vec<((usize, u8), Option<GenerateResult>)> = tasks
        .par_iter()
        .zip(task_seeds.par_iter())
        .map(|(&(day_idx, level), seeds)| {
            let profile = &PROFILES[level as usize - 1];
            let mut result = None;
            for &s in seeds {
                let mut rng = Rng::new(s);
                if let Some(r) = construct_puzzle::generate(profile, &mut rng, max_attempts) {
                    result = Some(r);
                    break;
                }
            }
            done_by_level[level as usize - 1].fetch_add(1, Ordering::Relaxed);
            if let Ok(mut last) = last_report.try_lock()
                && last.elapsed().as_secs() >= 15
            {
                let counts: Vec<usize> = (0..5)
                    .map(|i| done_by_level[i].load(Ordering::Relaxed))
                    .collect();
                let done_total: usize = counts.iter().sum();
                eprintln!(
                    "  {done_total}/{total}: L1={} L2={} L3={} L4={} L5={}",
                    counts[0], counts[1], counts[2], counts[3], counts[4],
                );
                *last = Instant::now();
            }
            ((day_idx, level), result)
        })
        .collect();

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

    for ((day_idx, level), result) in &results {
        let (mm, dd) = days[*day_idx];
        let key = format!("{mm:02}{dd:02}");
        match result {
            Some(r) => {
                ok_count += 1;
                let puzzle_json = puzzle_to_json(r, *level as usize);
                if let Some(Value::Object(day)) = year_map.get_mut(&key) {
                    day.insert(format!("level-{level}"), puzzle_json);
                }
            }
            None => {
                fail_count += 1;
                eprintln!("  FAILED: {} level-{}", key, level);
            }
        }
    }

    let elapsed = start_time.elapsed();
    let json_out = serde_json::to_string(&Value::Object(year_map)).unwrap();
    let raw_kb = json_out.len() / 1024;

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
    eprintln!("  Output:  {raw_kb}KB JSON");
    if show_stats {
        gen_common::print_stats();
        gen_common::print_extra_stats();
    }

    println!("{json_out}");
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
    // Constrained types always have A-E (0-4)
    if qt.is_constrained() {
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
    let questions: Vec<Value> = (0..n)
        .map(|qi| {
            let qt = &result.question_types[qi];
            let mut q = serde_json::Map::new();
            if let QuestionType::TrueStmt = qt {
                let claims: Vec<Value> = (0..5)
                    .map(|oi| claim_to_json(&result.fp.option_claims[qi][oi]))
                    .collect();
                q.insert("c".into(), json!(claims));
            } else {
                let options: Vec<Value> = (0..5)
                    .map(|oi| option_value_json(qt, qi, oi, &result.fp))
                    .collect();
                q.insert("o".into(), json!(options));
            }
            q.insert("r".into(), question_type_to_json(qt));
            Value::Object(q)
        })
        .collect();

    json!({ "q": questions })
}

fn dates_in_year(year: u32, start_mm: u32, start_dd: u32) -> Vec<(u32, u32)> {
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
            if let Some(t) = target {
                if key != t {
                    continue;
                }
            }
            let fp = match parse_puzzle(puzzle) {
                Some(fp) => fp,
                None => {
                    eprintln!("  SKIP {key}: parse failed");
                    continue;
                }
            };
            total += 1;
            let (ok, steps) = run_check(&fp);
            if ok {
                solved += 1;
            } else {
                let answered = steps
                    .iter()
                    .filter(|s| s.chars().last().map_or(false, |c| c.is_uppercase()))
                    .count();
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
                let level = lvl.strip_prefix("level-").unwrap_or(lvl);
                let hash = steps.join(".");
                let url =
                    format!("http://localhost:5173/day/{year}-{mm}-{dd}?l={level}&debug#{hash}");
                failures.push(format!("{key}: {answered}/{} — {url}", fp.n));
            }
            if target.is_some() {
                println!("{}", steps.join("."));
                let solutions = solver::solve(&fp, None, 10);
                eprintln!("Solutions found: {}", solutions.len());
                for (i, sol) in solutions.iter().enumerate() {
                    let s: String = sol.iter().take(fp.n).map(|a| a.as_char()).collect();
                    eprintln!("  #{}: {}", i + 1, s);
                }
                if !ok {
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

fn run_check(fp: &FlatPuzzle) -> (bool, Vec<String>) {
    let n = fp.n;
    let mut answers: [Option<Answer>; MAX_N] = [None; MAX_N];
    let mut eliminated = [0u8; MAX_N];
    let mut steps = Vec::new();
    let letters_lower = ['a', 'b', 'c', 'd', 'e'];

    for _ in 0..n * 30 {
        if (0..n).all(|i| answers[i].is_some()) {
            return (true, steps);
        }
        let drs = deduce::deduce(fp, &answers, &eliminated);
        if !drs.is_empty() {
            for dr in &drs {
                match dr.action {
                    deduce::DeduceAction::Force { qi, answer } => {
                        if let Some(existing) = answers[qi] {
                            if existing != answer {
                                eprintln!(
                                    "CONFLICT: Q{} forced {} but already {}",
                                    qi + 1,
                                    answer.as_char(),
                                    existing.as_char()
                                );
                            }
                        }
                        eliminated[qi] = 0b11111 ^ (1 << answer.idx());
                        answers[qi] = Some(answer);
                        steps.push(format!("{}{}", qi + 1, answer.as_char()));
                    }
                    deduce::DeduceAction::Eliminate { qi, oi } => {
                        if answers[qi] == Some(LETTERS[oi]) {
                            eprintln!(
                                "CONFLICT: Q{} eliminating {} but already forced to it (rule: {:?})",
                                qi + 1,
                                LETTERS[oi].as_char(),
                                dr.rule
                            );
                        }
                        eliminated[qi] |= 1 << oi;
                        steps.push(format!("{}{}", qi + 1, letters_lower[oi]));
                    }
                    deduce::DeduceAction::EliminateMulti {
                        question_mask,
                        option_mask,
                    } => {
                        for i in 0..n {
                            if (question_mask >> i) & 1 == 1 {
                                eliminated[i] |= option_mask;
                                for oi in 0..5usize {
                                    if (option_mask >> oi) & 1 == 1 {
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
        if let Some(lr) = lookahead::lookahead(fp, &answers, &eliminated, usize::MAX) {
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

    let mut question_types = [QuestionType::AnswerIsSelf; MAX_N];
    let mut option_nums = [[NAN_VAL; 5]; MAX_N];
    let mut option_answers = [[0xFFu8; 5]; MAX_N];
    let mut option_claims = [[Claim::None; 5]; MAX_N];

    for (qi, q) in qs.iter().enumerate() {
        let r = q.get("r")?;
        question_types[qi] = serde_json::from_value(r.clone()).ok()?;

        if let Some(claims) = q.get("c") {
            let claims = claims.as_array()?;
            for (oi, c) in claims.iter().enumerate() {
                if c.is_null() {
                    continue;
                }
                option_claims[qi][oi] = serde_json::from_value(c.clone()).ok()?;
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
            if question_types[qi].is_constrained() {
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
    })
}

fn claim_to_json(claim: &Claim) -> Value {
    match claim {
        Claim::None => Value::Null,
        _ => serde_json::to_value(claim).unwrap(),
    }
}
