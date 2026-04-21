# Logiquiz - Implementation Plan

## Overview

A database-less, client-side puzzle webapp where players solve self-referential quizzes. Each quiz has questions whose answers depend on the quiz itself. Every question has 5 alternatives (A-E). The puzzle has exactly one valid solution. Puzzles are auto-generated at build time across 5 difficulty levels.

## Tech Stack (from rookie2026)

| Layer | Technology |
|-------|-----------|
| Framework | Preact + preact-iso |
| Build | Vite 8 |
| Language | TypeScript (strict) |
| Styling | Plain CSS |
| Linting | Oxlint + oxfmt |
| Testing | Playwright E2E |
| Package manager | pnpm |

**Key difference from rookie2026:** No backend. No PHP, no SQLite, no API. Everything runs client-side. Puzzles are auto-generated at build time and bundled as TypeScript data files.

## Architecture

```
logiquiz/
├── index.html                    # Single entry point
├── public/
│   ├── manifest.json             # PWA manifest
│   └── icon.svg
├── src/
│   ├── main.tsx                  # Entry point
│   ├── App.tsx                   # Router (puzzle list, play puzzle, about)
│   ├── index.css                 # Global styles (light + dark themes)
│   │
│   ├── components/
│   │   ├── PuzzleList.tsx        # List available puzzles with difficulty + completion
│   │   ├── PuzzleView.tsx        # Main puzzle play view
│   │   ├── QuestionRow.tsx       # Single question with clickable options + validity bar
│   │   ├── OptionButton.tsx      # A single clickable option (unmarked/incorrect/correct)
│   │   ├── HintBar.tsx           # Contradiction highlights + hint controls
│   │   └── About.tsx             # About/how-to-play page
│   │
│   ├── engine/
│   │   ├── types.ts              # Core types (Puzzle, Question, Answer state)
│   │   ├── validate.ts           # Validation engine - checks all answers against rules
│   │   └── evaluators.ts         # One evaluator per question type
│   │
│   ├── generator/
│   │   ├── generate.ts           # Puzzle generator entry point (run via script)
│   │   ├── solver.ts             # Brute-force solver: verify exactly 1 solution exists
│   │   ├── templates.ts          # Question templates per difficulty tier
│   │   ├── assemble.ts           # Picks questions, assigns options, checks solvability
│   │   └── difficulty.ts         # Difficulty profiles (question count, allowed types, constraints)
│   │
│   ├── puzzles/
│   │   ├── index.ts              # Puzzle registry (exports all generated puzzles)
│   │   └── generated/            # Output of generator (committed to repo)
│   │       ├── level-1.ts        # 4 questions - Beginner
│   │       ├── level-2.ts        # 5 questions - Easy
│   │       ├── level-3.ts        # 8 questions - Medium
│   │       ├── level-4.ts        # 10 questions - Hard
│   │       └── level-5.ts        # 12 questions - Expert
│   │
│   ├── i18n/
│   │   ├── index.ts              # i18n setup, locale detection, t() function
│   │   └── en.ts                 # English strings (all UI text)
│   │
│   └── lib/
│       ├── store.ts              # localStorage - save/load puzzle progress
│       └── share.ts              # Encode/decode puzzle state for URL sharing
│
├── scripts/
│   └── generate-puzzles.ts       # CLI script: node --import tsx scripts/generate-puzzles.ts
│
├── package.json
├── vite.config.ts
├── tsconfig.json
├── tsconfig.app.json
├── tsconfig.node.json
├── oxlint.config.ts
└── CLAUDE.md
```

## Core Data Model

### Puzzle Definition

```typescript
type AnswerLetter = 'A' | 'B' | 'C' | 'D' | 'E'

interface Puzzle {
  id: string                      // URL slug, e.g. "starter-4"
  title: string
  difficulty: 1 | 2 | 3 | 4 | 5   // 5 levels
  questions: QuestionDef[]
}

interface QuestionDef {
  text: string                     // e.g. "How many questions have answer C?"
  options: OptionDef[]             // exactly 5, one per A-E
  rule: ValidationRule             // how to check correctness
}

// Options come in two flavors:
type OptionDef = SimpleOption | StatementOption

// Most questions: option label is a simple value (number, letter, "None")
interface SimpleOption {
  label: string                    // display text, e.g. "3", "A", "None"
}

// "Which statement is the only true statement?" — each option is an evaluable claim
interface StatementOption {
  label: string                    // display text, e.g. "How many questions have answer C? 1"
  claim: Claim                     // machine-evaluable version of the statement
}

// A Claim is a statement that can be checked true/false against current answers
type Claim =
  | { type: 'count_answer_equals'; answer: AnswerLetter; value: number }
    // "How many questions have answer C? 1" → true if count(C) == 1
  | { type: 'count_consonant_answers_equals'; value: number }
    // "How many questions have a consonant as the answer? 11"
    // Consonants: B, C, D.  Vowels: A, E.
  | { type: 'count_vowel_answers_equals'; value: number }
    // "How many questions have a vowel as the answer? 3"
  | { type: 'count_answer_after_equals'; answer: AnswerLetter; afterIndex: number; value: number }
    // "How many questions after #5 have answer A? 7"
  | { type: 'count_answer_before_equals'; answer: AnswerLetter; beforeIndex: number; value: number }
    // "How many questions before #5 have answer B? 1"
```

