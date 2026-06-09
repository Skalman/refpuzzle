#![allow(
    clippy::needless_range_loop,
    clippy::len_without_is_empty,
    clippy::new_without_default,
    clippy::should_implement_trait
)]

pub mod build;
pub mod check_answer;
pub mod check_form;
pub mod construct;
pub mod deduce;
pub mod difficulty;
pub mod format;
pub mod lookahead;
pub mod rng;
pub mod serialize;
pub mod solve_brute;
pub mod solve_deduce;
pub mod types;

#[cfg(target_arch = "wasm32")]
mod wasm_api {
    use crate::build;
    use crate::check_answer::{Validity, check_answer};
    use crate::construct;
    use crate::deduce::{DeduceAction, DeduceResult, deduce_assuming_unique};
    use crate::difficulty::PROFILES;
    use crate::lookahead::lookahead;
    use crate::rng::Rng;
    use crate::serialize::{parse_puzzle, puzzle_to_compact_value};
    use crate::solve_deduce::solve;
    use crate::types::{Answer, FlatPuzzle, MAX_N, State};
    use serde::{Deserialize, Serialize};
    use wasm_bindgen::prelude::*;

    fn err(msg: &str) -> JsError {
        JsError::new(msg)
    }

    fn validity_to_u8(v: Validity) -> u8 {
        match v {
            Validity::Neutral => 0,
            Validity::Valid => 1,
            Validity::Consistent => 2,
            Validity::Invalid => 3,
            Validity::Pending => 4,
        }
    }

