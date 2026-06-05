use serde_json::Value;

use crate::types::*;

pub fn parse_puzzle(v: &Value) -> Option<FlatPuzzle> {
    let qs = v.get("q")?.as_array()?;
    let opts_arr = v.get("o")?.as_array()?;
    let n = qs.len();
    if n == 0 || n > MAX_N || opts_arr.len() != n {
        return None;
    }

    let option_count = opts_arr
        .first()
        .and_then(|o| o.as_array())
        .map_or(5, |a| a.len());

    let mut question_types = [QuestionType::AnswerIsSelf; MAX_N];
    let mut options = [[OptionValue::UNUSED; 5]; MAX_N];

    for qi in 0..n {
        question_types[qi] = serde_json::from_value(qs[qi].clone()).ok()?;
    }

    let true_stmt_question_types: Option<[QuestionType; 5]> = if let Some(t) = v.get("t") {
        let arr = t.as_array()?;
        if arr.len() != 5 {
            return None;
        }
        let mut types = [QuestionType::AnswerIsSelf; 5];
        for (i, qt) in arr.iter().enumerate() {
            types[i] = serde_json::from_value(qt.clone()).ok()?;
        }
        Some(types)
    } else {
        None
    };

    for (qi, opts) in opts_arr.iter().enumerate() {
        let qt = &question_types[qi];
        // Identity-option types persist a degenerate `[null, null, ...]` row on
        // wire; rebuild the canonical letter indices in memory.
        if qt.has_identity_options() {
            for oi in 0..option_count {
                options[qi][oi] = OptionValue::num(oi as u8);
            }
            continue;
        }
        let row = opts.as_array()?;
        for (oi, o) in row.iter().enumerate() {
            options[qi][oi] = if o.is_null() {
                OptionValue::NONE
            } else {
                let v = o.as_i64()?;
                if (0..0xFE).contains(&v) {
                    OptionValue::num(v as u8)
                } else {
                    OptionValue::UNUSED
                }
            };
        }
    }

    let (affected_by, global_indices) = FlatPuzzle::build_deps(&question_types, n);
    Some(FlatPuzzle {
        question_types,
        options,
        true_stmt_question_types,
        affected_by,
        global_indices,
        n,
        option_count,
        initial_state: State::initial(option_count),
    })
}
