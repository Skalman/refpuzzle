use serde_json::{Value, json};

use crate::types::*;

/// A self-contained `/playground` link that renders this exact puzzle, opened on
/// `state`'s resolved cells (so e.g. a stuck case shows where the engine got).
/// Mirrors the frontend's `encodePlaygroundHash`: `#p=base64url(deflate-raw(
/// compact JSON))[&h=<play history>]`. Native-only: deflate/base64 aren't pulled
/// into the wasm build.
#[cfg(not(target_arch = "wasm32"))]
pub fn playground_link(
    origin: &str,
    question_types: &[QuestionType; MAX_N],
    fp: &FlatPuzzle,
    state: &State,
) -> String {
    use base64::Engine;
    use flate2::Compression;
    use flate2::write::DeflateEncoder;
    use std::io::Write;

    let json = serde_json::to_string(&puzzle_to_compact_value(question_types, fp)).unwrap();
    let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
    enc.write_all(json.as_bytes()).unwrap();
    let p = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(enc.finish().unwrap());

    // Play history: `{q}{A}` selects the correct option, `{q}{a}` eliminates a
    // wrong one — one step per resolved real cell, in (qi, oi) order to match
    // playground.ts. Phantom slots (oi >= option_count) aren't markable, skip.
    let mut steps: Vec<String> = Vec::new();
    for qi in 0..fp.n {
        for oi in 0..fp.option_count {
            let letter = (b'A' + oi as u8) as char;
            if state.answers[qi] == Some(Answer::from(oi as u8)) {
                steps.push(format!("{}{letter}", qi + 1));
            } else if (state.eliminated[qi] >> oi) & 1 == 1 {
                steps.push(format!("{}{}", qi + 1, letter.to_ascii_lowercase()));
            }
        }
    }

    if steps.is_empty() {
        format!("{origin}/playground#p={p}")
    } else {
        format!("{origin}/playground#p={p}&h={}", steps.join("."))
    }
}

/// Inverse of `parse_puzzle`: serialize an in-memory `FlatPuzzle` (plus its
/// original `question_types` slice) back to the compact `{q, o, t?}` JSON
/// shape stored on disk and accepted by the parser.
pub fn puzzle_to_compact_value(question_types: &[QuestionType; MAX_N], fp: &FlatPuzzle) -> Value {
    let n = fp.n;
    let oc = fp.option_count;
    let mut obj = serde_json::Map::new();

    let qs: Vec<Value> = (0..n)
        .map(|qi| serde_json::to_value(question_types[qi]).unwrap())
        .collect();
    obj.insert("q".into(), json!(qs));

    let opts: Vec<Value> = (0..n)
        .map(|qi| option_row_json(&question_types[qi], qi, oc, fp))
        .collect();
    obj.insert("o".into(), json!(opts));

    if let Some(types) = fp.true_stmt_question_types.as_ref() {
        let arr: Vec<Value> = types
            .iter()
            .map(|qt| serde_json::to_value(qt).unwrap())
            .collect();
        obj.insert("t".into(), json!(arr));
    }

    Value::Object(obj)
}

fn option_row_json(qt: &QuestionType, qi: usize, oc: usize, fp: &FlatPuzzle) -> Value {
    if qt.has_identity_options() {
        let row: Vec<Value> = (0..oc).map(|oi| json!(oi)).collect();
        return json!(row);
    }
    let row: Vec<Value> = (0..oc)
        .map(|oi| {
            let s = fp.options[qi][oi];
            if !s.is_num() {
                Value::Null
            } else {
                json!(s.value())
            }
        })
        .collect();
    json!(row)
}

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
