//! Shareable `/playground` links for the CLI's diagnostics. Bin-only: the deflate
//! and base64 encoders it needs aren't pulled into the wasm build.

use refpuzzle::serialize::puzzle_to_compact_value;
use refpuzzle::types::{Answer, FlatPuzzle, State};

/// A self-contained `/playground` link that renders this exact puzzle, opened on
/// `state`'s resolved cells (so e.g. a stuck case shows where the engine got).
/// Mirrors the frontend's `encodePlaygroundHash`: `#p=base64url(deflate-raw(
/// compact JSON))[&h=<play history>]`.
pub fn playground_link(origin: &str, fp: &FlatPuzzle, state: &State) -> String {
    use base64::Engine;
    use flate2::Compression;
    use flate2::write::DeflateEncoder;
    use std::io::Write;

    let json = serde_json::to_string(&puzzle_to_compact_value(fp)).unwrap();
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
            } else if state.is_eliminated(qi, oi) {
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
