# Hint Engine Design

## Terminology

| Term | Meaning |
|---|---|
| **question type** | What kind of question it is: count_answer, first_with_answer, etc. |
| **option** | One of the 5 choices (A–E) for a question |
| **deduction rule** | A specific logical check that can eliminate an option or force an answer |
| **DeduceResult** | The output of a deduction rule: action (eliminate/force) + which rule + why |
| **lookahead** | Hypothetical reasoning: assume an option, deduce, check answer validity |

### Functions

| Function | Returns | Purpose |
|---|---|---|
| `checkAnswerValidity(state, qi)` | valid / invalid / pending | Is this answer consistent with current state? Used by UI (validity bar) and by lookahead to detect contradictions. |
| `deduce(state)` | DeduceResult or null | Apply deduction rules to find next elimination or force. |
| `lookahead(state)` | LookaheadResult or null | Assume an option, deduce, checkAnswerValidity. If invalid → eliminate. |
| `checkQuestionAgainstSolution(puzzle, qi, solution)` | bool | Generation: is the puzzle well-formed for this question? |
| `checkUniqueSolution(puzzle)` | bool | Generation: brute-force solver, exactly 1 solution? |
| `checkSolvable(puzzle)` | bool | Generation: can the engine solve from blank? Uses deduce + lookahead (which uses checkAnswerValidity). |
| `checkPuzzleSolved(state)` | bool | Are all questions answered and valid? |
| `explain(DeduceResult or LookaheadResult)` | string | Human-readable hint text. JS only. |

### Legacy names in current code

| Current code | This doc | Notes |
|---|---|---|
| `Rule` / `RuleKind` | question type | e.g. `Rule::CountAnswer` = the count_answer question type |
| `find_action_fast` / `findActionFp` | deduce | currently also contains checkAnswerValidity (contradiction checks) |
| `Action::Contradiction` | checkAnswerValidity result | will be separated out |
| `Action::Eliminate` / `Action::Force` | deduce action | stays in deduce |
| `find_lookahead_action` / `findLookahead` | lookahead | |
| `canEliminate` / `findEliminable` | explain (JS) | duplicate logic, will be removed |
| `findSimpleDeduction` | deduce + explain | currently delegates to findActionFast |
| `option_nums` / `option_answers` | option values | semantic meaning depends on question type |
| `evaluate()` | checkQuestionAgainstSolution | generation only, full rule evaluation |
| `validate_and_check()` | checkSolvable pipeline | generation: evaluate + solver + hint engine + repair |

## Overview

The hint engine has three layers:

1. **checkAnswerValidity** — is each answered question valid, invalid, or pending?
2. **deduce** — apply deduction rules to eliminate an option or force an answer
3. **lookahead** — assume an option, deduce, then checkAnswerValidity. If invalid → eliminate.

## Mental model (how a human solves)

