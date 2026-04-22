mod assemble;
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
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use types::*;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut level: usize = 5;
    let mut count: usize = 10;
    let mut seed: Option<u32> = None;
    let mut max_attempts: usize = 100;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--level" | "-l" => {
                i += 1;
                level = args[i].parse().expect("invalid level");
            }
            "--count" | "-n" => {
                i += 1;
                count = args[i].parse().expect("invalid count");
            }
            "--seed" | "-s" => {
                i += 1;
                seed = Some(args[i].parse().expect("invalid seed"));
            }
            "--attempts" | "-a" => {
                i += 1;
                max_attempts = args[i].parse().expect("invalid attempts");
            }
            "--compare" => {
                // parse remaining args then jump to compare
                let prof = &PROFILES[level - 1];
                let mut results = Vec::new();
                let mut pass = 0u32;
                for s in 0..200u32 {
                    let mut rng = Rng::new(s);
                    let ok = assemble::generate(prof, &mut rng, 500).is_some();
                    results.push(if ok { 1 } else { 0 });
                    if ok {
                        pass += 1;
                    }
                }
                eprintln!("Rust pass={pass} fail={}", 200 - pass);
                println!("{:?}", results);
                return;
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: logiquiz-gen [--level 1-5] [--count N] [--seed S] [--attempts A]"
                );
                return;
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let seed = seed.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u32
    });

    let profile = &PROFILES[level - 1];

    eprintln!(
        "Generating {} level-{} ({}) puzzles with seed {}...",
        count, level, profile.name, seed
    );

    let start = Instant::now();

    let mut master_rng = Rng::new(seed);
    let puzzle_seeds: Vec<Vec<u32>> = (0..count)
        .map(|_| (0..100).map(|_| master_rng.next_u32()).collect())
        .collect();

    use rayon::prelude::*;
    let results: Vec<(usize, Option<GenerateResult>)> = puzzle_seeds
        .par_iter()
        .enumerate()
        .map(|(idx, seeds)| {
            for &s in seeds {
                let mut rng = Rng::new(s);
                if let Some(r) = construct::generate(profile, &mut rng, max_attempts) {
                    return (idx, Some(r));
                }
            }
            (idx, None)
        })
        .collect();

    let mut json_results: Vec<Value> = Vec::new();
    for (idx, result) in &results {
        match result {
            Some(r) => {
                json_results.push(result_to_json(r, profile, *idx));
            }
            None => {
                eprintln!("  Puzzle {}/{} FAILED", idx + 1, count);
            }
        }
    }
    let results = json_results;

    let total = start.elapsed();
    eprintln!(
        "Done: {}/{} puzzles in {:.3}s ({:.1}ms avg)",
        results.len(),
        count,
        total.as_secs_f64(),
        total.as_secs_f64() * 1000.0 / count as f64,
    );

    println!("{}", serde_json::to_string_pretty(&results).unwrap());
}

