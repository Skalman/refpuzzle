# Glossary

The vocabulary the codebase is written in, on both sides of the wasm boundary. The Rust
types named here live in `rust/src/types.rs`.

A puzzle is a grid: `n` **questions** down, `option_count` **options** across.

- **question** — one numbered row, `#1`..`#n`; indexed by `qi`. Carries a question type
  and its options.
- **option** — one lettered choice a question offers, `A`..`E`; indexed by `oi`.
- **cell** — one *question × option* square of the grid, `(qi, oi)` — the thing a player
  marks. `n * option_count` cells in all.

  Never a synonym for a question. A question runs out of options; a cell does not
  "empty". A force resolves a question, not a cell. Say "question" unless both
  coordinates are meant. The frontend keeps the distinction explicit —
  `CoachArrows.tsx` picks `cell(qi, oi)` or `row(qi)` by whether it has an `oi`.
- **row** — all of one question's cells. Frontend term (the grid line a question
  occupies); Rust just says "question".
- **answer** — the option a question resolves to, as a letter: `Answer`.
- **option value** — `OptionValue`: what an option *means* rather than what letter it is.
  A number — a question index, a count, a letter index, a distance, depending on the
  question type — or `NONE` for "no such question".
- **mark** — what the player has put on a cell: correct, incorrect, or blank. The engine
  sees marks as `State`'s `answers` (one per question) plus `eliminated` (a bitmask of
  options per question); `deriveState()` in `src/engine/state.ts` does the conversion.
- **eliminate** — rule one option out of a question, setting its `eliminated` bit.
  "Struck out" and "removed" mean the same thing; prefer *eliminate*.
- **force** — conclude that a question must take a given answer.
- **claim** — the proposition an option asserts if chosen: `Claim`, a question type plus
  a value. Every option makes one; a `TrueStmt` option makes someone else's.
- **question type** — `QuestionType`: the kind *with* its parameters, e.g.
  `CountAnswer { answer: B }`. This is what a claim compares against.
- **question type kind** — `QuestionTypeKind`: the same thing with parameters stripped,
  e.g. `CountAnswer`. Used for per-kind tables and coverage counts.
- **board** / **state** — `State`: the answers and eliminations, nothing else. The puzzle
  itself is a `FlatPuzzle`, which never changes during a solve.
- **checkpoint** — a history step that changes no marks, planted only when the board
  matches the key. Its marks are then verified, so they lock: clicking one refuses and
  flashes rather than changing it. Rewinding past the pin unlocks them, at the cost of
  the checkpoint. A safe rewind target, and the boundary the history track collapses at.
  Frontend-only.
- **marker** — a hint or refused-checkpoint record attached to a history step. Sits
  where the event happened; when a rewrite discards its step it folds onto the last
  surviving step instead of vanishing, so rewinding never erases the record. A hint
  marker carries the deepest hint level reached and the question the hint named; a
  fail marker carries the count of refusals and the questions they caught.
  Either may name no question.
- **flags** / **counters** — the two halves of the stored ledger. The flags (`s`
  solved, `st` stale) lead it and say where the puzzle stands; the counters follow
  and accumulate while solving. `docs/encoding.md` has the tokens.
- **outcome** — how one question went, for the shared result: *clean* (answered with
  no help), *hinted* (a hint named it), or *caught* (a refused checkpoint named it).
  Read off the markers' questions, so a question no marker named stays clean.
  A catch outranks a hint on the same question.
- **perfect** — every question clean. The result card then adds the all-green
  caption under the squares. *Clean* is about one question, *perfect* the board.
- **nudge** — an idle callout pointing at Checkpoint or Hint for a solver who is
  active but stuck. L2 and up; L1 has the coach instead.
- **result card** — the shareable picture of a solve: level, day, time, and one
  square per question in its outcome's color.
- **completion bar** — the row of ways onward that replaces the controls in the dock
  once the board is solved: Summary, and Next puzzle or Archive.

## Related

- `docs/engine.md` — what each module does.
- `docs/engine-details.md` — the deduce rules and lookahead in detail.
- `docs/encoding.md` — the saved-state / share-URL string format (v0 and v1).
