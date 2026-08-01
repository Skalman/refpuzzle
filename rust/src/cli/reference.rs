//! `refpuzzle reference`: a living reference of every question type and deduce
//! rule, each with a real rendered example collected by solving the daily corpus
//! under the full engine. Examples stay accurate as prose/rules change, and any
//! kind or rule never seen in the corpus is called out (a coverage signal).
//! Bin-only.

use std::collections::BTreeMap;

use crate::deduce::{ALL_DEDUCE_RULES, apply_action, deduce_assuming_unique};
use crate::explain::{ExplainStep, explain_deduce, explain_lookahead, no_reason_detail};
use crate::format;
use crate::lookahead::{lookahead, lookahead_shortest};
use crate::render;
use crate::solve_deduce::{EngineConfig, VERIFY_ITERS_PER_QUESTION};
use crate::types::{Claim, QuestionType, QuestionTypeKind};

/// The user-facing prose of an explanation: its `Simple` steps joined (`Look` steps
/// are navigation, carrying no text).
fn render_hint(steps: &[ExplainStep]) -> String {
    steps
        .iter()
        .filter_map(|s| match s {
            ExplainStep::Simple { text } => Some(text.clone()),
            ExplainStep::Complex { header, lines } => {
                Some(format!("{header} — {}", lines.join("; ")))
            }
            ExplainStep::Look { .. } => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Does the hint carry a reason, or is it the bare "#3 can't be E." fallback?
/// Prefer reasoned examples so a rule that *usually* explains itself isn't shown by
/// its reason-less edge case.
fn informative(text: &str) -> bool {
    text.contains(':')
        || text.contains("claims")
        || text.contains("must be")
        || text.contains("What if")
}

/// Keep the best example per rule: reasoned over bare, then shortest (a one-step
/// deduction beats a long lookahead chain).
fn consider(map: &mut BTreeMap<String, String>, rule: &str, text: String) {
    let better = match map.get(rule) {
        None => true,
        Some(existing) => match (informative(&text), informative(existing)) {
            (true, false) => true,
            (false, true) => false,
            _ => text.len() < existing.len(),
        },
    };
    if better {
        map.insert(rule.to_string(), text);
    }
}

/// Audit the hints a player reads at a lookahead: how often the closing
/// "But …. Contradiction." line can't say *why* the hypothesis failed, reported per blamed
/// question type with an example each. Costs most of `reference`'s runtime.
fn hint_audit(puzzles: &[(String, crate::types::FlatPuzzle)]) {
    let mut hints = 0usize;
    let mut lines = 0usize;
    // Blamed question type → (count, shortest example hint).
    let mut no_reason: BTreeMap<QuestionTypeKind, (usize, String)> = BTreeMap::new();
    let mut no_reason_total = 0usize;

    for (_, fp) in puzzles {
        let mut state = fp.initial_state;
        for _ in 0..fp.n * VERIFY_ITERS_PER_QUESTION {
            if (0..fp.n).all(|i| state.answers[i].is_some()) {
                break;
            }
            let mut drs = deduce_assuming_unique(fp, &state);
            drs.sort_by_key(|dr| dr.rule as u8);
            if let Some(dr) = drs.first() {
                apply_action(&dr.action, &mut state);
                continue;
            }
            let Some(lr) = lookahead_shortest(fp, &state, usize::MAX, &mut 0) else {
                break;
            };
            for step in &explain_lookahead(fp, &state, &lr) {
                let ExplainStep::Complex { header, lines: ls } = step else {
                    continue;
                };
                hints += 1;
                lines += ls.len();
                let vague = format!(
                    "But {}. Contradiction.",
                    no_reason_detail(lr.contradiction_qi)
                );
                if !ls.contains(&vague) {
                    continue;
                }
                no_reason_total += 1;
                let kind = fp.question_types[lr.contradiction_qi].kind();
                // The expanded hint only — `render_hint` would also pick up the collapsed
                // "What if …?" teaser that precedes it and read as a stutter.
                let text = format!("{header} — {}", ls.join("; "));
                let entry = no_reason.entry(kind).or_insert((0, text.clone()));
                entry.0 += 1;
                if text.len() < entry.1.len() {
                    entry.1 = text;
                }
            }
            state.eliminated[lr.eliminate_qi] |= 1 << lr.eliminate_oi;
        }
    }

    println!(
        "# Lookahead hints — {hints} on the corpus hint path, {:.1} lines each\n",
        lines as f64 / hints.max(1) as f64
    );
    println!(
        "{no_reason_total} ({:.1}%) can't say why the hypothesis failed, by blamed question type:\n",
        100.0 * no_reason_total as f64 / hints.max(1) as f64
    );
    for (kind, (count, example)) in &no_reason {
        println!("## {kind:?} — {count}\n    {example}\n");
    }
}

/// `details` adds [`hint_audit`], which costs several times the rest of the command.
pub fn reference(details: bool) {
    let puzzles = crate::daily_puzzles();

    // kind -> (display tag, prompt, option labels); rule name -> one rendered hint.
    let mut qtypes: BTreeMap<QuestionTypeKind, (String, String, Vec<String>)> = BTreeMap::new();
    let mut rules: BTreeMap<String, String> = BTreeMap::new();

    for (_key, fp) in &puzzles {
        for qi in 0..fp.n {
            let qt = fp.question_types[qi];
            qtypes.entry(qt.kind()).or_insert_with(|| {
                let opts = (0..fp.option_count)
                    .map(|oi| {
                        let ov = fp.options[qi][oi];
                        match (qt, fp.true_stmt_question_types.as_ref()) {
                            (QuestionType::TrueStmt, Some(types)) => render::claim_label(&Claim {
                                question_type: types[oi],
                                value: ov,
                            }),
                            _ => render::option_label(&qt, ov),
                        }
                    })
                    .collect();
                (
                    format::format_type_tag(&qt),
                    render::question_text(&qt),
                    opts,
                )
            });
        }

        // Mirror `run_engine` (the verify solver), but render every deduction of a
        // round against that round's pre-state — the exact state `deduce` derived it
        // from. A naive one-by-one replay would show later same-round steps a state
        // already mutated by earlier ones, and a count/positional reason then can't
        // reconstruct its source (e.g. the count already reads as saturated).
        let cfg = EngineConfig::fallback();
        let mut state = fp.initial_state;
        for _ in 0..fp.n * VERIFY_ITERS_PER_QUESTION {
            if (0..fp.n).all(|i| state.answers[i].is_some()) {
                break;
            }
            let drs = deduce_assuming_unique(fp, &state);
            if !drs.is_empty() {
                for dr in &drs {
                    consider(
                        &mut rules,
                        dr.rule.to_str(),
                        render_hint(&explain_deduce(fp, &state, dr)),
                    );
                }
                for dr in &drs {
                    apply_action(&dr.action, &mut state);
                }
                continue;
            }
            // No deduction: the verify engine falls to full, unbounded lookahead.
            // Render the refutation as production does (explain_lookahead), attributed
            // to every rule in the chain so answered-case rules that only fire inside
            // lookahead still get a real, player-accurate example.
            let mut deduce_calls = 0;
            if let Some(lr) = lookahead(fp, &state, cfg.lookahead_deduce_until, &mut deduce_calls) {
                let hint = render_hint(&explain_lookahead(fp, &state, &lr));
                for cd in &lr.chain {
                    consider(&mut rules, cd.rule.to_str(), hint.clone());
                }
                state.eliminated[lr.eliminate_qi] |= 1 << lr.eliminate_oi;
                continue;
            }
            break;
        }
    }

    let all_kinds = QuestionTypeKind::all();
    println!(
        "# Question types — {}/{} used\n",
        qtypes.len(),
        all_kinds.len()
    );
    for (tag, prompt, opts) in qtypes.values() {
        println!("## {tag}\n    {prompt}\n    [ {} ]\n", opts.join("  |  "));
    }
    let unused_kinds: Vec<String> = all_kinds
        .iter()
        .filter(|k| !qtypes.contains_key(k))
        .map(|k| format!("{k:?}"))
        .collect();
    if !unused_kinds.is_empty() {
        println!("NEVER USED IN CORPUS: {}\n", unused_kinds.join(", "));
    }

    println!(
        "# Deduce rules — {}/{} used\n",
        rules.len(),
        ALL_DEDUCE_RULES.len()
    );
    for (rule, example) in &rules {
        println!("## {rule}\n    {example}\n");
    }
    let unused_rules: Vec<&str> = ALL_DEDUCE_RULES
        .iter()
        .map(|r| r.to_str())
        .filter(|name| !rules.contains_key(*name))
        .collect();
    if !unused_rules.is_empty() {
        println!(
            "NEVER USED IN CORPUS ({}): {}",
            unused_rules.len(),
            unused_rules.join(", ")
        );
    }
    println!();
    if details {
        hint_audit(&puzzles);
    } else {
        println!("Run with --details for the lookahead hint audit (slower).");
    }
}
