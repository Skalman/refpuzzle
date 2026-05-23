# Hint Engine Design

All TS engine and generator files have Rust counterparts in `rust/src/` unless noted otherwise.

## Engine (`src/engine/` — shared logic, used by both UI and generator)

- **types.rs / types.ts**

  Core types: `Answer`, `Puzzle`, `QuestionType` (discriminated union), `FlatPuzzle`, `FlatQuestion`, `Claim`. Also `State` (answers + eliminated bitmasks), `OptionPos` (qi + oi pair), `flattenPuzzle()` which pre-computes the flat representation, and numeric `QuestionTypeId` constants for hot-path switches.

- **state.ts** (ts only)

  `Validity` type and constants (`V_NEUTRAL`, `V_VALID`, `V_CONSISTENT`, `V_INVALID`, `V_PENDING`), `isValid()` helper. `deriveState()` converts UI mark arrays into `answers[]` + `eliminated[]` bitmask arrays.

- **check_answer.rs / check-answer.ts**

  `checkAnswer(fp, state, qi)` — is a given answer valid/consistent/invalid/pending? Delegates to `checkClaim(fp, state, opt, claim)` for per-question-type logic. Also `checkAnswers(fp, answers)` — checks all questions and returns bool; used by generation to verify a puzzle is well-formed.

- **deduce.rs / deduce.ts**

  `deduce()`, `deduceFast()`, `deduceWithRule()` — apply deduction rules, return `DeduceResult[]`. Defines `DeduceAction`, `DeduceResult`, `DeduceRule` (all ~50 rule names), and `ALL_DEDUCE_RULES`.

- **lookahead.rs / lookahead.ts**

  `lookahead()` — hypothetical reasoning. Assumes each remaining option, runs deduce loop, checks for contradictions via checkAnswer. Returns `LookaheadResult` or null.

- **solve_deduce.rs / solve-deduce.ts**

  `solvePuzzle()` — full solve loop from blank (deduce + lookahead). `checkSolvable()` — thin wrapper returning `"solved"` or `"stuck"`. `checkPuzzleSolved()` — checks all questions are answered and valid.

- **evaluate.ts** (ts only; Rust: private helpers in `build.rs`)

  `evaluateClaim()` used by TrueStmt question type. On the Rust side, the equivalent `evaluate_claim` / `evaluate_claim_ext` live as private functions in `build.rs` since puzzle construction doesn't have a full `FlatPuzzle` yet.

- **explain.ts** (ts only)

  Human-readable hint text (player-facing). `explainDeduce()` — formats a deduction step. `explainLookahead()` — multi-step hint for hypothetical reasoning. `explainInvalid()` — explains why an answer shows a red bar.

- **render.ts** (ts only)

  Text rendering for puzzle display. `renderQuestionText()` — question stem text. `renderOptionLabel()` — option value label. `renderClaimLabel()` — TrueStmt claim text.

- **tutorial.ts** (ts only)

  `collectTutorialSteps()` — runs the deduce engine on a puzzle and collects steps as `TutorialStep[]` for the tutorial animation. Produces intro steps and deduce steps with explanations.

- **format.rs / format.ts**

  `formatTypeTag()` — debug-friendly string representation of a QuestionType (e.g. `"CountAnswer(A)"`). Used by tracing and error messages.

- **serialize.rs / serialize.ts**

  Deserialization helpers. `parsePuzzle()` converts compact JSON into a `FlatPuzzle`.

- **check_form.rs / check-form.ts**

  `checkForm()` — structural validation of a puzzle definition (reference bounds, duplicate option values, missing fields, etc.). Returns `FormError[]` with severity. Catches authoring mistakes that aren't logic errors.

## Generator (`src/generator/` — CLI-only, not bundled in frontend)

- **construct.rs + build.rs / construct.ts**

  `generateConstructive()` — builds a puzzle: picks a random solution, places question types, fills options with distractors. `validateAndRepair()` — runs checkAnswers, then `runHintEngine` (deduce+lookahead solve loop), then repair if stuck, then brute-force uniqueness check. Rust splits this: `construct.rs` has puzzle construction, `build.rs` has validate/repair/fill helpers + `Stats` for performance tracing.

- **solve_brute.rs / solve-brute.ts**

  `solve()` — brute-force backtracking solver. Returns all valid solutions up to a max (default 2). Used as a safety net to verify exactly 1 solution exists.

- **difficulty.rs / difficulty.ts**

  `DifficultyProfile` definitions — per-level settings: question count, option count, allowed question types.

- **rng.rs / rng.ts**

  `RNG` class — seeded PRNG (hash-based). Deterministic generation from a given seed.

- **trace.ts** (Rust: inline in `build.rs`)

  Structured JSON tracing for generation steps. Emits events (attempt, question placed, hint engine step, repair) to stderr for debugging and cross-engine diffing.

## Scripts (`scripts/` for TS, `rust/src/` for Rust)

- **main.rs / generate.ts** — `pnpm generate` / `cargo run` — CLI for generating puzzles. Takes date ranges, reads difficulty profiles, runs generation loop, writes compact year JSON files.

- **generate-puzzles.ts** — Older generator script for individual puzzles (by level/seed/count). Writes to `src/puzzles/generated/`.

- **check.rs / check.ts** — `pnpm check` — solvability checker. Verifies the engine solves every puzzle in a JSON file from blank. Single-puzzle mode outputs step trace. Also runs brute-force solver for cross-validation. Rust side also handles format-check and check-form commands.

- **test.ts** — `pnpm test` — test runner. Runs JSON test suites (`tests/*.json`) against the TS engine. Tests checkAnswer, deduce (with rule filter), lookahead, solve.

- **test-check-answer.mts** — Standalone runner for `tests/check-answer.json`.

- **test-deduce.mts** — Standalone runner for `tests/deduce.json`.

- **test-lookahead.mts** — Standalone runner for `tests/lookahead.json`.

- **test-solve.mts** — Standalone runner for `tests/solve.json`.

- **bench.ts** — `pnpm bench` — benchmarks `generateConstructive()` across multiple seeds for a given level.

- **regenerate.sh** — Shell script to regenerate puzzle JSON files, preserving days before a cutoff date.

- **check-diff.ts** — Runs both Rust and TS `check` on a puzzle file and diffs the output to catch engine divergence.

- **trace-diff.ts** — Runs both Rust and TS generation with `--trace` and diffs step-by-step output to catch divergence.

- **gen-logo.ts** — Generates the SVG logo file.

## Test data (`tests/`)

- **check-answer.json** — Test cases for `checkAnswer`: given a puzzle + state, expect a validity result.

- **deduce.json** — Test cases for `deduce` with rule filter: given a puzzle + state + rule, expect a specific action.

- **lookahead.json** — Test cases for `lookahead`: given a puzzle + state, expect a specific elimination.

- **evaluate.json** — Test cases for `checkQuestionAgainstSolution`.

- **solve.json** — End-to-end tests: verify the engine solves a puzzle from blank.

- **check-form.json** — Test cases for `checkForm`: given a puzzle, expect specific form errors.
