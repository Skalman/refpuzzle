#![allow(
    clippy::needless_range_loop,
    clippy::len_without_is_empty,
    clippy::new_without_default,
    clippy::should_implement_trait
)]

pub mod check_answer;
pub mod check_form;
pub mod check_well_posed;
pub mod construct;
pub mod deduce;
pub mod difficulty;
pub mod explain;
pub mod fill;
pub mod format;
pub mod lookahead;
pub mod render;
pub mod rng;
pub mod serialize;
pub mod solve_brute;
pub mod solve_deduce;
#[cfg(test)]
mod test_util;
pub mod types;

// Smaller allocator (~1–2 KB vs dlmalloc's ~10 KB) for the wasm bundle.
// AssumeSingleThreaded is sound because wasm32-unknown-unknown has no threads.
#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOCATOR: lol_alloc::AssumeSingleThreaded<lol_alloc::FreeListAllocator> =
    unsafe { lol_alloc::AssumeSingleThreaded::new(lol_alloc::FreeListAllocator::new()) };

#[cfg(target_arch = "wasm32")]
mod wasm_api {
    use crate::check_answer::{Validity, check_answer};
    use crate::check_form::{Severity, check_form};
    use crate::construct;
    use crate::deduce::{DeduceAction, deduce_assuming_unique};
    use crate::difficulty::PROFILES;
    use crate::explain::{ExplainStep, explain_deduce, explain_lookahead};
    use crate::fill;
    use crate::lookahead::lookahead_shortest;
    use crate::render;
    use crate::rng::Rng;
    use crate::serialize::{parse_puzzle, puzzle_to_compact_value};
    use crate::solve_deduce::solve;
    use crate::types::{Answer, Claim, FlatPuzzle, MAX_N, QuestionType, State};
    use serde::{Deserialize, Serialize};
    use wasm_bindgen::prelude::*;

    fn err(msg: &str) -> JsError {
        JsError::new(msg)
    }

    /// Wire encoding for `Validity`; `wasm.ts::validityFromU8` is the inverse.
    /// Documented on the `Validity` enum (check_answer.rs). Keep all three in sync.
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

    // ── Wire shapes for the hint engine (match the TS types in
    // src/engine/hint-types.ts). Answer rendered as "A".."E" string; field
    // names camelCase.

