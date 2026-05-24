# Project conventions

## Code quality

- Run `pnpm lint` before finishing — it covers TS linting and type checking
- Run `cargo clippy` before finishing — it covers RS linting; `cargo check` for type checking
- Use `pnpm fmt` or `cargo fmt` to format code

## Git

- Use simple `git commit -m "message"` — no heredocs, no `$(cat <<EOF)`
- Commit one logical change at a time

## Architecture

- Puzzle generation in both TypeScript and Rust
- All cargo commands run in the root dir, not in rust/
- Preact + preact-iso frontend, no backend
- All puzzle logic in src/engine/ (pure functions)
- Puzzle data in src/puzzles/generated/ (auto-generated, committed)
- Generator in src/generator/ (CLI tool, not bundled in frontend)
- All UI strings in src/i18n/ (English only for now)
