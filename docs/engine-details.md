# Hint Engine Design - details

## Terminology

| Term               | Meaning                                                                  |
| ------------------ | ------------------------------------------------------------------------ |
| **question type**  | What kind of question it is: CountAnswer, FirstWith, AnswerOf, etc.      |
| **option**         | One of the choices (A–E) for a question                                  |
| **deduction rule** | A specific logical check that can eliminate an option or force an answer |
| **DeduceResult**   | The output of a deduction rule: action (eliminate/force) + which rule    |
| **lookahead**      | Hypothetical reasoning: assume an option, deduce, check answer validity  |

### Functions

| Function                                                  | Returns                 | Purpose                                                                                                            |
| --------------------------------------------------------- | ----------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `checkAnswerValidity(fp, answers, eliminated, qi)`        | Validity                | Is this answer consistent with current state? Used by UI (validity bar) and by lookahead to detect contradictions. |
| `deduce(fp, answers, eliminated)`                         | DeduceResult[]          | Apply deduction rules to find eliminations or forces. Returns all applicable results.                              |
| `deduceFast(fp, answers, eliminated)`                     | DeduceResult[]          | Like deduce, but skips expensive rules (used by lookahead inner loop).                                             |
| `deduceWithRule(fp, answers, eliminated, rule, exclude)`  | DeduceResult[]          | Filter to a specific deduction rule, or exclude one. Used by tests.                                                |
| `lookahead(fp, answers, eliminated)`                      | LookaheadResult or null | Assume an option, deduce, checkAnswerValidity. If invalid → eliminate.                                             |
| `checkQuestionAgainstSolution(fp, qi, selected, answers)` | bool                    | Generation: is the puzzle well-formed for this question?                                                           |
| `solve()` (solve-brute)                                   | Answer[][]              | Brute-force solver, returns all solutions (up to maxSolutions).                                                    |
| `solvePuzzle(fp)`                                         | SolveResult             | Full solve from blank: deduce + lookahead loop. Returns answers + step trace.                                      |
| `checkSolvable(fp)`                                       | "solved" or "stuck"     | Thin wrapper around solvePuzzle — just checks if all questions answered.                                           |
| `checkPuzzleSolved(fp, answers, eliminated)`              | bool                    | Are all questions answered and valid?                                                                              |
| `explainDeduce(puzzle, fp, answers, eliminated, ...)`     | string                  | Human-readable hint text for a deduction result. JS only.                                                          |
| `explainLookahead(puzzle, fp, ...)`                       | string[]                | Multi-step hint text for a lookahead result. JS only.                                                              |
| `explainInvalid(puzzle, fp, answers, eliminated, qi)`     | string                  | Explain why an answer is invalid (red bar). JS only.                                                               |

## Overview

The hint engine has three layers:

1. **checkAnswerValidity** — is each answered question valid, consistent, invalid, or pending?
2. **deduce** — apply deduction rules to eliminate options or force answers
3. **lookahead** — assume an option, deduce, then checkAnswerValidity. If invalid → eliminate.

## Mental model (how a human solves)

