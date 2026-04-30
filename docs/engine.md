# Hint Engine Design

## Overview

The hint engine has three layers, each independent and simple:

1. **Validate** — is each answered question valid, invalid, or pending?
2. **Deduce** — can we eliminate an option or force an answer?
3. **Lookahead** — assume an option, deduce, then validate. If invalid → eliminate.

## Mental model (how a human solves)

1. Look at each question. Apply **deduction rules** to cross out options or determine answers.
2. When no rule helps, try a **hypothetical**: pick an option, assume it, keep deducing.
3. After each hypothetical step, **validate** all answered questions. If any is invalid, the hypothesis was wrong — cross out that option.
4. If no contradiction found, abandon the hypothesis (don't commit it).

The validity bar next to each question directly reflects the **validate** layer:
- Green: answer is provably correct
- Red: answer is provably wrong
- Amber: not enough information yet

## Layer 1: Validate

For each answered question, check whether the selected option's claim holds against the current state.

Returns one of three values per question:
- **Valid** — the claim is fully confirmed
- **Invalid** — the claim is provably false
- **Pending** — not enough information to decide

This is intentionally simple — just evaluate the rule against known answers:

| Rule type | Invalid when | Valid when |
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

**Key property**: validate never modifies state. It's a pure read-only check. It can be called at any time without side effects.

## Layer 2: Deduce

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

**Key property**: deduce never returns contradictions. It only returns eliminations and forces. Contradictions are handled by validate, not deduce.

## Layer 3: Lookahead

When deduce returns null:

1. For each unanswered question, for each remaining option:
   a. Copy the state
   b. Set this option as the answer
   c. Loop: call deduce, apply the action, repeat until deduce returns null
   d. Call validate on all answered questions
   e. If any question is **invalid** → this option is wrong, eliminate it from the original state
2. Return the first elimination found

Single-level only: assume one option, deduce consequences, validate. No nested hypotheticals.

### Future: prioritization
Currently iterates Q1→Qn, A→E. Could prioritize:
- Questions with fewest remaining options (most constrained)
- Options that seem unlikely (extreme values)
- Questions whose rules reference already-answered questions

## Solve loop

The top-level loop that solves a puzzle from blank:

```
loop:
  if all answered: return solved
  action = deduce(state)
  if action: apply(action); continue
  action = lookahead(state)  // internally uses deduce + validate
  if action: apply(action); continue
  return stuck
```

Note: the solve loop never calls validate directly. Validate is only used inside lookahead (to detect contradictions in hypothetical states) and by the UI (to show the validity bar).

During normal solving from blank, contradictions should never occur in the main state — every answer is deduced, not guessed. If validate finds an invalid answer in the main state, it indicates a bug in deduce (it forced a wrong answer) or a bug in the puzzle.

## Architecture

### Separation of concerns

| Component | Purpose | Used by |
|---|---|---|
| `validate(state, qi)` | Check if Q's answer is valid/invalid/pending | UI (validity bar), lookahead |
| `deduce(state)` | Find next elimination or force | Solve loop, lookahead |
| `lookahead(state)` | Hypothetical deduction + validation | Solve loop |
| `explain(state, action)` | Human-readable hint text | UI (hint button), JS only |

### Single source of truth
Both Rust and JS implement validate and deduce. The shared test suite (`tests/hint-checks.json`) verifies they agree.

Currently these are mixed into a single `find_action_fast` function. The refactor path:
1. Extract validate as a separate function
2. Remove contradiction checks from deduce (they move to validate)
3. Lookahead calls deduce in a loop, then validate to check for contradictions
4. Solve loop calls deduce, then lookahead. Never calls validate directly.

## Data model

### FlatPuzzle
Pre-computed structure:
- `rules[qi]`: rule type + parameters
- `option_nums[qi][oi]`: numeric value per option (count, position, distance)
- `option_answers[qi][oi]`: letter index for AnswerOf/LeastCommon/MostCommon
- `option_claims[qi][oi]`: claim data for TrueStmt
- `affected_by[qi]`: dependency graph
- `global_indices`: questions with global rules

### State
- `answers[qi]`: answer letter or null
- `eliminated[qi]`: bitmask (Rust) or marks array (JS)

### Option value semantics
Values in `option_nums` depend on rule type:
- Counting: claimed count (integer)
- Positional: claimed question index (0-based), NONE_VAL for "None"
- LetterDist: claimed distance
- AnswerOf/LeastCommon/MostCommon: in `option_answers` (letter index)
- Constrained (Unique, EqualCount, AnswerIsSelf): in `option_answers`
- TrueStmt: in `option_claims`
- NAN_VAL: not applicable

## Testing

### Shared test suite (`tests/hint-checks.json`)
Each test: puzzle + state → expected action. Both engines must agree.

Organized by deduction rule type. Covers happy paths, edge cases, boundary conditions, and no-action cases.

### Solvability checking (`--check` / `check-solvable.mts`)
Verifies the engine solves every puzzle in a JSON file from blank.
Single-puzzle mode outputs step trace: `1a.2b.3C` (eliminate/force notation).

### Adding a new deduction rule
1. Add to both Rust and JS deduce functions
2. Add test cases to `tests/hint-checks.json`
3. Run shared tests (both engines)
4. Run solvability check on all puzzle files
5. Add explanation text to JS (for player-facing hints)

### Adding validation for a rule type
1. Add to both Rust and JS validate functions
2. Add test cases (validate tests, separate from deduce tests)
3. Verify lookahead still works (it depends on validate)

## Implementation notes for refactor

### Deduce with rule filter

`deduce` accepts an optional rule filter so tests can exercise one deduction rule in isolation:

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
enum DeduceRule {
    All,
    CountSaturation,
    VowelConsonantCross,
    ForcedValues,
    Eliminations,
    // ... one per section
}

#[inline(always)]
fn deduce(fp, answers, eliminated) -> Option<Action> {
    deduce_with_rule(fp, answers, eliminated, DeduceRule::All)
}

fn deduce_with_rule(fp, answers, eliminated, rule: DeduceRule) -> Option<Action> {
    if rule == DeduceRule::All || rule == DeduceRule::CountSaturation {
        // count saturation logic
    }
    if rule == DeduceRule::All || rule == DeduceRule::ForcedValues {
        // forced values logic
    }
    // ...
}
```

Production calls `deduce()` — inlined with `All`, compiler eliminates branches. Tests call `deduce_with_rule()` with a specific rule. Zero overhead in production.

JS uses a string parameter (`null` for all, `"count_saturation"` etc. for specific).

### Test format (after refactor)

Three test types in `tests/hint-checks.json`:

**Validate tests** — check validity of an answered question:
```json
{
  "test": "validate",
  "name": "CountAnswer: count exceeds value",
  "qi": 0,
  "puzzle": { ... },
  "state": ["C", "A", "A", ""],
  "expect": "invalid"
}
```

**Deduce tests** — check that a specific rule produces the expected action:
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

**Solve tests** — end-to-end, verify the engine solves a puzzle:
```json
{
  "test": "solve",
  "name": "Simple 4-question puzzle",
  "puzzle": { ... },
  "expect": "solved"
}
```

Each test type exercises one layer. Deduce tests no longer need carefully crafted puzzles where only one check fires — the rule filter handles isolation.

### Current state (pre-refactor)

The code currently mixes validate and deduce into `find_action_fast` / `findActionFp`. The contradiction section IS validate, the rest IS deduce. Refactor steps:

1. Extract validate as a separate function (for each qi: valid/invalid/pending)
2. Remove contradiction checks from deduce
3. Update lookahead: call deduce in a loop, then validate to detect contradictions
4. Add rule filter parameter to deduce
5. Rewrite test suite to the new format
6. Add validate-specific tests