    #[derive(Serialize)]
    #[serde(tag = "type", rename_all = "camelCase")]
    enum DeduceActionApi {
        Force {
            qi: usize,
            answer: char,
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

    /// One solving step plus its rendered explanation — the unit the hint UI
    /// renders (`explain`) and the tutorial walks (applies `action`, shows
    /// `explain`).
    #[derive(Serialize)]
    struct StepApi {
        action: DeduceActionApi,
        explain: Vec<ExplainStep>,
    }

    /// One question's rendered board text: the prompt and one label per option.
    #[derive(Serialize)]
    struct BoardQuestionApi {
        text: String,
        options: Vec<String>,
    }

    fn action_to_api(a: DeduceAction) -> DeduceActionApi {
        match a {
            DeduceAction::Force { qi, answer } => DeduceActionApi::Force {
                qi,
                answer: answer.as_char(),
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
            // Reject a malformed (playground) puzzle here with a clear message,
            // rather than letting a downstream deduce/check_answer panic trap the
            // module. Warnings are tolerated; only fatal form errors block.
            let fatal: Vec<String> = check_form(&fp)
                .into_iter()
                .filter(|e| matches!(e.severity, Severity::Error))
                .map(|e| format!("Q{}: {}", e.qi + 1, e.message))
                .collect();
            if !fatal.is_empty() {
                return Err(err(&format!("malformed puzzle: {}", fatal.join("; "))));
            }
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
            let answers: Vec<Option<char>> = r
                .answers
                .iter()
                .take(self.fp.n)
                .map(|a| a.map(Answer::as_char))
                .collect();
            serde_wasm_bindgen::to_value(&answers).map_err(|e| err(&e.to_string()))
        }

        /// The next solving step plus its rendered explanation, or null when the
        /// puzzle is solved or truly stuck. Prefers the highest-priority single
        /// deduction (sorted to match the TS engine's `sortDeduceResults`);
        /// failing that, falls back to the shortest lookahead contradiction,
        /// whose `action` eliminates the refuted assumption. The hint UI renders
        /// `explain`; the tutorial also applies `action` and walks to a full
        /// solve. Keeping deduction + prose together means no `DeduceResult`
        /// crosses the wire.
        #[wasm_bindgen(js_name = nextStep)]
        pub fn next_step(&self, state: JsValue) -> Result<JsValue, JsError> {
            let s = parse_state(state, self.fp.n)?;
            let mut drs = deduce_assuming_unique(&self.fp, &s);
            drs.sort_by_key(|dr| dr.rule as u8);
            let api = if let Some(dr) = drs.first() {
                StepApi {
                    action: action_to_api(dr.action),
                    explain: explain_deduce(&self.fp, &s, dr),
                }
            } else if let Some(lr) = lookahead_shortest(&self.fp, &s) {
                StepApi {
                    action: DeduceActionApi::Eliminate {
                        qi: lr.eliminate_qi,
                        oi: lr.eliminate_oi,
                    },
                    explain: explain_lookahead(&self.fp, &s, &lr),
                }
            } else {
                return Ok(JsValue::NULL);
            };
            serde_wasm_bindgen::to_value(&api).map_err(|e| err(&e.to_string()))
        }

        /// The rendered board text for every question: the prompt plus one
        /// label per option. TrueStmt rows carry per-option claim text. The
        /// frontend caches these strings and models no question types itself.
        #[wasm_bindgen(js_name = renderBoard)]
        pub fn render_board(&self) -> Result<JsValue, JsError> {
            let fp = &self.fp;
            let board: Vec<BoardQuestionApi> = (0..fp.n)
                .map(|qi| {
                    let qt = &fp.question_types[qi];
                    let options = (0..fp.option_count)
                        .map(|oi| {
                            let ov = fp.options[qi][oi];
                            match (qt, fp.true_stmt_question_types.as_ref()) {
                                (QuestionType::TrueStmt, Some(types)) => {
                                    render::claim_label(&Claim {
                                        question_type: types[oi],
                                        value: ov,
                                    })
                                }
                                _ => render::option_label(qt, ov),
                            }
                        })
                        .collect();
                    BoardQuestionApi {
                        text: render::question_text(qt),
                        options,
                    }
                })
                .collect();
            serde_wasm_bindgen::to_value(&board).map_err(|e| err(&e.to_string()))
        }
    }

    /// Returns a CompactPuzzle JSON string, or an `Err` if generation
    /// exhausted its retry budget. Mirrors the seed-retry loop the native
    /// CLI uses in `main.rs`.
    #[wasm_bindgen(js_name = generatePuzzle)]
    pub fn generate_puzzle(seed: u32, level: u8) -> Result<String, JsError> {
        if !(1..=6).contains(&level) {
            return Err(err("level must be 1..=6"));
        }
        let profile = &PROFILES[(level - 1) as usize];
        let mut stats = fill::Stats::default();
        // The generator fixes the key on the first skeleton and retries internally, so one
        // seed suffices. `seed * 17` matches the CLI's `task_seeds` derivation
        // (main.rs), so a puzzle generated here is identical to the same
        // date/level built by `gen`.
        let mut rng = Rng::new(seed.wrapping_mul(17));
        match construct::generate(
            &construct::RECIPES[(level - 1) as usize],
            profile.question_count,
            profile.option_count,
            &mut rng,
            construct::DEFAULT_MAX_REGENERATIONS,
            &mut stats,
            "wasm",
        ) {
            Some(result) => {
                let value = puzzle_to_compact_value(&result.question_types, &result.fp);
                Ok(serde_json::to_string(&value).unwrap())
            }
            None => Err(err("generator exhausted retry budget")),
        }
    }
}
