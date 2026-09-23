# State encoding

The string format for a puzzle's saved state — the player's solve, not the
puzzle. Owned by `encodeHistory` / `decodeHistory` in `src/lib/store.ts`. It
appears in the localStorage value (`<history>|<meta>`), the share-URL hash,
and the playground hash's `&h=` parameter (both `<history>` only — meta never
leaves the device).

The frontend is the only parser. Two Rust CLI diagnostics emit the
mark/eliminate subset (no unmark, `cp`, annotations, cursor):
`format_steps` / `solution_str_to_steps` / `make_url` in
`rust/src/cli/check.rs`, and `playground_link` in `rust/src/cli/link.rs` —
which also builds the playground hash's `p=` parameter, a separate format:
the Rust-owned compact puzzle blob (`rust/src/serialize.rs`), deflated and
base64url-encoded (frontend-side in `src/lib/playground.ts`).

A string whose first `.`-token matches `v<digits>` is versioned; anything
else is **v0**, the original unversioned format. The encoder always emits
the newest version; v0 strings still arrive from old localStorage, backup
files, and shared links.

## v1

Alphabet: URL-unreserved characters only (`A-Z a-z 0-9 - . _`), so the string
survives linkifiers unescaped. Tokens are joined with `.`; the meta segment
follows after `|` (localStorage only, so `|` being URL-hostile is fine).

Token dispatch is first-character-driven:

| shape | class | meaning |
|---|---|---|
| starts with a digit | action | a cell move; pushes one history step |
| starts with a letter, then digits | annotation | attaches to the most recent step |
| named token | special | `v<n>`, `cp`, `_` |

### History tokens

