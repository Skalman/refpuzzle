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
- `pnpm gen` and `pnpm check` are thin wrappers for `cargo run -- gen` / `check`.
- `pnpm test` runs the Rust test suite (`cargo test --release`); TS code is
  covered by `pnpm lint` (type-checking) and the wasm boundary itself.
- `pnpm wasm` rebuilds `rust/pkg/`; `pnpm build` does it as part of the build.
- All cargo commands run in the root dir, not in rust/
- Preact + preact-iso frontend, no backend
- TS engine survivors in `src/engine/`: `state.ts` (mark derivation),
  `explain.ts` (hint prose), `render.ts` (i18n), `tutorial.ts` (scripted
  steps), `types.ts` + `hint-types.ts` (shared wire/types), `serialize.ts`
  (small TS-side helpers).
- Puzzle data in `public/puzzles/daily/<year>.json` (compact form, parsed
  by both wasm and TS).
- Year JSON missing or doesn't contain the date → wasm generates on the
  fly in `src/puzzles/daily.ts::fetchDaily`.
- All UI strings in src/i18n/ (English only for now)