fn result_to_json(
    result: &GenerateResult,
    profile: &difficulty::DifficultyProfile,
    idx: usize,
) -> Value {
    let n = result.n;
    let questions: Vec<Value> = (0..n)
        .map(|qi| {
            let rule = &result.rules[qi];
            let options: Vec<Value> = (0..5)
                .map(|oi| {
                    let label = option_label_str(rule, qi, oi, &result.fp);
                    if let Rule::TrueStmt = rule {
                        let claim = &result.fp.option_claims[qi][oi];
                        json!({
                            "label": label,
                            "claim": claim_to_json(claim)
                        })
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

    let solution: Vec<String> = (0..n)
        .map(|i| result.solution[i].as_char().to_string())
        .collect();

    json!({
        "puzzle": {
            "id": format!("level-{}-{}", profile.level, idx + 1),
            "title": format!("{} #{}", profile.name, idx + 1),
            "difficulty": profile.level,
            "questions": questions
        },
        "solution": solution
    })
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
                format!("{} and {}", v + 1, v + 2)
            }
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
        } => {
            format!(
                "How many questions before #{} have answer {}?",
                before_index + 1,
                answer.as_char()
            )
        }
        Rule::CountAnswerAfter {
            answer,
            after_index,
        } => {
            format!(
                "How many questions after #{} have answer {}?",
                after_index + 1,
                answer.as_char()
            )
        }
        Rule::CountVowel => "How many questions have a vowel as the answer?".into(),
        Rule::CountConsonant => "How many questions have a consonant as the answer?".into(),
        Rule::MostCommonCount => "How many times does the most common answer occur?".into(),
        Rule::ClosestAfter {
            after_index,
            answer,
        } => {
            format!(
                "Which is the closest question after #{} that has answer {}?",
                after_index + 1,
                answer.as_char()
            )
        }
        Rule::ClosestBefore {
            before_index,
            answer,
        } => {
            format!(
                "Which is the closest question before #{} that has answer {}?",
                before_index + 1,
                answer.as_char()
            )
        }
        Rule::FirstWith { answer } => {
            format!(
                "Which is the first question with answer {}?",
                answer.as_char()
            )
        }
        Rule::LastWith { answer } => {
            format!(
                "Which is the last question with answer {}?",
                answer.as_char()
            )
        }
        Rule::PrevSame => {
            "Which is the previous question that has the same answer as this one?".into()
        }
        Rule::NextSame => "Which is the next question that has the same answer as this one?".into(),
        Rule::OnlySame => "The only other question with the same answer as this one is?".into(),
        Rule::SameAs => "The answer to this question is the same as the answer to question?".into(),
        Rule::OnlyOdd { answer } => {
            format!(
                "The only odd-numbered question with answer {} is?",
                answer.as_char()
            )
        }
        Rule::ConsecIdent => {
            "The only two consecutive questions with identical answers are?".into()
        }
        Rule::AnswerOf { question_index } => {
            format!("What is the answer to question #{}?", question_index + 1)
        }
        Rule::LeastCommon => "Which is the least common answer?".into(),
        Rule::MostCommon => "Which is the most common answer?".into(),
        Rule::Unique => "The answer that is not the answer to any other question is?".into(),
        Rule::EqualCount { answer } => {
            format!(
                "The number of questions with answer {} equals the number of questions with answer?",
                answer.as_char()
            )
        }
        Rule::AnswerIsSelf => "What is the answer to this question?".into(),
        Rule::LetterDist {
            other_question_index,
        } => {
            format!(
                "How many letters away is the answer to this question from the answer to question #{}?",
                other_question_index + 1
            )
        }
        Rule::TrueStmt => "Which statement is the only true statement?".into(),
    }
}

fn claim_label_str(claim: &Claim) -> String {
    match *claim {
        Claim::None => String::new(),
        Claim::CountAnswerEquals { answer, value } => {
            format!(
                "How many questions have answer {}? {}",
                answer.as_char(),
                value
            )
        }
        Claim::CountConsonantEquals { value } => {
            format!(
                "How many questions have a consonant as the answer? {}",
                value
            )
        }
        Claim::CountVowelEquals { value } => {
            format!("How many questions have a vowel as the answer? {}", value)
        }
        Claim::CountAnswerAfterEquals {
            answer,
            after_index,
            value,
        } => {
            format!(
                "How many questions after #{} have answer {}? {}",
                after_index + 1,
                answer.as_char(),
                value
            )
        }
        Claim::CountAnswerBeforeEquals {
            answer,
            before_index,
            value,
        } => {
            format!(
                "How many questions before #{} have answer {}? {}",
                before_index + 1,
                answer.as_char(),
                value
            )
        }
    }
}

fn rule_to_json(rule: &Rule) -> Value {
    match *rule {
        Rule::CountAnswer { answer } => json!({
            "type": "count_answer",
            "answer": answer.as_char().to_string()
        }),
        Rule::CountAnswerBefore {
            answer,
            before_index,
        } => json!({
            "type": "count_answer_before",
            "answer": answer.as_char().to_string(),
            "beforeIndex": before_index
        }),
        Rule::CountAnswerAfter {
            answer,
            after_index,
        } => json!({
            "type": "count_answer_after",
            "answer": answer.as_char().to_string(),
            "afterIndex": after_index
        }),
        Rule::CountVowel => json!({ "type": "count_vowel_answers" }),
        Rule::CountConsonant => json!({ "type": "count_consonant_answers" }),
        Rule::MostCommonCount => json!({ "type": "most_common_count" }),
        Rule::ClosestAfter {
            after_index,
            answer,
        } => json!({
            "type": "closest_after",
            "afterIndex": after_index,
            "answer": answer.as_char().to_string()
        }),
        Rule::ClosestBefore {
            before_index,
            answer,
        } => json!({
            "type": "closest_before",
            "beforeIndex": before_index,
            "answer": answer.as_char().to_string()
        }),
        Rule::FirstWith { answer } => json!({
            "type": "first_with_answer",
            "answer": answer.as_char().to_string()
        }),
        Rule::LastWith { answer } => json!({
            "type": "last_with_answer",
            "answer": answer.as_char().to_string()
        }),
        Rule::PrevSame => json!({ "type": "previous_same_answer" }),
        Rule::NextSame => json!({ "type": "next_same_answer" }),
        Rule::OnlySame => json!({ "type": "only_same_answer" }),
        Rule::SameAs => json!({ "type": "same_answer_as" }),
        Rule::OnlyOdd { answer } => json!({
            "type": "only_odd_with_answer",
            "answer": answer.as_char().to_string()
        }),
        Rule::ConsecIdent => json!({ "type": "consecutive_identical" }),
        Rule::AnswerOf { question_index } => json!({
            "type": "answer_of_question",
            "questionIndex": question_index
        }),
        Rule::LeastCommon => json!({ "type": "least_common_answer" }),
        Rule::MostCommon => json!({ "type": "most_common_answer" }),
        Rule::Unique => json!({ "type": "unique_answer" }),
        Rule::EqualCount { answer } => json!({
            "type": "equal_count_as",
            "answer": answer.as_char().to_string()
        }),
        Rule::AnswerIsSelf => json!({ "type": "answer_is_self" }),
        Rule::LetterDist {
            other_question_index,
        } => json!({
            "type": "letter_distance",
            "otherQuestionIndex": other_question_index
        }),
        Rule::TrueStmt => json!({ "type": "only_true_statement" }),
    }
}

fn claim_to_json(claim: &Claim) -> Value {
    match *claim {
        Claim::None => Value::Null,
        Claim::CountAnswerEquals { answer, value } => json!({
            "type": "count_answer_equals",
            "answer": answer.as_char().to_string(),
            "value": value
        }),
        Claim::CountConsonantEquals { value } => json!({
            "type": "count_consonant_answers_equals",
            "value": value
        }),
        Claim::CountVowelEquals { value } => json!({
            "type": "count_vowel_answers_equals",
            "value": value
        }),
        Claim::CountAnswerAfterEquals {
            answer,
            after_index,
            value,
        } => json!({
            "type": "count_answer_after_equals",
            "answer": answer.as_char().to_string(),
            "afterIndex": after_index,
            "value": value
        }),
        Claim::CountAnswerBeforeEquals {
            answer,
            before_index,
            value,
        } => json!({
            "type": "count_answer_before_equals",
            "answer": answer.as_char().to_string(),
            "beforeIndex": before_index,
            "value": value
        }),
    }
}