1. Look at each question. Apply **deduction rules** to cross out options or determine answers.
2. When no deduction rule helps, try a **hypothetical**: pick an option, assume it, keep deducing.
3. After deducing under the hypothesis, **check answer validity** for all answered questions. If any is invalid, the hypothesis was wrong — cross out that option.
4. If no contradiction found, abandon the hypothesis (don't commit it).

The validity bar next to each question directly reflects **checkAnswerValidity**:
- Green: answer is provably correct (valid)
- Red: answer is provably wrong (invalid)
- Amber: not enough information yet (pending)

## checkAnswerValidity

For each answered question, check whether the selected option's claim holds against the current state.

Returns one of three values per question:
- **Valid** — the claim is fully confirmed
- **Invalid** — the claim is provably false
- **Pending** — not enough information to decide

This is intentionally simple — just evaluate the question type against known answers:

| Question type | Invalid when | Valid when |
|---|---|---|
| count_answer (value=V) | count > V | count = V and no remaining unknowns can match |
| count_answer_before/after | same, within range | same, within range |
| count_vowel/consonant | same | same |
| answer_of_question (claims Q2=B) | Q2 answered and ≠ B | Q2 = B |
| letter_distance (claims dist=D) | other answered and actual dist ≠ D | other answered and dist = D |
| unique_answer | same letter appears elsewhere | no other question has this letter |
| first_with_answer A (claims Q3) | Q3 answered ≠ A, or earlier Q has A | Q3 = A and no earlier Q has A and all earlier Qs answered |
| last_with_answer A (claims Q3) | Q3 answered ≠ A, or later Q has A | Q3 = A and no later Q has A and all later Qs answered |
| closest_after/before | same pattern as first/last | same |
| same_answer_as (claims Q3) | Q3 answered ≠ this answer | Q3 = this answer |
| only_same_answer (claims Q3) | Q3 answered ≠ this answer | Q3 = this answer and exactly 1 match |
| previous_same_answer (claims Q3) | Q3 ≥ self, or Q3 answered ≠ this answer, or closer match exists | Q3 < self, Q3 = this answer, no closer match |
| next_same_answer (claims Q3) | Q3 ≤ self or ≥ n, or Q3 answered ≠ this answer, or closer match exists | Q3 > self, Q3 = this answer, no closer match |
| consecutive_identical (claims Q3&Q4) | Q3 and Q4 both answered and differ | Q3 = Q4 and exactly one such pair |
| consecutive_identical (None) | any consecutive pair has same answer | all consecutive pairs answered and none match |
| only_odd_with_answer A (claims Q3) | Q3 is even, or Q3 ≠ A, or another odd Q has A | Q3 is odd, Q3 = A, no other odd Q has A |
| only_true_statement | selected claim evaluates false (needs all answered) | selected claim is true and all others false |
| answer_is_self | never invalid | always valid |

**Key property**: checkAnswerValidity never modifies state. It's a pure read-only check.

### Return type

```
CheckAnswerValidityResult =
  | Valid { reason }      // green bar
  | Invalid { reason }    // red bar
  | Pending               // amber bar
```

The `reason` explains why (e.g. "count is 3 but you said 2", "Q3 is C not A"). Used by explain for error messages and by lookahead to explain contradictions.

## deduce

Apply deduction rules to find one action: eliminate an option or force an answer. Returns null if nothing can be deduced.

### Deduction rules (in priority order)

**Count saturation** — when a counting rule's count is fully determined:
- count = value → eliminate remaining matches from other questions
- count + remaining = value → all remaining must match, eliminate non-matches

**Vowel/consonant cross-elimination** — if both count_vowel and count_consonant exist, their values must sum to n. Eliminate options where the complement isn't available.

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

### Return type

```
DeduceResult {
    action: Action               // eliminate Q3 option B, or force Q5=A
    rule: DeduceRule             // "count_saturation", "forced_answer_of", etc.
    reason: Reason               // rule-specific detail (shared with checkAnswerValidity)
}
```

### Rule filter for testing

`deduce` accepts an optional rule filter so tests can exercise one deduction rule in isolation:

```rust
fn deduce(fp, answers, eliminated) -> Option<DeduceResult> {
    deduce_with_rule(fp, answers, eliminated, DeduceRule::All)
}

#[inline(always)]
fn deduce_with_rule(fp, answers, eliminated, rule: DeduceRule) -> Option<DeduceResult> {
    if rule == DeduceRule::All || rule == DeduceRule::CountSaturation {
        // ...
    }
    if rule == DeduceRule::All || rule == DeduceRule::ForcedValues {
        // ...
    }
    // ...
}
```

Production calls `deduce()` which passes `All`. `#[inline(always)]` on `deduce_with_rule` ensures the constant propagates and the compiler eliminates dead branches. Tests call `deduce_with_rule()` directly with a specific rule.

JS uses a string parameter (`null` for all, `"count_saturation"` etc. for specific).

## lookahead

When deduce returns null:

1. For each unanswered question, for each remaining option:
   a. Copy the state
   b. Set this option as the answer
   c. Loop: call deduce, apply the action, accumulate into chain. Repeat until deduce returns null.
   d. Call checkAnswerValidity on all answered questions (including ones deduced in step c)
   e. If any question is **invalid** → this option is wrong, eliminate it from the original state
2. Return the first elimination found (as a LookaheadResult with the full chain)

Single-assumption: only one option is hypothesized, but the deduction chain can go arbitrarily deep (potentially solving the entire puzzle). No branching — it never assumes a second option on top of the first.

### Return type

```
LookaheadResult {
    eliminate: (qi, oi)                // what to eliminate from the real state
    assumption: (qi, Answer)           // "assume Q3=A"
    chain: [DeduceResult, ...]         // deductions made under the assumption
    contradiction: (qi, CheckAnswerValidityResult)  // which question became invalid and why
}
```

### Future: prioritization
Currently iterates Q1→Qn, A→E. Could prioritize:
- Questions with fewest remaining options (most constrained)
- Options that seem unlikely (extreme values)
- Questions whose rules reference already-answered questions

## checkSolvable (solve loop)

The top-level loop that solves a puzzle from blank:

```
loop:
  if all answered: return solved
  result = deduce(state)
  if result: apply(result.action); continue
  result = lookahead(state)  // internally uses deduce + checkAnswerValidity
  if result: apply(result.eliminate); continue
  return stuck
```

Note: the solve loop never calls checkAnswerValidity directly. It's only used inside lookahead (to detect contradictions in hypothetical states) and by the UI (to show the validity bar).

During normal solving from blank, contradictions should never occur in the main state — every answer is deduced, not guessed. If checkAnswerValidity finds an invalid answer in the main state, it indicates a bug in deduce (it forced a wrong answer) or a bug in the puzzle.

## explain

The explain layer converts results into human-readable hint text. It is JS-only (player-facing) and contains no game logic — just text formatting.

### From deduce

`explain(DeduceResult)` formats the action + rule + reason into text:

- "Q3 can't be B: says Q5, but Q5 is C not A."
- "Q5 must be A: only option remaining."

### From lookahead

`explain(LookaheadResult)` produces multi-step hints:

- Step 1: "Try looking at Q3."
- Step 2: "What if Q3 is A?"
- Step 3: "Then Q5 must be B (answer to Q5 matches Q3), and Q7 must be C (only option left) — but Q7 says 2 consonants and there are already 3."
- Step 4: "So Q3 can't be A."

### From checkAnswerValidity

When the player asks "why is this red?", explain formats the CheckAnswerValidityResult's reason:

- "You said 2 questions have answer A, but there are already 3."

### Shared Reason type

`DeduceResult` and `CheckAnswerValidityResult` share the same `Reason` type — many reasons apply to both (e.g. `CountExceedsValue` explains both "this option is wrong" in deduce and "this answer is wrong" in checkAnswerValidity).

## Generation pipeline

1. **Construct** — build puzzle: pick solution, place rules, generate distractors
2. **checkQuestionAgainstSolution** — verify each question type is correct for the solution
3. **checkSolvable** — verify puzzle is solvable by deduction from blank (uses deduce + lookahead)
4. **Repair** — if checkSolvable fails, tweak distractors, retry
5. **checkUniqueSolution** — brute-force solver, verify exactly 1 solution (safety net)

The hint engine (step 3) is the main filter — most rejected puzzles fail here. Running it before the expensive brute-force solver avoids wasting time checking uniqueness of puzzles that can't be solved by deduction anyway.

The brute-force solver (step 5) is a safety net that runs last, only on puzzles the hint engine already solves. Deduction rules may assume the puzzle has a unique solution — for example, a lookahead that reaches a complete valid solution could accept it as correct. This is only sound if uniqueness is guaranteed. The solver provides that guarantee.

## Architecture

### Separation of concerns

| Component | Returns | Used by |
|---|---|---|
| `checkAnswerValidity(state, qi)` | valid/invalid/pending + reason | UI (validity bar), lookahead |
| `deduce(state)` | DeduceResult (action + rule + reason) | checkSolvable loop, lookahead |
| `lookahead(state)` | LookaheadResult (elimination + chain + contradiction) | checkSolvable loop |
| `explain(result)` | string | UI (hint button), JS only |

### Single source of truth
Both Rust and JS implement checkAnswerValidity and deduce. The shared test suite (`tests/hint-checks.json`) verifies they agree.

Currently these are mixed into a single `find_action_fast` function. The refactor path:
1. Extract checkAnswerValidity as a separate function
2. Remove contradiction checks from deduce (they move to checkAnswerValidity)
3. Update lookahead: call deduce in a loop, then checkAnswerValidity to detect contradictions
4. Add rule filter parameter to deduce
5. Rewrite test suite to the new format
6. Add checkAnswerValidity-specific tests

## Data model

### FlatPuzzle
Pre-computed structure:
- `rules[qi]`: question type + parameters
- `option_nums[qi][oi]`: numeric value per option (count, position, distance)
- `option_answers[qi][oi]`: letter index for AnswerOf/LeastCommon/MostCommon
- `option_claims[qi][oi]`: claim data for TrueStmt
- `affected_by[qi]`: dependency graph
- `global_indices`: questions with global question types

### State
- `answers[qi]`: answer letter or null
- `eliminated[qi]`: bitmask (Rust) or marks array (JS)

### Option value semantics
Values in `option_nums` depend on question type:
- Counting: claimed count (integer)
- Positional: claimed question index (0-based), NONE_VAL for "None"
- LetterDist: claimed distance
- AnswerOf/LeastCommon/MostCommon: in `option_answers` (letter index)
- Constrained (Unique, EqualCount, AnswerIsSelf): in `option_answers`
- TrueStmt: in `option_claims`
- NAN_VAL: not applicable

## Testing

### Shared test suite (`tests/hint-checks.json`)

Three test types:

**checkAnswerValidity tests** — check validity of an answered question:
```json
{
  "test": "checkAnswerValidity",
  "name": "CountAnswer: count exceeds value",
  "qi": 0,
  "puzzle": { ... },
  "state": ["C", "A", "A", ""],
  "expect": "invalid"
}
```

**deduce tests** — check that a specific deduction rule produces the expected action:
```json
{
  "test": "deduce",
  "rule": "count_saturation",
  "name": "Count met, eliminate remaining matches",
  "puzzle": { ... },
  "state": ["B", "A", "", ""],
  "expect": "3a"
}
```

**solve tests** — end-to-end, verify the engine solves a puzzle:
```json
{
  "test": "solve",
  "name": "Simple 4-question puzzle",
  "puzzle": { ... },
  "expect": "solved"
}
```

Each test type exercises one layer. Deduce tests use the rule filter for isolation — no need to carefully craft puzzles where only one deduction rule fires.

### Solvability checking (`--check` / `check-solvable.mts`)
Verifies the engine solves every puzzle in a JSON file from blank.
Single-puzzle mode outputs step trace: `1a.2b.3C` (eliminate/force notation).

### Adding a new deduction rule
1. Add to both Rust and JS deduce functions
2. Add a DeduceRule enum variant
3. Add test cases to `tests/hint-checks.json` (using the rule filter)
4. Run shared tests (both engines)
5. Run solvability check on all puzzle files
6. Add explanation text to JS explain layer

### Adding checkAnswerValidity for a question type
1. Add to both Rust and JS checkAnswerValidity functions
2. Add test cases (checkAnswerValidity tests)
3. Verify lookahead still works (it depends on checkAnswerValidity)
