mod construct;
mod difficulty;
mod evaluate;
mod gen_common;
mod hints;
mod rng;
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
            "--help" | "-h" => {
                eprintln!(
                    "Usage: logiquiz-gen --year YYYY [--start YYYY-MM-DD] [--level 1-5] [--attempts A] [--stats]"
                );
                eprintln!("  Generates a year of daily puzzles.");
                eprintln!(
                    "  Seeds are derived from the date, so the same date always produces the same puzzle."
                );
                eprintln!("  --level  generate only this level (default: all 5)");
                eprintln!("  --start  defaults to YYYY-01-01 (or 2026-04-19 for 2026).");
                eprintln!("  --stats  show generation statistics");
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
                if let Some(r) = construct::generate(profile, &mut rng, max_attempts) {
                    result = Some(r);
                    break;
                }
            }
            done_by_level[level as usize - 1].fetch_add(1, Ordering::Relaxed);
            if let Ok(mut last) = last_report.try_lock() {
                if last.elapsed().as_secs() >= 15 {
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
        "  Time:    {:.1}s ({:.0}ms per day)",
        elapsed.as_secs_f64(),
        elapsed.as_secs_f64() * 1000.0 / day_count as f64
    );
    eprintln!("  Output:  {raw_kb}KB JSON");
    if show_stats {
        gen_common::print_stats();
    }

    println!("{json_out}");
}

fn puzzle_to_json(result: &GenerateResult, level: usize) -> Value {
    let n = result.n;
    let questions: Vec<Value> = (0..n)
        .map(|qi| {
            let rule = &result.rules[qi];
            let options: Vec<Value> = (0..5)
                .map(|oi| {
                    let label = option_label_str(rule, qi, oi, &result.fp);
                    if let Rule::TrueStmt = rule {
                        let claim = &result.fp.option_claims[qi][oi];
                        json!({ "label": label, "claim": claim_to_json(claim) })
                    } else {
                        json!({ "label": label })
                    }
                })
                .collect();
            json!({
                "text": question_text(rule),
                "options": options,
                "rule": rule_to_json(rule)
            })
        })
        .collect();

    json!({
        "difficulty": level,
        "questions": questions
    })
}

