//! `refpuzzle hint-dump`: every hint the corpus can produce, one line each, for
//! diffing what players read across engine changes. Walks each puzzle's verify
//! solve and renders every deduction of every round (against that round's
//! pre-state) plus every lookahead refutation — the same paths `reference`
//! samples, dumped in full instead of one example per rule.

use refpuzzle::deduce::{
    DeduceReason, DeduceReasons, DeduceResult, apply_action, deduce_assuming_unique_with_reasons,
};
use refpuzzle::explain::{ExplainStep, explain_deduce, explain_lookahead, focus_questions};
use refpuzzle::lookahead::lookahead_shortest;
use refpuzzle::solve_deduce::VERIFY_ITERS_PER_QUESTION;

/// One hint as a line: first the questions its last `Look` step points at (where
/// the L1 coach aims its arrows), then the prose from its text steps. The
/// highlight can move while the wording holds still, so the line carries both.
fn render_hint(steps: &[ExplainStep]) -> String {
    let look = focus_questions(steps)
        .iter()
        .map(|qi| format!("#{}", qi + 1))
        .collect::<Vec<_>>()
        .join(",");
    let prose = steps
        .iter()
        .filter_map(|s| match s {
            ExplainStep::Simple { text } => Some(text.clone()),
            ExplainStep::Complex { header, lines } => {
                Some(format!("{header} — {}", lines.join("; ")))
            }
            ExplainStep::Look { .. } => None,
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("[look {look}] {prose}")
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