| token | class | meaning |
|---|---|---|
| `v1` | header | format version; always the first token |
| `5D` | action | mark option D of #5 correct (uppercase letter) |
| `5d` | action | eliminate option D of #5 (lowercase letter) |
| `5d-` | action | clear the mark on #5 D (suffix `-`) |
| `cp` | action | checkpoint: pushes a step with no board change |
| `h1` | annotation | hint marker; value is the deepest escalation level reached at that step (`h1`…`h4` in practice) |
| `h2q5` | annotation | the same, naming the question the hint pointed at (#5) |
| `cpx2` | annotation | count of refused checkpoint presses at that step |
| `cpx2q1q4` | annotation | the same, naming the questions refused (#1 and #4) |
| `_` | cursor | the current step is the one preceding this token. Leading `_` = cursor on the blank board; absent = cursor at the last step |

Canonical emission order: header, then per step its action, its annotations
(`h` before `cpx`), then `_` if the cursor rests there. Annotations before
any action attach to step 0 (the blank board) — this is how refusals and
hints on `Start` are recorded.

There are no flags in the history segment. Completion is *derivable* — every
client recomputes "all answers valid" from the board — so it isn't shared
state; it lives in the ledger (`s`) for the pre-wasm synchronous init. A client
decoding a shared URL derives completion once wasm loads, and must treat
completion that predates the player's first local change as not theirs
(no celebration, no completion analytics).

Semantic notes:

- `-` appears only as the action suffix (retracts state); `x` appears only in
  `cpx` (a denied press). They are deliberately distinct: one is a step, the
  other a tally on a step.
- A hint marker's level is the index of the explain step shown, 1-based;
  fold-on-rewrite keeps the maximum, never a sum. The longest explanation the
  engine builds is four steps, so `h4` is the highest level in practice — the
  codec reads any `h<digits>`, and explanations are meant to stay within four.
- `cpx` counts are unbounded (`cpx13` is valid).
- Both markers take an optional question suffix, 1-based like the actions, `q`
  marking and separating each (`h2q5`, `cpx2q1q4`). A hint names at most one —
  the question its steps pointed at first, fixed once set; refusals name a set,
  and the count stands on its own, so `cpx2q4` is two presses refused over the
  same question. No suffix means no step named a question (a vague opening
  line, or no obvious next step), so no question reads as hinted in the shared
  result. Fold-on-rewrite unions the refusals' questions and keeps the earlier
  hint's.

Unknown tokens are skipped — in history without pushing a step, in the
ledger without effect; decoding never fails outright. That policy is lossless
for annotations (a skipped marker costs a badge, never the board) but not for
actions (a skipped action skews every later board). Hence the versioning
contract: new annotation stems, and new suffixes on existing ones, may be added
*within* v1 — old clients degrade gracefully, dropping the marker whole — while
any new action shape or semantics must bump the header.
A v1 client reading a higher version still best-effort parses; refusing would
read as lost progress and invite an overwrite.

### Meta: the local ledger

Dot-joined, same shapes as history annotations: counters are
letters-then-digits (omitted at zero), flags are bare stems (present or
absent). Bare flags were a trap in v0 only because its meta was
substring-scanned; with discrete tokens, presence is an exact match.

The ledger is flags, then counters. The flags say where the puzzle stands;
the counters accumulate while solving and stay once it is solved, so the
solve summary can read them back.

Flags — what the list pages read:

| token | meaning |
|---|---|
| `s` | solved |
| `st` | stale — the answers no longer validate against the regenerated puzzle |

Counters — what `puzzle_completed` receives, and the summary shows:

| token | meaning |
|---|---|
| `se2` | sessions: stretches of the puzzle being on screen |
| `e743` | elapsed seconds, summed over those sessions |
| `n1` | history-navigation bursts |
| `h2` | total hint presses |
| `cp1` | checkpoints planted |
| `cpx2` | checkpoint presses refused |
| `f` | state originated from a shared link |

`cp`/`cpx` share stems with the history tokens deliberately — same concept,
tally vs placement. The flags live here rather than in history because
neither is a fact about the solve's board: solved is derivable from the
board, and stale is about this device's cache (v0 already stripped it before
sharing). Invariants: `st` only ever appears beside `s`, and a solved ledger
opens with `s` — so the list pages' solved/stale checks are a prefix read, no
decode. Both the boot re-check and a solve in play can set or clear `st`;
either way the writer has just checked the board against the current puzzle.
A ledger with the solved flag and no counters is a solve that didn't happen
on this device: a shared board, or a backup without counters.

### Examples

```
v1.3b.3c.7D.cp.2a.5C.h1.1A|se2.e743.n1.h2.cp1    mid-solve, cursor at end
v1.3b.3c.7D.cp.2a._.5C.h1.1A                     cursor parked after 2a (redo available)
v1._.3b.4E                                       cursor on the blank board
v1.cpx1.3b.4E                                    a refusal before the first mark (on Start)
v1.3b.h2q4.1c.4C.cp.2a.cpx1q2.2A                 a hint pointed at #4, a refusal over #2
v1.3b.3c.7D.cp.2a.5C.1A|s.se2.e743.n1.h2.cp1     solved; the counters stay
v1.3b.3c.7D.cp.2a.5C.1A|s.st.se2.e743            solved, later went stale
v1.3b.3c.7D.cp.2a.5C.1A|s                        arrived solved (shared link, old backup)
```

## v0 (legacy, decode-only)

The original format. Never emitted anymore; still decoded from old
localStorage, backups, and shared links.

### History tokens

| token | meaning |
|---|---|
| `5D` / `5d` | mark correct / eliminate (as v1) |
| `-5d` | clear (prefix `-`, not suffix) |
| `cp` | checkpoint |
| `h1`…`h4` | hint marker |
| `_5d` | prefix form: this action is the current step |
| `_` | bare: cursor at the preceding position; emitted only spliced to the front, for cursor-at-blank |
| `x` | completed flag — positional, second-to-last or last token |
| `!` | stale flag — positional, must be the last token |

### Meta

Not tokenized: one string after `|`, scan-parsed with independent
`/<letter>(\d+)/` regexes — `s` sessions, `e` elapsed, `n` bursts, `h` hints,
`c` checkpoints — plus `fromShared` as a bare `f` matched by substring test.
That substring test is why v0 could never gain a field whose value might
contain an `f`.

Decoding is best-effort like v1: an unrecognized token is skipped without
pushing a step. (The shipped v0 decoder pushed a phantom step there; since no
legitimate v0 string contains unknown tokens, the difference only shows on
corrupt input.)

### Quirks (bugs preserved as behavior)

- **Cursor-at-blank round-trip**: the decoder initializes its cursor to 0 and
  a leading `_` also produces 0, so the "default to end when no marker" rule
  fires and a save made at the blank board decodes with the cursor at the
  end. v1 fixes this structurally (absent `_` is the only end-marker).
- **Flag order**: `!` must follow `x`; the decoder reads both by position
  from the end of the token list.
- v0 never had a refusal token. Refused-checkpoint data exists only in v1.

## Migration

- localStorage converts in a one-shot textual sweep at boot: a token-level
  v0→v1 rewrite that needs no decode and no question count. Lazy conversion
  is not an option: the cheap helpers (`saveMeta`, `markStale`, `hasState`,
  …) edit one segment while preserving the other verbatim, and the meta
  segment is not self-versioning — mixed writes would corrupt. After the
  sweep, those helpers are v1-only.
- The sweep is gated on a stored format version, kept beside the puzzle
  revalidation version in one querystring-shaped key:
  `refpuzzle:version = "puzzle=3&format=1"` (a legacy bare number means
  `format=0`). The fields have different writers in different phases, so
  all access goes through one read-merge-write accessor. The conversion
  stays idempotent: if `loadState` ever sniffs a v0 string in localStorage
  after migration (a pre-deploy tab wrote late), it converts that value in
  place.
- `decodeHistory` still sniffs the first token and dispatches to the v1 or
  v0 parser — for external inputs: share links, backup/sync ingestion
  (which migrates each incoming value the same way), and CLI links.
- Backup files and share links deliver v0 strings long after localStorage
  has converted — dropping the v0 decoder orphans them.
- The Rust CLI emitters keep producing headerless mark/eliminate strings.
  That subset is byte-identical in v0 and v1, so their links sniff as v0 and
  decode correctly without any Rust-side change.
- v0 clients reading v1 data (stale service worker) mis-decode it as phantom
  steps — the version header cannot protect decoders that predate it. Known,
  accepted, and the last time this class of hazard exists.
