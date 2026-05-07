# Refpuzzle

Self-referential logic puzzles, in the spirit of Jim Propp's *Self-Referential Aptitude Test*.

Live at **[refpuzzle.com](https://refpuzzle.com)**.

## How it works

Each puzzle is a set of multiple-choice questions that refer to *the puzzle itself* — how many questions have a certain answer, which answers appear where, whether answers match between specific questions, and so on.

There is exactly one combination of answers that satisfies every question simultaneously. "Answers" means the answers *you give*: the puzzle is entirely self-contained, with no external truth to look up. Solving requires logic and deduction — eliminating wrong options, making tentative selections, and revising as new constraints emerge.

A new puzzle is published daily, in five difficulty levels.

## Development

```sh
pnpm install
pnpm dev      # start dev server
pnpm build    # lint + type-check + production build
pnpm fmt      # format (oxfmt)
pnpm lint     # lint + type-check (oxlint)
pnpm test     # run tests
```

Puzzle generation lives in Rust:

```sh
cargo run --release    # in rust/
```

## Architecture

- **Frontend**: Preact + preact-iso, no backend.
- **Engine** (`src/engine/`): pure functions for puzzle representation, deduction, and validation.
- **Generator** (`rust/`): Rust CLI that produces puzzles, output as JSON in `public/puzzles/daily/`.
- **i18n** (`src/i18n/`): all UI strings live here (English only for now).

## License

Copyright © 2026 Dan Wolff. Licensed under [AGPL-3.0](LICENSE).
