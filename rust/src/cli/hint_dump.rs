//! `refpuzzle hint-dump`: every hint the corpus can produce, one line each, for
//! diffing prose across engine changes. Walks each puzzle's verify solve and
//! renders every deduction of every round (against that round's pre-state) plus
//! every lookahead refutation — the same paths `reference` samples, dumped in
//! full instead of one example per rule. Bin-only.

use crate::deduce::{
    DeduceReason, DeduceReasons, DeduceResult, apply_action, deduce_assuming_unique_with_reasons,
};
use crate::explain::{ExplainStep, explain_deduce, explain_lookahead};
use crate::lookahead::lookahead_shortest;
use crate::solve_deduce::VERIFY_ITERS_PER_QUESTION;

/// The user-facing prose of an explanation: its text steps joined (`Look` steps
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

pub fn hint_dump() {
    let puzzles = crate::corpus::daily_puzzles();
    for (key, fp) in &puzzles {
        let mut state = fp.initial_state;
        for _ in 0..fp.n * VERIFY_ITERS_PER_QUESTION {
            if (0..fp.n).all(|i| state.answers[i].is_some()) {
                break;
            }
            let mut reasons = DeduceReasons::new();
            let results = deduce_assuming_unique_with_reasons(fp, &state, &mut reasons);
            let mut drs: Vec<(DeduceResult, DeduceReason)> =
                results.iter().copied().zip(reasons).collect();
            drs.sort_by_key(|(dr, _)| dr.rule as u8);
            if !drs.is_empty() {
                for (dr, reason) in &drs {
                    println!(
                        "{key} {}: {}",
                        dr.rule.to_str(),
                        render_hint(&explain_deduce(fp, &state, dr, *reason))
                    );
                }
                for (dr, _) in &drs {
                    apply_action(&dr.action, &mut state);
                }
                continue;
            }
            // Same picker as the browser hint engine, so the dump covers the
            // chains and closing lines a player actually gets.
            let Some(lr) = lookahead_shortest(fp, &state, usize::MAX, &mut 0) else {
                break;
            };
            println!(
                "{key} lookahead: {}",
                render_hint(&explain_lookahead(fp, &state, &lr))
            );
            state.eliminated[lr.eliminate_qi] |= 1 << lr.eliminate_oi;
        }
    }
}
