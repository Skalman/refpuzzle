//! `refpuzzle reference`: a living reference of every question type and deduce
//! rule, each with a real rendered example collected by solving the daily corpus
//! under the full engine. Examples stay accurate as prose/rules change, and any
//! kind or rule never seen in the corpus is called out (a coverage signal).
//! Bin-only.

use std::collections::BTreeMap;

use crate::deduce::{ALL_DEDUCE_RULES, DeduceAction, apply_action, deduce_assuming_unique};
use crate::explain::{
    ExplainStep, explain_deduce, explain_lookahead, no_reason_detail, optionless_detail,
};
use crate::format;
use crate::lookahead::{Contradiction, lookahead, lookahead_shortest};
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

/// The routes `explain_lookahead` can phrase a closing "But …. Contradiction." line by,
/// in the order it tries them: the blamed claim's own reason if `check_answer` supplied
/// one, else the shape of the refuting deduction, else nothing.
const ROUTES: [&str; 5] = [
    "the blamed claim's own reason",
    "forced onto a ruled-out option",
    "an elimination removing the answer",
    "no options left",
    "nothing to say",
];

/// Audit the hints a player reads at a lookahead: how each closing "But …. Contradiction."
/// line got phrased, with a shortest example per route, plus every question type still
/// blamed by a line that can't say *why* the hypothesis failed. Costs most of
/// `reference`'s runtime.
fn hint_audit(puzzles: &[(String, crate::types::FlatPuzzle)]) {
    let mut hints = 0usize;
    let mut lines = 0usize;
    // Route → (count, shortest example).
    let mut routes: Vec<(usize, String)> = vec![(0, String::new()); ROUTES.len()];
    // The work list: blamed question types whose line stayed generic, and how often.
    let mut generic_kinds: BTreeMap<QuestionTypeKind, usize> = BTreeMap::new();

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
                let ExplainStep::Complex { lines: ls, .. } = step else {
                    continue;
                };
                hints += 1;
                lines += ls.len();
                // `explain_lookahead` always closes with the contradiction line followed
                // by "So #n can't be X.". Matching on ". Contradiction." instead would
                // also catch chain lines that end that way.
                let Some(closing) = ls.iter().rev().nth(1) else {
                    continue;
                };
                let but = |detail: String| *closing == format!("But {detail}. Contradiction.");
                let generic = but(no_reason_detail(lr.contradiction_qi));
                // The sentence identifies this route better than the variant does: a
                // question with no options left arrives as a `Conflict` on the elimination
                // that took its last one, not under a variant of its own.
                let optionless = but(optionless_detail(lr.contradiction_qi));
                // " would say " is the opener `ClaimSubject::Hypothesis` gives a claim's
                // own reason, and nothing else uses it, so it identifies that route exactly.
                let route = match (generic, optionless, closing.contains(" would say ")) {
                    (true, ..) => 4,
                    (_, true, _) => 3,
                    (.., true) => 0,
                    _ => match lr.contradiction {
                        Contradiction::Conflict {
                            result:
                                crate::deduce::DeduceResult {
                                    action: DeduceAction::Force { .. },
                                    ..
                                },
                            ..
                        } => 1,
                        _ => 2,
                    },
                };
                if generic {
                    *generic_kinds
                        .entry(fp.question_types[lr.contradiction_qi].kind())
                        .or_insert(0) += 1;
                }
                let entry = &mut routes[route];
                if entry.0 == 0 || closing.len() < entry.1.len() {
                    entry.1 = closing.clone();
                }
                entry.0 += 1;
            }
            state.eliminated[lr.eliminate_qi] |= 1 << lr.eliminate_oi;
        }
    }

    println!(
        "# Lookahead hints — {hints} on the corpus hint path, {:.1} lines each\n",
        lines as f64 / hints.max(1) as f64
    );
    for (route, (count, example)) in ROUTES.iter().zip(&routes) {
        println!(
            "## {route} — {count} ({:.1}%)",
            100.0 * *count as f64 / hints.max(1) as f64
        );
        if *count > 0 {
            println!("    {example}");
        }
        println!();
    }
    if generic_kinds.is_empty() {
        println!("Every closing line says why the hypothesis failed.");
    } else {
        println!("Generic endings by blamed question type:");
        for (kind, count) in &generic_kinds {
            println!("    {kind:?} — {count}");
        }
    }
}

/// `details` adds [`hint_audit`], which costs several times the rest of the command.
pub fn reference(details: bool) {
    let puzzles = crate::corpus::daily_puzzles();

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
