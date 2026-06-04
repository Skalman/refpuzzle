use serde_json::Value;

use crate::types::*;

pub fn parse_puzzle(v: &Value) -> Option<FlatPuzzle> {
    let qs = v.get("q")?.as_array()?;
    let n = qs.len();
    if n == 0 || n > MAX_N {
        return None;
    }

    let option_count = qs
        .first()
        .and_then(|q| q.get("o"))
        .and_then(|o| o.as_array())
        .map_or(5, |a| a.len());

    let mut question_types = [QuestionType::AnswerIsSelf; MAX_N];
    let mut options = [[OptionValue::UNUSED; 5]; MAX_N];
    let mut true_stmt_question_types: Option<[QuestionType; 5]> = None;

    for (qi, q) in qs.iter().enumerate() {
        let t = q.get("t")?;
        question_types[qi] = serde_json::from_value(t.clone()).ok()?;

        if let Some(claims) = q.get("c") {
            // Old wire format: claims array stores `{question_type fields, "v": value}`.
            // Split into SoA: claim question types in `true_stmt_question_types`,
            // claim values in `options[qi]`.
            let claims = claims.as_array()?;
            let mut types = [QuestionType::AnswerIsSelf; 5];
            for (oi, c) in claims.iter().enumerate() {
                if c.is_null() {
                    continue;
                }
                let claim: Claim = {
                    let qt: QuestionType = serde_json::from_value(c.clone()).ok()?;
                    let value = c.get("v").and_then(|v| v.as_i64())?;
                    let ov = if value == NONE_VAL as i64 {
                        OptionValue::NONE
                    } else if (0..0xFE).contains(&value) {
                        OptionValue::num(value as u8)
                    } else {
                        OptionValue::UNUSED
                    };
                    Claim {
                        question_type: qt,
                        value: ov,
                    }
                };
                types[oi] = claim.question_type;
                options[qi][oi] = claim.value;
            }
            true_stmt_question_types = Some(types);
        } else if question_types[qi].has_identity_options() {
            for oi in 0..option_count {
                options[qi][oi] = OptionValue::num(oi as u8);
            }
        } else if let Some(opts) = q.get("o") {
            let opts = opts.as_array()?;
            for (oi, o) in opts.iter().enumerate() {
                options[qi][oi] = if o.is_null() {
                    OptionValue::NONE
                } else {
                    let v = o.as_i64()?;
                    // Out-of-range JSON values get stored verbatim as u8 so check_form
                    // can flag them via its existing range checks. Truly malformed
                    // values (negative, or >= 0xFE) collapse to UNUSED.
                    if (0..0xFE).contains(&v) {
                        OptionValue::num(v as u8)
                    } else {
                        OptionValue::UNUSED
                    }
                };
            }
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

/// Serialize a `Claim` to the old wire-format object (question-type fields plus `"v"`).
pub fn claim_to_json(claim: &Option<Claim>) -> Value {
    match claim {
        None => Value::Null,
        Some(c) => {
            let mut obj = serde_json::to_value(c.question_type).unwrap();
            if let Some(map) = obj.as_object_mut() {
                map.insert("v".into(), serde_json::json!(c.value.to_i16()));
            }
            obj
        }
    }
}