    fn answer_to_str(a: Answer) -> &'static str {
        match a {
            Answer::A => "A",
            Answer::B => "B",
            Answer::C => "C",
            Answer::D => "D",
            Answer::E => "E",
        }
    }

    #[derive(Deserialize)]
    struct StateInput {
        answers: Vec<Option<Answer>>,
        eliminated: Vec<u32>,
    }

    fn parse_state(state: JsValue, n: usize) -> Result<State, JsError> {
        let input: StateInput =
            serde_wasm_bindgen::from_value(state).map_err(|e| err(&e.to_string()))?;
        if input.answers.len() < n || input.eliminated.len() < n {
            return Err(err("state too short"));
        }
        let mut answers = [None; MAX_N];
        let mut eliminated = [0u8; MAX_N];
        for qi in 0..n {
            answers[qi] = input.answers[qi];
            eliminated[qi] = input.eliminated[qi] as u8;
        }
        Ok(State {
            answers,
            eliminated,
        })
    }

    // ── Wire shapes for the hint engine (match the TS types in deduce.ts /
    // lookahead.ts). Answer rendered as "A".."E" string; field names camelCase.

    #[derive(Serialize)]
    #[serde(tag = "type", rename_all = "camelCase")]
    enum DeduceActionApi {
        Force {
            qi: usize,
            answer: &'static str,
        },
        Eliminate {
            qi: usize,
            oi: usize,
        },
        EliminateMulti {
            #[serde(rename = "questionMask")]
            question_mask: u16,
            #[serde(rename = "optionMask")]
            option_mask: u8,
        },
    }

    #[derive(Serialize)]
    struct DeduceResultApi {
        action: DeduceActionApi,
        rule: &'static str,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct LookaheadResultApi {
        eliminate_qi: usize,
        eliminate_oi: usize,
        assumption_qi: usize,
        assumption_answer: &'static str,
        chain: Vec<DeduceResultApi>,
        contradiction_qi: usize,
    }

    fn action_to_api(a: DeduceAction) -> DeduceActionApi {
        match a {
            DeduceAction::Force { qi, answer } => DeduceActionApi::Force {
                qi,
                answer: answer_to_str(answer),
            },
            DeduceAction::Eliminate { qi, oi } => DeduceActionApi::Eliminate { qi, oi },
            DeduceAction::EliminateMulti {
                question_mask,
                option_mask,
            } => DeduceActionApi::EliminateMulti {
                question_mask,
                option_mask,
            },
        }
    }

    fn result_to_api(r: DeduceResult) -> DeduceResultApi {
        DeduceResultApi {
            action: action_to_api(r.action),
            rule: r.rule.to_str(),
        }
    }

    #[wasm_bindgen]
    pub struct Puzzle {
        fp: FlatPuzzle,
    }

    #[wasm_bindgen]
    impl Puzzle {
        /// `compact_json` is the on-disk shape: `{ "q": [...], "o": [...], "t"?: [...] }`.
        #[wasm_bindgen(constructor)]
        pub fn new(compact_json: &str) -> Result<Puzzle, JsError> {
            let v: serde_json::Value =
                serde_json::from_str(compact_json).map_err(|e| err(&e.to_string()))?;
            let fp = parse_puzzle(&v).ok_or_else(|| err("failed to parse puzzle"))?;
            Ok(Puzzle { fp })
        }

        /// Returns one `Validity` (as u8) per question, indexed by `qi`.
        /// `state` must be `{ answers: (Answer|null)[], eliminated: number[] }`.
        #[wasm_bindgen(js_name = checkAllAnswers)]
        pub fn check_all_answers(&self, state: JsValue) -> Result<Vec<u8>, JsError> {
            let s = parse_state(state, self.fp.n)?;
            let mut out = Vec::with_capacity(self.fp.n);
            for qi in 0..self.fp.n {
                out.push(validity_to_u8(check_answer(&self.fp, s, qi)));
            }
            Ok(out)
        }

        /// Solves the puzzle deterministically; returns `(string|null)[]` of
        /// answers indexed by `qi`. Nulls only for questions that the deduce
        /// solver cannot uniquely answer.
        #[wasm_bindgen(js_name = solve)]
        pub fn solve(&self) -> Result<JsValue, JsError> {
            let r = solve(&self.fp);
            let answers: Vec<Option<&'static str>> = r
                .answers
                .iter()
                .take(self.fp.n)
                .map(|a| a.map(answer_to_str))
                .collect();
            serde_wasm_bindgen::to_value(&answers).map_err(|e| err(&e.to_string()))
        }

        /// One pass of `deduce_assuming_unique`, sorted by rule order
        /// (matching the TS engine's `sortDeduceResults` output).
        #[wasm_bindgen(js_name = deduce)]
        pub fn deduce(&self, state: JsValue) -> Result<JsValue, JsError> {
            let s = parse_state(state, self.fp.n)?;
            let mut drs = deduce_assuming_unique(&self.fp, &s);
            drs.sort_by_key(|dr| dr.rule as u8);
            let out: Vec<DeduceResultApi> = drs.into_iter().map(result_to_api).collect();
            serde_wasm_bindgen::to_value(&out).map_err(|e| err(&e.to_string()))
        }

        /// Shortest lookahead chain, or null if no single-step assumption
        /// reaches a contradiction.
        #[wasm_bindgen(js_name = lookaheadShortest)]
        pub fn lookahead_shortest(&self, state: JsValue) -> Result<JsValue, JsError> {
            let s = parse_state(state, self.fp.n)?;
            // `lookahead(.., .., 1, false)` is the "shortest" mode the TS engine uses.
            let Some(lr) = lookahead(&self.fp, &s, 1, false) else {
                return Ok(JsValue::NULL);
            };
            let chain: Vec<DeduceResultApi> = lr.chain.iter().copied().map(result_to_api).collect();
            let api = LookaheadResultApi {
                eliminate_qi: lr.eliminate_qi,
                eliminate_oi: lr.eliminate_oi,
                assumption_qi: lr.assumption_qi,
                assumption_answer: answer_to_str(lr.assumption_answer),
                chain,
                contradiction_qi: lr.contradiction_qi,
            };
            serde_wasm_bindgen::to_value(&api).map_err(|e| err(&e.to_string()))
        }
    }

    /// Returns a CompactPuzzle JSON string, or empty string if generation
    /// exhausted its retry budget. Mirrors the seed-retry loop the native
    /// CLI uses in `main.rs`.
    #[wasm_bindgen(js_name = generatePuzzle)]
    pub fn generate_puzzle(seed: u32, level: u8) -> Result<String, JsError> {
        if !(1..=6).contains(&level) {
            return Err(err("level must be 1..=6"));
        }
        let profile = &PROFILES[(level - 1) as usize];
        let mut stats = build::Stats::default();
        for retry in 0..100u32 {
            let s = seed
                .wrapping_mul(17)
                .wrapping_add(retry.wrapping_mul(0x9e3779b9));
            let mut rng = Rng::new(s);
            if let Some(result) =
                construct::generate(profile, &mut rng, 100, &mut stats, false, "wasm")
            {
                let value = puzzle_to_compact_value(&result.question_types, &result.fp);
                return Ok(serde_json::to_string(&value).unwrap());
            }
        }
        Err(err("generator exhausted retry budget"))
    }
}