1. Look at each question. Apply **deduction rules** to cross out options or determine answers.
2. When no deduction rule helps, try a **hypothetical**: pick an option, assume it, keep deducing.
3. After deducing under the hypothesis, **check answer validity** for all answered questions. If any is invalid, the hypothesis was wrong — cross out that option.
4. If no contradiction found, abandon the hypothesis (don't commit it).

The validity bar next to each question directly reflects **checkAnswerValidity**:

- Green: answer is provably correct (valid) or correct assuming this option (consistent)
- Red: answer is provably wrong (invalid)
- Amber: not enough information yet (pending)

## checkAnswerValidity

For each answered question, check whether the selected option's claim holds against the current state.

Returns one of five values:

- **Neutral** — question is unanswered
- **Valid** — the claim is provably true regardless of this question's answer
- **Consistent** — the claim is true under the assumption that this option is selected, but may not hold for other options
- **Invalid** — the claim is provably false
- **Pending** — not enough information to decide

`isValid()` treats both Valid and Consistent as positive results.

Internally delegates to `checkValueValidity`, which handles the per-question-type logic.

| Question type              | Invalid when                                                           | Valid when                                                |
| -------------------------- | ---------------------------------------------------------------------- | --------------------------------------------------------- |
| CountAnswer (value=V)      | count > V                                                              | count = V and no remaining unknowns can match             |
| CountAnswerBefore/After    | same, within range                                                     | same, within range                                        |
| CountVowel/CountConsonant  | same                                                                   | same                                                      |
| AnswerOf (claims Q2=B)     | Q2 answered and ≠ B                                                    | Q2 = B                                                    |
| LetterDist (claims dist=D) | other answered and actual dist ≠ D                                     | other answered and dist = D                               |
| NoOtherHasAnswer           | same letter appears elsewhere                                          | no other question has this letter                         |
| FirstWith A (claims Q3)    | Q3 answered ≠ A, or earlier Q has A                                    | Q3 = A and no earlier Q has A and all earlier Qs answered |
| LastWith A (claims Q3)     | Q3 answered ≠ A, or later Q has A                                      | Q3 = A and no later Q has A and all later Qs answered     |
| ClosestAfter/ClosestBefore | same pattern as first/last                                             | same                                                      |
| SameAs (claims Q3)         | Q3 answered ≠ this answer                                              | Q3 = this answer                                          |
| OnlySame (claims Q3)       | Q3 answered ≠ this answer                                              | Q3 = this answer and exactly 1 match                      |
| PrevSame (claims Q3)       | Q3 ≥ self, or Q3 answered ≠ this answer, or closer match exists        | Q3 < self, Q3 = this answer, no closer match              |
| NextSame (claims Q3)       | Q3 ≤ self or ≥ n, or Q3 answered ≠ this answer, or closer match exists | Q3 > self, Q3 = this answer, no closer match              |
| ConsecIdent (claims Q3&Q4) | Q3 and Q4 both answered and differ                                     | Q3 = Q4 and exactly one such pair                         |
| ConsecIdent (None)         | any consecutive pair has same answer                                   | all consecutive pairs answered and none match             |
| OnlyOdd A (claims Q3)      | Q3 is even, or Q3 ≠ A, or another odd Q has A                          | Q3 is odd, Q3 = A, no other odd Q has A                   |
| TrueStmt                   | selected claim evaluates false (needs all answered)                    | selected claim is true and all others false               |
| AnswerIsSelf               | never invalid                                                          | always valid                                              |

**Key property**: checkAnswerValidity never modifies state. It's a pure read-only check.

### Types

TS (`src/engine/state.ts`):

```ts
type Validity = "neutral" | "valid" | "consistent" | "invalid" | "pending";
```

Rust (`check_validity.rs`):

```rust
enum Validity { Neutral, Valid, Consistent, Invalid, Pending }
```

## deduce

Apply deduction rules to find actions: eliminate options or force answers. Returns all applicable results as an array (TS) or SmallVec (Rust). Returns empty if nothing can be deduced.

### Variants

- `deduce(fp, answers, eliminated)` — all rules, full cost
- `deduceFast(fp, answers, eliminated)` — skips expensive rules (used by lookahead inner loop)
- `deduceWithRule(fp, answers, eliminated, rule, exclude)` — filter to one specific rule, or exclude a rule. Used by tests.

Rust additionally has `deduce_with_rule_exclude()` as a separate function.

### Deduction rules (in priority order)

**Count saturation** — when a counting rule's count is fully determined:

- count = value → eliminate remaining matches from other questions
- count + remaining = value → all remaining must match, eliminate non-matches

**Vowel/consonant cross-elimination** — if both CountVowel and CountConsonant exist, their values must sum to n. Eliminate options where the complement isn't available.

**Forced values** — only one possibility remains:

- Only 1 non-eliminated option → force it
- AnswerOf: target is answered → force the matching option
- Reverse AnswerOf: another AnswerOf references this one and is answered → force
- LetterDist: referenced question answered, only 1 option satisfies distance → force
- Counting: all in range answered, only 1 option matches → force

**Eliminations** — a specific option is provably wrong:

- Counting: count already exceeds option value, or count + remaining < value
- AnswerOf: target answered with different letter, or claimed letter eliminated from target
- LetterDist: distance doesn't match when other is answered
- Positional (FirstWith, LastWith, ClosestAfter, ClosestBefore): position out of range, position has wrong answer, answer eliminated from target, closer match exists, None but match exists
- PrevSame/NextSame: position out of range, closer match exists
- OnlySame/SameAs: self-reference, target answered differently
- ConsecIdent: pair has different answers, None but pair exists
- OnlyOdd: even position, wrong answer at target, answer eliminated from target, None but odd match exists

**Key property**: deduce never returns contradictions. It only returns eliminations and forces. Contradictions are detected by checkAnswerValidity, not by deduce.

### Types

TS (`src/engine/deduce.ts`):

```ts
type DeduceAction =
  | { type: "force"; qi: number; answer: Answer }
  | { type: "eliminate"; qi: number; oi: number }
  | { type: "eliminateMulti"; questionMask: number; optionMask: number };

interface DeduceResult {
  action: DeduceAction;
  rule: DeduceRule;
}

type DeduceRule = "CountSaturated" | "OnlyOptionLeft" | "AnswerOfForward" | ... ; // ~50 rules
```

Rust (`deduce.rs`):

```rust
enum DeduceAction {
    Force { qi: usize, answer: Answer },
    Eliminate { qi: usize, oi: usize },
    EliminateMulti { question_mask: u16, option_mask: u8 },
}

struct DeduceResult {
    action: DeduceAction,
    rule: DeduceRule,
}
```

## lookahead

When deduce returns empty:

1. For each unanswered question, for each remaining option:
   a. Copy the state
   b. Set this option as the answer
   c. Loop: call deduce (or deduceFast), apply actions, accumulate into chain. Repeat until deduce returns empty.
   d. Also detect contradictions during deduction (force conflicts with existing answer, eliminate targets current answer).
   e. Call checkAnswerValidity on all answered questions (including ones deduced in step c)
   f. If any question is **invalid** or a contradiction was detected → this option is wrong, eliminate it from the original state
2. Return the first elimination found (as a LookaheadResult with the full chain)

Single-assumption: only one option is hypothesized, but the deduction chain can go arbitrarily deep (potentially solving the entire puzzle). No branching — it never assumes a second option on top of the first.

### Types

TS (`src/engine/lookahead.ts`):

```ts
interface LookaheadResult {
  eliminateQi: number;
  eliminateOi: number;
  assumptionQi: number;
  assumptionAnswer: Answer;
  chain: DeduceResult[];
  contradictionQi: number;
}
```

Rust (`lookahead.rs`):

```rust
struct LookaheadResult {
    eliminate_qi: usize,
    eliminate_oi: usize,
    assumption_qi: usize,
    assumption_answer: Answer,
    chain: ArrayVec<DeduceResult, 80>,
    contradiction_qi: usize,
}
```

Rust lookahead additionally takes `stop_deducing_after_n_results` and `fast` parameters.

### Future: prioritization

Currently iterates Q1→Qn, A→E. Could prioritize:

- Questions with fewest remaining options (most constrained)
- Options that seem unlikely (extreme values)
- Questions whose rules reference already-answered questions

## solvePuzzle (solve loop)

The top-level loop that solves a puzzle from blank:

```
loop:
  if all answered: return solved
  results = deduce(state)
  if results non-empty: apply all actions; continue
  result = lookahead(state)
  if result: apply elimination; continue
  return stuck
```

`checkSolvable(fp)` is a thin wrapper: calls `solvePuzzle`, returns `"solved"` or `"stuck"`.

Note: the solve loop never calls checkAnswerValidity directly. It's only used inside lookahead (to detect contradictions in hypothetical states) and by the UI (to show the validity bar).

During normal solving from blank, contradictions should never occur in the main state — every answer is deduced, not guessed. If checkAnswerValidity finds an invalid answer in the main state, it indicates a bug in deduce (it forced a wrong answer) or a bug in the puzzle.

## explain

The explain layer converts results into human-readable hint text. It is JS-only (player-facing) and contains no game logic — just text formatting. Three separate functions in `src/engine/explain.ts`:

### explainDeduce

Formats a DeduceResult into text:

- "Q3 can't be B: says Q5, but Q5 is C not A."
- "Q5 must be A: only option remaining."

### explainLookahead

Produces multi-step hints from a LookaheadResult:

- Step 1: "Try looking at Q3."
- Step 2: "What if Q3 is A?"
- Step 3: "Then Q5 must be B (answer to Q5 matches Q3), and Q7 must be C (only option left) — but Q7 says 2 consonants and there are already 3."
- Step 4: "So Q3 can't be A."

### explainInvalid

Explains why an answer is invalid (for the red validity bar):

- "You said 2 questions have answer A, but there are already 3."

## Generation pipeline

1. **Construct** (`generateConstructive`) — build puzzle: pick solution, place rules, generate distractors
2. **checkQuestionAgainstSolution** — verify each question type is correct for the solution
3. **runHintEngine** — solve from blank using deduce + lookahead loop (like solvePuzzle, but within the generator)
4. **Repair** (`validateAndRepair`) — if runHintEngine gets stuck, tweak distractors, retry
5. **Brute-force solver** (`solve`) — verify exactly 1 solution (safety net)

The hint engine (step 3) is the main filter — most rejected puzzles fail here. Running it before the expensive brute-force solver avoids wasting time checking uniqueness of puzzles that can't be solved by deduction anyway.

The brute-force solver (step 5) is a safety net that runs last, only on puzzles the hint engine already solves. Deduction rules may assume the puzzle has a unique solution — for example, a lookahead that reaches a complete valid solution could accept it as correct. This is only sound if uniqueness is guaranteed. The solver provides that guarantee.

## Architecture

### Separation of concerns

| Component                                                              | Returns                                               | Used by                      |
| ---------------------------------------------------------------------- | ----------------------------------------------------- | ---------------------------- |
| `checkAnswerValidity(fp, answers, eliminated, qi)`                     | Validity (neutral/valid/consistent/invalid/pending)   | UI (validity bar), lookahead |
| `deduce(fp, answers, eliminated)`                                      | DeduceResult[] (action + rule)                        | solve loop, lookahead        |
| `lookahead(fp, answers, eliminated)`                                   | LookaheadResult (elimination + chain + contradiction) | solve loop                   |
| `explainDeduce(...)` / `explainLookahead(...)` / `explainInvalid(...)` | string                                                | UI (hint button), JS only    |

### Dual implementation

Both Rust and TS implement checkAnswerValidity, deduce, and lookahead. The shared test suite verifies they agree.

**Structural differences between TS and Rust:**

| Aspect                 | TS                                   | Rust                                     |
| ---------------------- | ------------------------------------ | ---------------------------------------- |
| Question type          | `QuestionType` (discriminated union) | `QuestionType` (tagged enum)             |
| Question type ID       | `QuestionTypeId` (numeric constants) | `QuestionTypeKind` (fieldless enum)      |
| FlatPuzzle option data | `optionValues[][]` (unified)         | `option_nums[][]` + `option_answers[][]` |
| FlatPuzzle questions   | `questions: FlatQuestion[]`          | `question_types: [QuestionType; MAX_N]`  |
| Validity               | string literals                      | `Validity` enum                          |
| DeduceRule             | string literal union                 | enum (macro-generated)                   |

## Data model

### FlatPuzzle

Pre-computed structure for hot-path performance.

TS (`src/engine/types.ts`):

```ts
interface FlatPuzzle {
  questions: FlatQuestion[]; // flattened question data (type ID + params)
  optionValues: (number | null)[][]; // [qi][oi] → semantic value
  optionClaims: (Claim | null)[][]; // for TrueStmt
  affectedBy: number[][]; // affectedBy[j] = question indices to re-check when Q_j changes
  globalIndices: number[]; // questions with global rules (need all answers)
  n: number;
  optionCount: number;
}
```

Rust (`types.rs`):

```rust
struct FlatPuzzle {
    question_types: [QuestionType; MAX_N],
    option_nums: [[i16; 5]; MAX_N],
    option_answers: [[u8; 5]; MAX_N],
    option_claims: [[Option<Claim>; 5]; MAX_N],
    affected_by: [SmallList; MAX_N],
    global_indices: SmallList,
    n: usize,
    option_count: usize,
}
```

### State

- `answers[qi]`: answer letter or null
- `eliminated[qi]`: bitmask (both TS and Rust)

### Option value semantics

Values in `optionValues` (TS) / `option_nums` (Rust) depend on question type:

- Counting: claimed count (integer)
- Positional: claimed question index (0-based), NONE_VAL for "None"
- LetterDist: claimed distance
- AnswerOf/LeastCommon/MostCommon: letter index (in `optionValues` for TS, `option_answers` for Rust)
- Identity options (NoOtherHasAnswer, EqualCount, AnswerIsSelf): letter index
- TrueStmt: in `optionClaims`
- NAN_VAL: not applicable

## Testing

### Shared test suite

Split across multiple JSON files in `tests/`:

| File                  | Tests                                             |
| --------------------- | ------------------------------------------------- |
| `check-validity.json` | checkAnswerValidity: given state, expect validity |
| `deduce.json`         | deduce with rule filter: expect specific action   |
| `lookahead.json`      | lookahead: expect specific elimination            |
| `evaluate.json`       | checkQuestionAgainstSolution                      |
| `solve.json`          | end-to-end: verify the engine solves a puzzle     |
| `hint-checks.json`    | combined checks (legacy, still present)           |

Deduce tests use the rule filter (`deduceWithRule`) for isolation — no need to carefully craft puzzles where only one deduction rule fires.

### Solvability checking (`pnpm check`)

`scripts/check.ts` — verifies the engine solves every puzzle in a JSON file from blank.
Single-puzzle mode outputs step trace: `1a.2b.3C` (eliminate/force notation).

### Adding a new deduction rule

1. Add to both Rust and TS deduce functions
2. Add a DeduceRule variant to both
3. Add test cases to `tests/deduce.json` (using the rule filter)
4. Run shared tests (both engines)
5. Run solvability check on all puzzle files
6. Add explanation text to `explainDeduce` in `src/engine/explain.ts`

### Adding checkAnswerValidity for a question type

1. Add to both Rust and TS checkAnswerValidity / checkValueValidity functions
2. Add test cases to `tests/check-validity.json`
3. Verify lookahead still works (it depends on checkAnswerValidity)
