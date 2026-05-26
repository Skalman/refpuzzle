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
    let mut option_nums = [[NAN_VAL; 5]; MAX_N];
    let mut option_answers = [[0xFFu8; 5]; MAX_N];
    let mut option_claims: [[Option<Claim>; 5]; MAX_N] = [[None; 5]; MAX_N];

    for (qi, q) in qs.iter().enumerate() {
        let t = q.get("t")?;
        question_types[qi] = serde_json::from_value(t.clone()).ok()?;

        if let Some(claims) = q.get("c") {
            let claims = claims.as_array()?;
            for (oi, c) in claims.iter().enumerate() {
                if c.is_null() {
                    continue;
                }
                option_claims[qi][oi] = Some(serde_json::from_value(c.clone()).ok()?);
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
                for oi in 0..option_count {
                    option_answers[qi][oi] = if option_nums[qi][oi] >= 0 && option_nums[qi][oi] <= 4
                    {
                        option_nums[qi][oi] as u8
                    } else {
                        0xFF
                    };
                    option_nums[qi][oi] = NAN_VAL;
                }
            }
            if question_types[qi].has_identity_options() {
                for oi in 0..option_count {
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
        option_count,
        initial_state: State::initial(option_count),
    })
}

pub fn claim_to_json(claim: &Option<Claim>) -> Value {
    match claim {
        None => Value::Null,
        Some(c) => serde_json::to_value(c).unwrap(),
    }
}