fn dates_in_year(year: u32, start_mm: u32, start_dd: u32) -> Vec<(u32, u32)> {
    let days_in_month = |m: u32| -> u32 {
        match m {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
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

fn option_label_str(rule: &Rule, qi: usize, oi: usize, fp: &FlatPuzzle) -> String {
    match *rule {
        Rule::AnswerOf { .. } => {
            let a = fp.option_answers[qi][oi];
            Answer::from_u8(a).map_or("?".into(), |a| a.as_char().to_string())
        }
        Rule::ConsecIdent => {
            let v = fp.option_nums[qi][oi];
            if v == NONE_VAL {
                "None".into()
            } else {
                format!("{} & {}", v + 1, v + 2)
            }
        }
        Rule::LeastCommon | Rule::MostCommon => {
            let a = fp.option_answers[qi][oi];
            Answer::from_u8(a).map_or("?".into(), |a| a.as_char().to_string())
        }
        ref r if r.is_constrained() => LETTERS[oi].as_char().to_string(),
        Rule::TrueStmt => claim_label_str(&fp.option_claims[qi][oi]),
        _ => {
            let v = fp.option_nums[qi][oi];
            if v == NONE_VAL {
                "None".into()
            } else {
                v.to_string()
            }
        }
    }
}

fn question_text(rule: &Rule) -> String {
    match *rule {
        Rule::CountAnswer { answer } => {
            format!("How many questions have answer {}?", answer.as_char())
        }
        Rule::CountAnswerBefore {
            answer,
            before_index,
        } => format!(
            "How many questions before #{} have answer {}?",
            before_index + 1,
            answer.as_char()
        ),
        Rule::CountAnswerAfter {
            answer,
            after_index,
        } => format!(
            "How many questions after #{} have answer {}?",
            after_index + 1,
            answer.as_char()
        ),
        Rule::CountVowel => "How many questions have a vowel as the answer?".into(),
        Rule::CountConsonant => "How many questions have a consonant as the answer?".into(),
        Rule::MostCommonCount => "How many times does the most common answer occur?".into(),
        Rule::ClosestAfter {
            after_index,
            answer,
        } => format!(
            "Which is the closest question after #{} that has answer {}?",
            after_index + 1,
            answer.as_char()
        ),
        Rule::ClosestBefore {
            before_index,
            answer,
        } => format!(
            "Which is the closest question before #{} that has answer {}?",
            before_index + 1,
            answer.as_char()
        ),
        Rule::FirstWith { answer } => format!(
            "Which is the first question with answer {}?",
            answer.as_char()
        ),
        Rule::LastWith { answer } => format!(
            "Which is the last question with answer {}?",
            answer.as_char()
        ),
        Rule::PrevSame => {
            "Which is the previous question that has the same answer as this one?".into()
        }
        Rule::NextSame => "Which is the next question that has the same answer as this one?".into(),
        Rule::OnlySame => {
            "Which is the only other question with the same answer as this one?".into()
        }
        Rule::SameAs => "Which question has the same answer as this one?".into(),
        Rule::OnlyOdd { answer } => format!(
            "Which is the only odd-numbered question with answer {}?",
            answer.as_char()
        ),
        Rule::ConsecIdent => {
            "Which are the only two consecutive questions with identical answers?".into()
        }
        Rule::AnswerOf { question_index } => {
            format!("What is the answer to question #{}?", question_index + 1)
        }
        Rule::LeastCommon => "Which is the least common answer?".into(),
        Rule::MostCommon => "Which is the most common answer?".into(),
        Rule::Unique => "Which answer is not the answer to any other question?".into(),
        Rule::EqualCount { answer } => format!(
            "The number of questions with answer {} equals the number of questions with answer?",
            answer.as_char()
        ),
        Rule::AnswerIsSelf => "What is the answer to this question?".into(),
        Rule::LetterDist {
            other_question_index,
        } => format!(
            "How many letters away is the answer to this question from the answer to question #{}?",
            other_question_index + 1
        ),
        Rule::TrueStmt => "Which statement is the only true statement?".into(),
    }
}

fn claim_label_str(claim: &Claim) -> String {
    match *claim {
        Claim::None => String::new(),
        Claim::CountAnswerEquals { answer, value } => format!(
            "How many questions have answer {}? {}",
            answer.as_char(),
            value
        ),
        Claim::CountConsonantEquals { value } => format!(
            "How many questions have a consonant as the answer? {}",
            value
        ),
        Claim::CountVowelEquals { value } => {
            format!("How many questions have a vowel as the answer? {}", value)
        }
        Claim::CountAnswerAfterEquals {
            answer,
            after_index,
            value,
        } => format!(
            "How many questions after #{} have answer {}? {}",
            after_index + 1,
            answer.as_char(),
            value
        ),
        Claim::CountAnswerBeforeEquals {
            answer,
            before_index,
            value,
        } => format!(
            "How many questions before #{} have answer {}? {}",
            before_index + 1,
            answer.as_char(),
            value
        ),
    }
}

fn rule_to_json(rule: &Rule) -> Value {
    match *rule {
        Rule::CountAnswer { answer } => {
            json!({"type": "count_answer", "answer": answer.as_char().to_string()})
        }
        Rule::CountAnswerBefore {
            answer,
            before_index,
        } => {
            json!({"type": "count_answer_before", "answer": answer.as_char().to_string(), "beforeIndex": before_index})
        }
        Rule::CountAnswerAfter {
            answer,
            after_index,
        } => {
            json!({"type": "count_answer_after", "answer": answer.as_char().to_string(), "afterIndex": after_index})
        }
        Rule::CountVowel => json!({"type": "count_vowel_answers"}),
        Rule::CountConsonant => json!({"type": "count_consonant_answers"}),
        Rule::MostCommonCount => json!({"type": "most_common_count"}),
        Rule::ClosestAfter {
            after_index,
            answer,
        } => {
            json!({"type": "closest_after", "afterIndex": after_index, "answer": answer.as_char().to_string()})
        }
        Rule::ClosestBefore {
            before_index,
            answer,
        } => {
            json!({"type": "closest_before", "beforeIndex": before_index, "answer": answer.as_char().to_string()})
        }
        Rule::FirstWith { answer } => {
            json!({"type": "first_with_answer", "answer": answer.as_char().to_string()})
        }
        Rule::LastWith { answer } => {
            json!({"type": "last_with_answer", "answer": answer.as_char().to_string()})
        }
        Rule::PrevSame => json!({"type": "previous_same_answer"}),
        Rule::NextSame => json!({"type": "next_same_answer"}),
        Rule::OnlySame => json!({"type": "only_same_answer"}),
        Rule::SameAs => json!({"type": "same_answer_as"}),
        Rule::OnlyOdd { answer } => {
            json!({"type": "only_odd_with_answer", "answer": answer.as_char().to_string()})
        }
        Rule::ConsecIdent => json!({"type": "consecutive_identical"}),
        Rule::AnswerOf { question_index } => {
            json!({"type": "answer_of_question", "questionIndex": question_index})
        }
        Rule::LeastCommon => json!({"type": "least_common_answer"}),
        Rule::MostCommon => json!({"type": "most_common_answer"}),
        Rule::Unique => json!({"type": "unique_answer"}),
        Rule::EqualCount { answer } => {
            json!({"type": "equal_count_as", "answer": answer.as_char().to_string()})
        }
        Rule::AnswerIsSelf => json!({"type": "answer_is_self"}),
        Rule::LetterDist {
            other_question_index,
        } => json!({"type": "letter_distance", "otherQuestionIndex": other_question_index}),
        Rule::TrueStmt => json!({"type": "only_true_statement"}),
    }
}

fn claim_to_json(claim: &Claim) -> Value {
    match *claim {
        Claim::None => Value::Null,
        Claim::CountAnswerEquals { answer, value } => {
            json!({"type": "count_answer_equals", "answer": answer.as_char().to_string(), "value": value})
        }
        Claim::CountConsonantEquals { value } => {
            json!({"type": "count_consonant_answers_equals", "value": value})
        }
        Claim::CountVowelEquals { value } => {
            json!({"type": "count_vowel_answers_equals", "value": value})
        }
        Claim::CountAnswerAfterEquals {
            answer,
            after_index,
            value,
        } => {
            json!({"type": "count_answer_after_equals", "answer": answer.as_char().to_string(), "afterIndex": after_index, "value": value})
        }
        Claim::CountAnswerBeforeEquals {
            answer,
            before_index,
            value,
        } => {
            json!({"type": "count_answer_before_equals", "answer": answer.as_char().to_string(), "beforeIndex": before_index, "value": value})
        }
    }
}