### Validation Rules

Each question type maps to a validation function. The rule encodes what makes the selected answer correct/incorrect for that question, given all current answers.

**Simple rules** — the selected option's label is compared against a computed value:

```typescript
type ValidationRule =
  | { type: 'count_answer'; answer: AnswerLetter }
    // "How many questions have answer C?"
    // Correct when: option label == count of questions answered C

  | { type: 'answer_of_question'; questionIndex: number }
    // "What is the answer to question #N?"
    // Correct when: selected option label matches answer letter of question N

  | { type: 'closest_after'; afterIndex: number; answer: AnswerLetter }
    // "Which is the closest question after #N that has answer X?"

  | { type: 'closest_before'; beforeIndex: number; answer: AnswerLetter }
    // "Which is the closest question before #N that has answer X?"

  | { type: 'first_with_answer'; answer: AnswerLetter }
    // "Which is the first question with answer X?"

  | { type: 'last_with_answer'; answer: AnswerLetter }
    // "Which is the last question with answer X?"

  | { type: 'letter_distance'; otherQuestionIndex: number }
    // "How many letters away is the answer from question #N's answer?"
    // e.g. A→C = 2

  | { type: 'least_common_answer' }
    // "Which is the least common answer?"

  | { type: 'most_common_answer' }
    // "Which is the most common answer?"

  | { type: 'most_common_count' }
    // "How many times does the most common answer occur?"

  | { type: 'count_answer_before'; answer: AnswerLetter; beforeIndex: number }
    // "How many questions before #N have answer X?"

  | { type: 'count_answer_after'; answer: AnswerLetter; afterIndex: number }
    // "How many questions after #N have answer X?"

  | { type: 'previous_same_answer' }
    // "Which is the previous question with the same answer as this one?"
```

**Compound rule** — each option is a statement; exactly one must be true:

```typescript
  | { type: 'only_true_statement' }
    // "Which statement is the only true statement?"
    // Validation: evaluate each option's `claim` against current answers.
    // The selected option is correct when:
    //   1. Its claim evaluates to TRUE, AND
    //   2. All other options' claims evaluate to FALSE
    // Options MUST be StatementOption (with claim field).
```

The `only_true_statement` evaluator iterates all 5 options, evaluates each `Claim`, and checks the "exactly one true" invariant. The `Claim` types reuse the same logic as the simple evaluators (counting answers, etc.) but return a boolean against a specific claimed value rather than computing the value for comparison.

### Example: "Which statement is the only true statement?"

```typescript
{
  text: 'Which statement is the only true statement?',
  options: [
    { label: 'How many questions have answer C? 1',
      claim: { type: 'count_answer_equals', answer: 'C', value: 1 } },
    { label: 'How many questions have a consonant as the answer? 11',
      claim: { type: 'count_consonant_answers_equals', value: 11 } },
    { label: 'How many questions have answer E? 4',
      claim: { type: 'count_answer_equals', answer: 'E', value: 4 } },
    { label: 'How many questions after #5 have answer A? 7',
      claim: { type: 'count_answer_after_equals', answer: 'A', afterIndex: 4, value: 7 } },
    { label: 'How many questions after #2 have answer B? 1',
      claim: { type: 'count_answer_after_equals', answer: 'B', afterIndex: 1, value: 1 } },
  ],
  rule: { type: 'only_true_statement' },
}
```

### Player State

```typescript
interface PuzzleState {
  puzzleId: string
  // For each question: null (untouched) or the selected answer + mark state
  answers: (null | { letter: AnswerLetter; marked: 'incorrect' | 'correct' })[]
  completed: boolean
  startedAt: number               // timestamp
  completedAt?: number
}
```

## UI Interaction Model

### Option Click Cycle

Each option button cycles through 3 states on click:

1. **Unmarked** (default) - neutral appearance
2. **Marked incorrect** (first click) - ~~strikethrough~~ / dimmed / red-ish
3. **Marked correct** (second click from unmarked, or click on another option) - highlighted / bold / green-ish
4. Clicking a "correct" option returns it to **unmarked**

