# Project conventions

## Code quality

- Run `pnpm lint` before finishing — it covers TS linting and type checking
- Run `cargo clippy` before finishing — it covers RS linting; `cargo check` for type checking
- Use `pnpm fmt` or `cargo fmt` to format code

## Git

- Use simple `git commit -m "message"` — no heredocs, no `$(cat <<EOF)`
- Commit one logical change at a time

## Architecture

- Puzzle engine + generator live in Rust (`rust/src/`), compiled to wasm
  via `wasm-pack` and loaded from `src/lib/wasm.ts`. Browser runtime uses
  wasm exclusively for check_answer / deduce / lookahead / solve / generate.
- `pnpm test` runs the Rust test suite (`cargo test --release`); TS code is
  covered by `pnpm lint` (type-checking) and the wasm boundary itself.
- All cargo commands run in the root dir, not in rust/
- Rust owns all question types, logic, and prose. The frontend never models
  a `QuestionType`; it holds the compact blob plus rendered board text
  (`PuzzleHandle.renderBoard` → question prompt + option labels) and marks.
- TS engine survivors in `src/engine/`: `state.ts` (mark derivation),
  `tutorial.ts` (scripted steps), `types.ts` (Puzzle/marks types + letter
  helpers), `hint-types.ts` (hint wire types).
- Puzzle data in `public/puzzles/daily/<year>.json` (compact form, parsed
  by wasm; the frontend hands the blob to Rust rather than expanding it).
- Year JSON missing or doesn't contain the date → wasm generates on the
  fly in `src/puzzles/daily.ts::fetchDaily`.
- All UI strings in src/i18n/ (English only for now)
