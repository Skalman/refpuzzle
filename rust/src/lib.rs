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
    use crate::difficulty::PROFILES;
    use crate::rng::Rng;
    use crate::serialize::{parse_puzzle, puzzle_to_compact_value};
    use crate::types::{Answer, FlatPuzzle, MAX_N, State};
    use serde::Deserialize;
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

    #[derive(Deserialize)]
    struct StateInput {
        answers: Vec<Option<Answer>>,
        eliminated: Vec<u32>,
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
            let input: StateInput =
                serde_wasm_bindgen::from_value(state).map_err(|e| err(&e.to_string()))?;
            let n = self.fp.n;
            if input.answers.len() < n || input.eliminated.len() < n {
                return Err(err("state too short"));
            }
            let mut answers = [None; MAX_N];
            let mut eliminated = [0u8; MAX_N];
            for qi in 0..n {
                answers[qi] = input.answers[qi];
                eliminated[qi] = input.eliminated[qi] as u8;
            }
            let s = State {
                answers,
                eliminated,
            };
            let mut out = Vec::with_capacity(n);
            for qi in 0..n {
                out.push(validity_to_u8(check_answer(&self.fp, s, qi)));
            }
            Ok(out)
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