Only one option per question can be marked "correct" at a time. Marking a new option as correct automatically unmarks the previous one.

### Validity Bar

Each question has a thin colored bar on the side:
- **Gray** - no answer marked correct yet
- **Green** - the marked-correct answer is logically valid given all current answers
- **Red** - the marked-correct answer is logically invalid

The bar updates live as any answer changes anywhere in the puzzle.

### Win Condition

The puzzle is solved when every question has exactly one "correct" mark AND all validity bars are green (all answers are mutually consistent).

## Validation Engine

The engine runs on every state change:

```
for each question:
  if no answer marked correct → validity = 'neutral'
  else:
    evaluate(question.rule, allCurrentAnswers) → true/false
    validity = true ? 'valid' : 'invalid'

puzzle complete = every question has validity === 'valid'
```

Each evaluator function takes `(questionIndex, selectedAnswer, allAnswers[])` and returns boolean.

The evaluators are pure functions with no side effects — they just inspect the current answer state.

### Edge Cases in Evaluation

**Unanswered questions affect counting rules.** When evaluating "How many questions have answer C?", unanswered questions are _not_ counted as having any answer. This means a counting rule can show green even if the puzzle is incomplete — it's valid _given the current state_. This is correct and intentional: the validity bar reflects whether your answer is consistent with what you've filled in so far, not whether the puzzle is complete.

