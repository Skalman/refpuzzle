# What's checked where

Three modules validate a puzzle, distinguished by *what they read* — plus
`deduce`, which reads what the checks don't. Each one's module doc is the
authority; this is the side-by-side view.

`check_form.rs` — is the *shape* legal — option counts, index ranges,
statement shape?

- **Reads:** the parsed puzzle's structure only
- **Doesn't read:** the answer key, the board, any semantics ("is this claim
  true?" is `check_answer`'s)
- **During construction:** designed to only produce well-formed puzzles;
  asserted well-formed at the end

`check_well_posed.rs` — does question `qi`'s definition match the resulting
shape of the answer key? What breaks the match is each question type's own
meaning — e.g. a tie ruins MostCommon but not MostCommonCount.

- **Reads:** split by dependency — `check_well_posed_given_key` reads the
  answer key + the question's own params; `check_well_posed_given_key_and_options`
  also reads the filled option/claim values (OnlySameAmong/OnlySameAsAmong
  targets, TrueStmt claims)
- **Doesn't read:** the play-time board/marks
- **During construction:** the key half rejects candidates at parametrize,
  the options half rejects repair edits; asserted well-posed at the end

`check_answer.rs` — is an answer to question `qi` valid / pending / invalid
on the current (possibly partial) board?

- **Reads:** every cell's mark on the raw board — answers + eliminations
  (CountVowel tallies the whole board)
- **Doesn't read:** another *question's* meaning — cross-question reasoning is
  deduce's job; consequences arrive only as forced/eliminated marks
- **During construction:** `fill_options` builds with it (`check_claim_fast`
  rejects candidate claims/options against the key), acceptance asserts every
  key answer comes back Valid, and the brute engine leans on it

`deduce.rs` — not a check, but the contrast that completes the picture: what
sound *progress* can be made on the current board — eliminate cells, force
answers?

- **Reads:** the marks *and* every question's meaning — cross-question
  reasoning (propagating one question's meaning into the marks, combining
  questions) is deduce's alone
- **Doesn't read:** the answer key. And it never owns validity: deliberately
  incomplete (a finite set of cheap, explainable rules), it may surface a
  contradiction but defers "is this answer already impossible?" to
  `check_answer`
- **During construction:** the deduce+lookahead engine must actually solve a
  candidate for it to be accepted, so generation only ships puzzles this
  engine can finish

As a matrix — what each module reads (✅ / —) and the contract it operates
under:

|  | `check_form` | `check_well_posed` | `check_answer` | `deduce` |
| --- | --- | --- | --- | --- |
| Reads: own question's type + params | ✅ shape only | ✅ | ✅ | ✅ |
| Reads: answer key | — | ✅ | — | — |
| Reads: own question's option values | counts/ranges only | `_given_key_and_options` half only | ✅ | ✅ |
| Reads: board state / marks (answers + eliminations) | — | — | ✅ | ✅ |
| Reads: other questions' meaning (type) | existence of index targets only | — | — never; cross-question consequences arrive as marks, via deduce | ✅ its specialty |
| May throw unless well-formed | — (the authority) | — (runs on untrusted input, reports instead) | ✅ tagged `Fatal check_form error.` | ✅ untagged |
| Produces incorrect results unless well-formed | — | ✅ the dodged panics return a meaningless verdict | ✅ | ✅ |
| Produces incorrect results unless well-posed | — | — (the authority) | ✅ | ✅ |
| Intended to be "complete" given its scope | ✅ by definition | ✅ except what `check_form` already checks | ✅ a stated goal | ❌ deliberately incomplete |

Ordering follows from the reads:

- `check_form` comes first: both other checks assume a **well-formed**
  puzzle, and `check_answer` may panic on structural nonsense (sites tagged
  `Fatal check_form error.`).
- `check_well_posed` runs in the generator pipeline where its inputs exist —
  the key-only half at parametrize (authoritative; `fill_options`/repair never
  change it), the options half only once options exist. Uniqueness only;
  feasibility is the reserve's problem.
- `check_answer::check_claim` is the play-time authority shared by solver,
  generator, and UI. Within its one-question scope it is *complete* (a definite
  verdict is always reachable), unlike `deduce`, which prunes and propagates
  but never owns validity.

Severity lives only in `check_form`: an `Error` may never ship (served puzzles
get edited), a `Warning` is grandfathered on served puzzles but retired from
generation.

## TODO

- **Unify the fatal-panic tag convention.** `check_answer` tags malformed-input
  panics `Fatal check_form error.`. `deduce` has the same panics, untagged.
  Tag them identically, or share a macro.
- **Adopt "may throw unless well-formed" in `check_well_posed`.** Its
  panic-dodging guards report malformed input as well-posed. Callers should
  run `check_form` first instead. Replace the guards with tagged panics,
  matching `check_answer`'s contract.
