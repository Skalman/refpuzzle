# Project conventions

## Code quality
- Run `pnpm lint` before finishing — it covers linting and type checking
- Use `pnpm fmt` to format code

## Git
- Use simple `git commit -m "message"` — no heredocs, no `$(cat <<EOF)`
- Commit one logical change at a time

## Architecture
- Preact + preact-iso frontend, no backend
- All puzzle logic in src/engine/ (pure functions)
- Puzzle data in src/puzzles/generated/ (auto-generated, committed)
- Generator in src/generator/ (CLI tool, not bundled in frontend)
- All UI strings in src/i18n/ (English only for now)