**Ties in least/most common.** "Which is the least common answer?" — if two answers tie for least common, both are valid answers. The generator must ensure the solution has no ties (or the question wouldn't have a unique answer). But during solving, with partial answers, ties can exist temporarily. The evaluator should accept any tied answer as valid — this avoids misleading red bars mid-solve.

**"None" / no match cases.** "Which is the closest question after #10 that has answer A?" — if no question after #10 has answer A, the correct answer is "None". Options must always include a "None" alternative for positional rules. The evaluator returns the "None" option index when the search finds no match.

**`only_true_statement` circularity.** This question type is not circular — the claims inside its options reference _other_ questions' answers, never this question itself. The generator enforces this constraint. The evaluator checks: (1) the selected option's claim is true, (2) all other options' claims are false. If not all other questions are answered yet, unanswered questions are treated as having no answer (same as counting rules).

**`previous_same_answer` self-reference.** "Which is the previous question with the same answer as this one?" — this question knows its own answer (the selected option letter), so it searches backward for another question with the same letter. If this is question 1, or no prior question has the same answer, the answer is "None".

## Pages / Routes

| Route | Component | Description |
|-------|-----------|-------------|
| `/` | PuzzleList | Grid of available puzzles with difficulty, completion status |
| `/puzzle/:id` | PuzzleView | The main puzzle play view |
| `/about` | About | How to play, how to solve, about Logiquiz |

## Puzzle Generator

The generator is a CLI script that creates puzzles and writes them to `src/puzzles/generated/`. Generated files are committed to the repo so there's no runtime generation cost.

### Why Generation Is Hard

A self-referential quiz is a **fixed-point problem**: a valid solution is an assignment of letters A-E where every question's answer, when evaluated against the full assignment, agrees with itself. We need to generate puzzles with **exactly one** fixed point.

Key difficulties:
1. **Circular dependencies.** "How many questions have answer C?" — the answer depends on all other answers, which depend on this one. There's no topological order to evaluate questions in.
2. **Option values determine which letter is correct.** For `count_answer` with options `[0,1,2,3,4]`, if the count of C is 2, the answer _must_ be C (third option). We can't independently choose the answer letter — it's determined by the option-value mapping AND the solution state.
3. **Some question types have fixed options.** `least_common_answer` always has options `[A,B,C,D,E]`, and the answer IS the least common letter. The answer is fully determined by the global solution — no option-engineering possible.
4. **`only_true_statement` is doubly circular.** Its options contain evaluable claims about the solution, but the solution includes this question's answer.
5. **Unique solvability ≠ human solvability.** A puzzle can have exactly 1 solution but require pure trial-and-error to find. Good puzzles need logical deduction paths.

### The Core Insight: Solution-First Construction

Random assembly (pick questions, pick options, hope for 1 solution) is hopelessly inefficient. Instead: **choose the solution first, then engineer questions to fit it.**

We categorize question types by how much freedom we have over which letter becomes the correct answer:

| Category | Types | Why |
|----------|-------|-----|
| **Flexible** — option values can be shifted to make ANY letter correct | count_answer, closest_after/before, first/last_with_answer, letter_distance, most_common_count, count_answer_before/after, previous_same_answer | The correct value is a number or position. We choose which option slot it lands in by shifting the option range. |
| **Constrained** — the answer letter is determined by the solution | least_common_answer, most_common_answer, answer_of_question | Options are always `[A,B,C,D,E]`. The correct letter is whatever the solution dictates. |
| **Compound** — requires special two-pass handling | only_true_statement | Claims must be engineered after the rest of the solution is known. |

### Algorithm: Solution-First with Fixed-Point Resolution

```
generate(difficulty, rng):

  // ══════════════════════════════════════════════
  // STEP 1: Choose question types
  // ══════════════════════════════════════════════
  Pick N question types from the difficulty's allowed pool.
  Constraints:
    - Max 40% of any single type
    - At least 1 entry-point type (see below)
    - Place only_true_statement last (it's constructed in step 5)
  Assign parameters (referenced indices, referenced letters) randomly.
  Avoid self-references and circular answer_of_question chains.

  // ══════════════════════════════════════════════
  // STEP 2: Choose a target solution
  // ══════════════════════════════════════════════
  Start with a random assignment S: [A-E] for each of the N questions.

  // ══════════════════════════════════════════════
  // STEP 3: Reconcile constrained questions (fixed-point iteration)
  // ══════════════════════════════════════════════
  Repeat until stable (max 20 iterations):
    For each CONSTRAINED question i:
      Compute what S[i] SHOULD be given S:
        - answer_of_question(j): S[i] ← S[j]
        - most_common_answer:    S[i] ← most frequent letter in S
        - least_common_answer:   S[i] ← least frequent letter in S
      Update S[i] if it changed.

  If not stable after 20 iterations → discard, retry from step 1.
  (Oscillation is rare. Example: most_common flips between B and C
   when they're tied. Most starting points converge in 2-3 iterations.)

  // ══════════════════════════════════════════════
  // STEP 4: Engineer option values for flexible questions
  // ══════════════════════════════════════════════
  For each FLEXIBLE question i with target answer S[i]:
    Compute the correct value V that the evaluator produces given S:
      - count_answer('C'):       V = count of 'C' in S
      - closest_after(3, 'A'):   V = first index >3 where S[j]='A', or "None"
      - letter_distance(5):      V = |letterIndex(S[i]) - letterIndex(S[5])|
      - etc.

    Build 5 option values such that option S[i] maps to V:
      - k = letterIndex(S[i])        // 0 for A, 1 for B, ... 4 for E
      - options[k] = V               // the target answer maps to the correct value
      - Fill other 4 slots with plausible but DIFFERENT values
        (see "Option Value Strategy" below)

  // ══════════════════════════════════════════════
  // STEP 5: Construct only_true_statement (if present)
  // ══════════════════════════════════════════════
  For each COMPOUND question i with target answer S[i]:
    Generate claim for option S[i]:
      - Pick a claim type (count_answer_equals, count_consonant_answers_equals, etc.)
      - Compute its true value given S → this claim IS true in S
    Generate claims for the other 4 options:
      - Pick claim types and compute their true values in S
      - Use WRONG values (off by 1-2) → these claims are false in S
      - Verify each is actually false

  // ══════════════════════════════════════════════
  // STEP 6: Self-consistency check
  // ══════════════════════════════════════════════
  Run all evaluators with S. Every question must evaluate to valid.
  (This MUST pass by construction. If it doesn't → bug.)

  // ══════════════════════════════════════════════
  // STEP 7: Verify uniqueness
  // ══════════════════════════════════════════════
  Run solver(puzzle, maxSolutions=2).
  If exactly 1 solution → done! Proceed to step 8.
  If 2+ solutions → tighten (see below), then re-verify.
  If tightening fails after K attempts → discard, retry from step 1.

  // ══════════════════════════════════════════════
  // STEP 8: Verify human-solvability
  // ══════════════════════════════════════════════
  Run deduction checker (see below).
  If puzzle is solvable by logic → emit puzzle.
  If puzzle requires too much guessing for its level → retry from step 1.
```

### Why Solution-First Works

By choosing S first and engineering options to fit, we **guarantee at least 1 solution** (step 6). The only remaining question is whether extra solutions snuck in. This is dramatically more efficient than random assembly, where most candidates have 0 solutions.

### Tightening: Eliminating Spurious Solutions

When the solver finds a spurious solution S' alongside our target S:

```
tighten(puzzle, target S, spurious S'):
  diffs = positions where S[i] ≠ S'[i]

  for i in diffs:
    if question[i] is FLEXIBLE:
      // S'[i] maps to some value V' in the current options.
      // The evaluator produces a different value V'' for S'.
      // If V' ≠ V'', S' is already invalid at this question — skip.
      // If V' == V'' (spurious solution satisfies this question):
      //   Adjust option values to break it.
      //   E.g., shift the option range so S'[i] no longer maps to V''.
      //   Constraint: must keep S[i] → V mapping intact.

  If option-tightening didn't eliminate S':
    Replace a FLEXIBLE question at a diff position with a
    more constraining type, re-engineer its options, re-verify.
```

### Option Value Strategy

Option values are the main lever for controlling difficulty and uniqueness. The target answer's option must hold the correct value; the other 4 must hold plausible distractors.

For **flexible** questions, we control which letter is correct by positioning the correct value:

```
Example: count_answer('C'), target answer = D, correct count = 2
  k = letterIndex('D') = 3
  options[3] = 2                  // D maps to the correct count
  options[0,1,2,4] = other values // e.g. [0, 1, 3, 4] or [1, 3, 0, 4]
  Result: A=0, B=1, C=3, D=2, E=4
```

| Rule type | Options[target] | Distractor strategy |
|-----------|----------------|-------------------|
| count_answer | count(X) in S | 4 other values from [0..N], spread around correct value. Prefer including 0 and nearby integers. |
| closest_after/before | question # or "None" | 4 other question numbers near the correct one + "None" if not already correct. |
| first/last_with_answer | question # or "None" | Bias low for first, high for last. Include "None". |
| letter_distance | \|S[i] - S[j]\| | Always [0,1,2,3,4] — fixed range, answer determined by target. |
| most_common_count | max frequency in S | 4 values from [1..N] centered around correct. |
| count_answer_before/after | count in range | Values from [0..range_size]. |
| previous_same_answer | question # or "None" | Previous 4 question numbers + "None". |

For **constrained** questions (options always `[A,B,C,D,E]`): no engineering possible. The answer is determined by S, and S was reconciled in step 3.

For **compound** questions: see step 5 above.

### Difficulty Profiles

| Level | Name | Questions | Allowed Question Types | Design Intent |
|-------|------|-----------|----------------------|---------------|
| 1 | Beginner | 4 | count_answer, answer_of_question | At least 1 easy entry point. Simple counting loops. |
| 2 | Easy | 5 | Level 1 + closest_after, first/last_with_answer | Positional questions add a second reasoning axis. |
| 3 | Medium | 8 | Level 2 + closest_before, least/most_common, count_answer_before/after | Frequency + range queries. Multiple interleaved constraint chains. |
| 4 | Hard | 10 | Level 3 + letter_distance, most_common_count | Indirect references. Fewer easy entry points. |
| 5 | Expert | 12 | All types including only_true_statement, previous_same_answer | Meta-questions. Dense cross-references. Requires full-puzzle reasoning. |

### Entry Point Questions

An "entry point" is a question solvable early with partial information. The generator ensures each puzzle has at least one (more for easier levels).

- **`answer_of_question` referencing a `count_answer`** — reason about Q1's range to narrow Q2.
- **`count_answer` where the answer is extreme** — "How many have answer E?" is often 0 or 1 in small puzzles.
- **`first_with_answer` / `last_with_answer`** — scanning from edges gives bounded search.

### Human Solvability Check

After generating a puzzle with a unique solution, verify it's solvable by logic:

```
checkSolvability(puzzle, solution):
  state = [null, null, ..., null]  // no answers assigned yet

  while state has nulls:
    progress = false

    for each unanswered question i:
      possible = []
      for letter in [A,B,C,D,E]:
        state[i] = letter
        if solver(puzzle, fixedAnswers=state, maxSolutions=1) has ≥ 1 solution:
          possible.push(letter)
        state[i] = null

      if possible.length == 1:
        // FORCED — only one option is consistent with remaining puzzle
        state[i] = possible[0]
        progress = true

    if not progress:
      // Stuck. Puzzle requires branching (guessing + checking for contradiction).
      return { deducible: false, remaining: count(nulls) }

  return { deducible: true }
```

Solvability requirements by level:
- **Levels 1-2**: must be fully deducible (no guessing needed)
- **Level 3**: must be fully deducible
- **Level 4**: allow up to 1 branch point (guess, then everything else follows)
- **Level 5**: allow up to 2 branch points

If the solvability check fails, discard and regenerate.

### Solver

DFS with pruning. Used during generation (verify uniqueness), during tightening, during solvability checks, and at runtime for the hint deduction engine.

```
solve(puzzle, fixedAnswers?, maxSolutions=2):
  solutions = []

  function search(depth, partialAnswers):
    if depth == N:
      if allValid(partialAnswers): solutions.push(copy(partialAnswers))
      return

    if fixedAnswers[depth] != null:
      partialAnswers[depth] = fixedAnswers[depth]
      if not anyContradiction(partialAnswers, depth):
        search(depth + 1, partialAnswers)
      return

    for letter in [A, B, C, D, E]:
      partialAnswers[depth] = letter
      if anyContradiction(partialAnswers, depth): continue
      search(depth + 1, partialAnswers)
      if solutions.length >= maxSolutions: return  // early exit

    partialAnswers[depth] = null  // backtrack
```

**`anyContradiction` — the pruning engine:**

```
anyContradiction(answers, upToDepth):
  for each question i where i ≤ upToDepth AND answers[i] != null:
    rule = puzzle.questions[i].rule

    // PRUNE 1: Eager full evaluation
    // If all questions referenced by this rule are assigned, evaluate it.
    if canFullyEvaluate(rule, answers):
      if not evaluate(rule, i, answers): return true

    // PRUNE 2: Forward checking for counting rules
    if rule.type == 'count_answer':
      selectedValue = optionValueOf(answers[i], question[i])  // e.g. "2" → 2
      currentCount = count of rule.answer in answers[0..upToDepth]
      remainingSlots = N - upToDepth - 1
      if currentCount > selectedValue: return true             // already too many
      if currentCount + remainingSlots < selectedValue: return true  // can never reach

    // PRUNE 3: Forward checking for positional rules
    if rule.type in ['closest_after', 'closest_before']:
      // If the search region is fully assigned and no match found,
      // but "None" isn't the selected answer → contradiction
      ...

  return false
```

**Performance estimates:**

| Level | Questions | Raw space | Typical nodes (pruned) | Time |
|-------|-----------|-----------|----------------------|------|
| 1 | 4 | 625 | ~50-100 | <1ms |
| 2 | 5 | 3,125 | ~100-300 | <1ms |
| 3 | 8 | 390,625 | ~1,000-5,000 | ~1ms |
| 4 | 10 | ~10M | ~10,000-50,000 | ~10ms |
| 5 | 12 | ~244M | ~50,000-500,000 | ~100ms |

The solver is called many times per generation run (~50-200 candidates per puzzle). At ~100ms for level 5, generation takes ~5-20 seconds — acceptable for a CLI tool.

### Generation Pipeline Summary

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Step 1-2   │     │   Step 3    │     │   Step 4-5  │
│ Pick types  │────▶│ Fixed-point │────▶│  Engineer   │
│ + random S  │     │ reconcile S │     │   options   │
└─────────────┘     └─────────────┘     └─────────────┘
                                              │
                    ┌─────────────┐     ┌─────┴───────┐
                    │   Step 8    │     │  Step 6-7   │
                    │  Solvability│◀────│  Verify &   │
                    │   check     │     │  uniqueness │
                    └──────┬──────┘     └─────────────┘
                           │
                    ┌──────┴──────┐
                    │   Emit or   │
                    │   retry     │
                    └─────────────┘
```

### Regeneration

```bash
pnpm generate              # regenerate all 5 puzzles
pnpm generate --level 3    # regenerate only level 3
pnpm generate --seed 42    # deterministic generation for reproducibility
```

The script writes TypeScript files directly. Each generated file includes:
- The puzzle definition (questions, options, rules)
- The solution as an exported constant (for testing; tree-shaken in prod)
- The seed used to generate it (for reproducibility)

## Hints System

The hint system has multiple levels, progressively revealing more information. Each level is player-initiated (button press). Hints used are tracked in PuzzleState so we know if the solve was unassisted.

### Level 0: Validity Bars (always on)

The colored bars already tell you which answers are wrong. This is the baseline — not counted as a "hint".

### Level 1: Contradiction Highlighting

Toggle button in the header. When active, the system highlights **why** answers are wrong by showing which questions conflict:

- **Direct contradictions:** Q1 says "2 questions have answer C" but only 1 question currently has answer C → Q1 gets a highlight, and each question whose answer contributes to the mismatch gets a subtle connector/indicator.
- **Mutual contradictions:** Q3 says "the answer to Q5 is B" and Q5 says "the answer to Q3 is A", but Q3=A and Q5=B — both are individually valid but mutually inconsistent. Highlight the pair.
- **Displayed as:** a tooltip or inline annotation on the red bar explaining the conflict in natural language: _"You marked 2, but only 1 question currently has answer C"_

### Level 2: Logical Deductions

"Show me what I can deduce" button. The system runs the **solver** on the current partial state and highlights:

- **Forced values:** "Q7 must be B" — when all other options for a question lead to contradictions given current answers. Shown as a gentle glow on the forced option.
- **Eliminated options:** "Q4 cannot be A or E" — when specific options are provably wrong. Shown as auto-strikethrough on those options (dimmer than player-marked strikethroughs).
- **Entry point suggestions:** If no deductions are possible, highlight the question that has the fewest remaining valid options — suggesting where to focus next.

The deduction engine is the same solver used in generation:
```
for each unanswered question Q:
  for each option O in [A,B,C,D,E]:
    tentatively set Q = O
    run solver(puzzle, fixedAnswers=currentState+{Q=O}, maxSolutions=1)
    if 0 solutions → O is eliminated for Q
  if only 1 option remains → Q is forced
```

For small puzzles this is instant; for level 5 it might take a few hundred ms (run in a web worker to avoid blocking UI).

### Level 3: Reveal One Answer

"Reveal a question" button. The system picks the most strategically useful question to reveal:

1. Prefer questions that **unlock the most deductions** — i.e., after revealing this answer, the most other questions become forced or have options eliminated.
2. Avoid revealing a question the player already has a correct (green) answer for.
3. The revealed answer is marked with a special "given" indicator (distinct from player-selected).

The player can also click a specific question to reveal it instead of letting the system choose.

### Hint UI

```
[puzzle header]
  [Contradictions: ON/OFF]  [Show Deductions]  [Reveal Answer]
  hint count: 2 used

[question rows with annotations when hints are active]
```

Hints are non-destructive — toggling them off hides annotations but doesn't erase knowledge. The puzzle tracks:
- `contradictionHintsUsed: boolean`
- `deductionHintsUsed: number` (how many times pressed)
- `answersRevealed: number[]` (indices of revealed questions)

Completion message reflects hint usage: "Solved!" vs "Solved with hints!"

## Sharing

Puzzle state is encoded in the URL fragment for sharing:

```
https://logiquiz.example.com/puzzle/level-3#state=ABCD_EAB_
```

Encoding scheme:
- One character per question: A-E for marked correct, underscore for no answer
- Appended as URL hash fragment (no server round-trip)
- Clicking a shared link loads the puzzle with that state pre-filled
- "Share progress" button copies the URL to clipboard
- "Share result" (after completion) shares a spoiler-free summary: "Solved Level 3 in 42 moves!" with a link

## Internationalization (i18n)

All user-facing strings go through a `t()` function. Strings are organized by namespace in plain TypeScript files (no JSON, no runtime loading).

```typescript
// src/i18n/en.ts
export default {
  puzzleList: {
    title: 'Logiquiz',
    beginner: 'Beginner',
    // ...
  },
  puzzle: {
    hint: 'Show contradictions',
    reset: 'Reset puzzle',
    solved: 'Puzzle solved!',
    // ...
  },
  about: {
    howToPlay: 'How to Play',
    // ...
  },
} as const
```

Adding a new language = adding a new file (e.g. `sv.ts`) with the same structure. Locale detected from browser, overridable in UI.

Question text in generated puzzles is also produced via i18n-aware templates (the generator outputs localization keys, not raw English strings).

## Styling & Theming

### Themes

- **Light and dark** themes, respecting `prefers-color-scheme` by default
- Manual toggle in header
- CSS custom properties for all colors, switching via `[data-theme="dark"]` on `<html>`

### Visual Design

- **Fun but compact** - the #1 priority is that the entire puzzle fits on screen at once on desktop
- Small, dense text. Tight row spacing. No wasted vertical space
- Questions in a compact table/grid layout
- Option buttons are small inline pills: `[A] [B] [C] [D] [E]`
- Validity bar: 4px colored stripe on the left edge of each row
- Header: puzzle title, difficulty stars, hint toggle, share button
- Completion: subtle confetti or glow animation (nothing that obscures the puzzle)

### Responsive

- Desktop: everything visible at once (the key constraint)
- Mobile: scrollable, but still compact. Options may stack or wrap if needed
- Breakpoint: ~768px

## Persistence (localStorage)

- Save progress per puzzle: `logiquiz:puzzle:{id}` → serialized PuzzleState
- Save on every answer change (debounced)
- Restore on page load
- Track: answers, hints used, completion status
- "Reset puzzle" button clears saved state

## Implementation Sequence

### Phase 1: Project Scaffold
1. Copy config files from rookie2026 (package.json, vite, tsconfig, oxlint)
2. Adapt: remove PHP/backend scripts, simplify vite config to single entry
3. Remove lean-qr dependency, keep Preact + preact-iso
4. Set up index.html, main.tsx, App.tsx with routing
5. Basic CSS with light/dark theme variables
6. Set up i18n with `en.ts` strings file
7. `pnpm install`, verify dev server starts

### Phase 2: Engine (evaluators + validation)
1. Define types in `engine/types.ts` (Puzzle, QuestionDef, OptionDef, ValidationRule, Claim, etc.)
2. Implement evaluators in `engine/evaluators.ts` — one per rule type, pure functions
3. Implement claim evaluators (for `only_true_statement` sub-statements)
4. Implement `validate.ts` — runs all evaluators, returns validity per question
5. Write test harness: manually encode the 5 example puzzles with known solutions, verify the engine accepts the correct solution and rejects wrong ones

### Phase 3: Solver
1. Implement DFS solver in `generator/solver.ts` with `fixedAnswers` support
2. Add pruning: eager evaluation, forward checking on counting/positional rules
3. Verify solver finds exactly 1 solution for each of the 5 hand-encoded example puzzles
4. Benchmark solver performance across puzzle sizes (target: <1s for 12 questions)

### Phase 4: Generator
1. Define difficulty profiles in `generator/difficulty.ts`
2. Build question templates in `generator/templates.ts` — parameterized by indices/letters
3. Build option value engineering per rule type
4. Build assembler in `generator/assemble.ts` — solution-first algorithm (steps 1-6)
5. Implement fixed-point reconciliation for constrained questions (step 3)
6. Implement tightening for uniqueness (step 7)
7. Implement human-solvability check (step 8)
8. Implement two-pass generation for `only_true_statement` (step 5)
9. Build CLI script `scripts/generate-puzzles.ts`
10. Generate all 5 difficulty levels, verify unique solvability + human solvability
11. Iterate on generation parameters until puzzles feel right (human playtest)

### Phase 5: UI Components
1. `OptionButton` — click cycling through unmarked/incorrect/correct states
2. `QuestionRow` — question text, 5 option buttons, validity bar
3. `PuzzleView` — renders all questions, manages answer state, runs validation on every change
4. `PuzzleList` — grid of puzzle cards with difficulty + completion status
5. `About` — static content page with how-to-play info

### Phase 6: Hints
1. Level 1: contradiction highlighting — annotate red bars with natural-language explanations
2. Level 2: deduction engine — run solver on partial state in a web worker
3. Level 2 UI: show forced values + eliminated options
4. Level 3: reveal answer — pick most strategically useful question to reveal
5. Track hint usage in PuzzleState

### Phase 7: State, Sharing & Polish
1. localStorage persistence in `lib/store.ts`
2. URL sharing in `lib/share.ts` (state in fragment)
3. Win condition detection + celebration
4. Puzzle reset
5. Theme toggle (light/dark)
6. Mobile responsiveness
7. PWA manifest

### Phase 8: Testing
1. E2E tests: load puzzle, solve it, verify completion
2. E2E tests: option cycling, validity bar colors
3. E2E tests: hint system (contradictions, deductions, reveal)
4. E2E tests: sharing URL restores state
5. E2E tests: persistence (reload mid-puzzle)
6. E2E tests: theme switching

## Example Puzzle Encoding

The 4-question starter puzzle from the brief:

```typescript
const puzzle01: Puzzle = {
  id: 'starter-4',
  title: 'Starter',
  difficulty: 1,
  questions: [
    {
      text: 'How many questions have answer C?',
      options: [{ label: '0' }, { label: '1' }, { label: '2' }, { label: '3' }, { label: '4' }],
      rule: { type: 'count_answer', answer: 'C' },
    },
    {
      text: 'What is the answer to question #1?',
      options: [{ label: 'A' }, { label: 'B' }, { label: 'C' }, { label: 'D' }, { label: 'E' }],
      rule: { type: 'answer_of_question', questionIndex: 0 },
    },
    {
      text: 'How many questions have answer B?',
      options: [{ label: '0' }, { label: '1' }, { label: '2' }, { label: '3' }, { label: '4' }],
      rule: { type: 'count_answer', answer: 'B' },
    },
    {
      text: 'How many questions have answer E?',
      options: [{ label: '0' }, { label: '1' }, { label: '2' }, { label: '3' }, { label: '4' }],
      rule: { type: 'count_answer', answer: 'E' },
    },
  ],
}
```

## Design Decisions

- **No timer** - the experience is about logic, not speed
- **No backend** - everything client-side, puzzles bundled at build time
- **Generator committed output** - generated puzzles are checked into the repo so builds are deterministic and fast
- **Solution-first generation** - choose the answer first, engineer options to fit, then verify uniqueness. Dramatically more efficient than random assembly.
- **Solver reused everywhere** - same DFS+pruning solver powers generation, uniqueness verification, solvability checking, and runtime hint deductions
- **i18n from day 1** - all strings in translation files, even though only English for now
- **Question text is templated** - generator uses i18n-aware templates so questions can be localized without regenerating puzzles
