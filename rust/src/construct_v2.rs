//! v2 generation pipeline — parallel track. See docs/generation-redesign.md.
//!
//! `compose()` runs SELECT → ASSIGN → PARAMETRIZE: pick the question kinds,
//! decide the answer key plus a kind per slot (shape-types seed the structure
//! they need), then turn each kind into a full `QuestionType` against that key.
//! `generate()` wraps that with `fill_options` + `validate_and_repair`.
//! Runs alongside the live generator (`construct.rs`); reachable via
//! `type-stats --v2` for comparison, not yet the production path.
#![allow(dead_code)] // not yet the production generator; reachable via `type-stats --v2`

use arrayvec::ArrayVec;

use crate::build::{
    ComposeStats, FallbackCounts, GenerateResult, Stats, fill_options, solution_satisfies_type,
    validate_and_repair,
};
use crate::check_form::check_form;
use crate::construct::{random_type_params, solution_fits_type};
use crate::rng::Rng;
use crate::types::QuestionTypeKind::*;
use crate::types::*;

/// v2 per-level recipe — selection-shaped, deliberately separate from
/// `DifficultyProfile`. `required` + `allowed` + `caps` fully describe selection;
/// weighted overrides land here when we tune.
pub struct LevelRecipe {
    /// Types that must appear, with how many of each.
    pub required: &'static [(QuestionTypeKind, usize)],
    /// The pool the remaining slots are filled from.
    pub allowed: &'static [QuestionTypeKind],
    /// Per-type max occurrences (default 3; unit variants 1 — see `DEFAULT_CAPS`).
    pub caps: [u8; 32],
}

const fn caps_with(overrides: &[(QuestionTypeKind, u8)]) -> [u8; 32] {
    let mut c = [3u8; 32];
    let mut i = 0;
    while i < overrides.len() {
        c[overrides[i].0 as usize] = overrides[i].1;
        i += 1;
    }
    c
}

const DEFAULT_CAPS: [u8; 32] = caps_with(&[
    (QuestionTypeKind::LetterDist, 1),
    (QuestionTypeKind::AnswerOf, 3),
    // Parameter-free variants: a second of the same kind is the *identical*
    // question, so cap them at 1 (the live generator gets this reactively via
    // `types_contain`; we do it up front).
    (QuestionTypeKind::CountVowel, 1),
    (QuestionTypeKind::CountConsonant, 1),
    (QuestionTypeKind::MostCommonCount, 1),
    (QuestionTypeKind::PrevSame, 1),
    (QuestionTypeKind::NextSame, 1),
    (QuestionTypeKind::OnlySame, 1),
    (QuestionTypeKind::SameAs, 1),
    (QuestionTypeKind::ConsecIdent, 1),
    (QuestionTypeKind::LeastCommon, 1),
    (QuestionTypeKind::MostCommon, 1),
    (QuestionTypeKind::NoOtherHasAnswer, 1),
    (QuestionTypeKind::AnswerIsSelf, 1),
    (QuestionTypeKind::TrueStmt, 1),
]);

/// Initial recipes — rough; tuned via type-stats. Indexed by level-1.
pub static RECIPES: [LevelRecipe; 6] = [
    // L1
    LevelRecipe {
        required: &[],
        allowed: &[
            CountAnswer,
            CountAnswerBefore,
            CountAnswerAfter,
            AnswerOf,
            ClosestAfter,
            ClosestBefore,
            FirstWith,
            LastWith,
            SameAs,
            PrevSame,
            NextSame,
            MostCommon,
            LeastCommon,
            NoOtherHasAnswer,
        ],
        caps: DEFAULT_CAPS,
    },
    // L2
    LevelRecipe {
        required: &[],
        allowed: &[CountAnswer, AnswerOf, AnswerIsSelf, FirstWith, LastWith],
        caps: DEFAULT_CAPS,
    },
    // L3
    LevelRecipe {
        required: &[],
        allowed: &[
            CountAnswer,
            CountAnswerBefore,
            CountAnswerAfter,
            AnswerOf,
            AnswerIsSelf,
            ClosestAfter,
            ClosestBefore,
            FirstWith,
            LastWith,
            NextSame,
            PrevSame,
            SameAs,
        ],
        caps: DEFAULT_CAPS,
    },
    // L4
    LevelRecipe {
        required: &[],
        allowed: &[
            CountAnswer,
            AnswerOf,
            AnswerIsSelf,
            ClosestAfter,
            ClosestBefore,
            FirstWith,
            LastWith,
            NextSame,
            PrevSame,
            LeastCommon,
            MostCommon,
            CountAnswerBefore,
            CountAnswerAfter,
            CountVowel,
            CountConsonant,
            NoOtherHasAnswer,
            OnlySame,
            SameAs,
        ],
        caps: DEFAULT_CAPS,
    },
    // L5
    LevelRecipe {
        required: &[],
        allowed: &[
            CountAnswer,
            AnswerOf,
            AnswerIsSelf,
            ClosestAfter,
            ClosestBefore,
            FirstWith,
            LastWith,
            NextSame,
            PrevSame,
            LeastCommon,
            MostCommon,
            MostCommonCount,
            CountAnswerBefore,
            CountAnswerAfter,
            CountVowel,
            CountConsonant,
            NoOtherHasAnswer,
            OnlySame,
            SameAs,
            LetterDist,
            EqualCount,
            ConsecIdent,
            OnlyOdd,
            OnlyEven,
            SameAsWhich,
        ],
        caps: DEFAULT_CAPS,
    },
    // L6
    LevelRecipe {
        required: &[(TrueStmt, 1)],
        allowed: &[
            CountAnswer,
            AnswerOf,
            AnswerIsSelf,
            ClosestAfter,
            ClosestBefore,
            FirstWith,
            LastWith,
            NextSame,
            PrevSame,
            LeastCommon,
            MostCommon,
            MostCommonCount,
            CountAnswerBefore,
            CountAnswerAfter,
            CountVowel,
            CountConsonant,
            NoOtherHasAnswer,
            OnlySame,
            SameAs,
            LetterDist,
            EqualCount,
            ConsecIdent,
            OnlyOdd,
            OnlyEven,
            TrueStmt,
            SameAsWhich,
        ],
        caps: DEFAULT_CAPS,
    },
];

pub struct Composed {
    pub types: [QuestionType; MAX_N],
    pub solution: [Answer; MAX_N],
    pub n: usize,
}

/// SELECT the kinds, ASSIGN each a slot + answer, then PARAMETRIZE the kinds
/// against the finished answer key. Always succeeds
/// (fallbacks guarantee a result); uniqueness is checked later by the caller.
/// Telemetry (compose count + per-phase AnswerOf fallbacks) is tallied into `cs`.
pub fn compose(
    recipe: &LevelRecipe,
    n: usize,
    oc: usize,
    rng: &mut Rng,
    cs: &mut ComposeStats,
) -> Composed {
    cs.compose_count += 1;
    let fb = &mut cs.fallbacks;
    let kinds = select_kinds(recipe, n, rng);
    let SolutionAndKinds { solution, kind_of } =
        SolutionAndKindsBuilder::new(n, oc).build(&kinds, rng, fb);
    let types = parametrize(n, oc, &solution, &kind_of, rng, fb);

    Composed { types, solution, n }
}

/// Full v2 generation: `compose` a fresh skeleton, encode its option values
/// (`fill_options`), and validate (`validate_and_repair` — solvable + unique).
/// Retries with a fresh skeleton each attempt until one validates or the budget
/// runs out. Mirrors `construct::generate`'s tail, but re-composes per attempt
/// because `compose` authors the solution itself (not just the rule placement).
pub fn generate(
    recipe: &LevelRecipe,
    n: usize,
    oc: usize,
    rng: &mut Rng,
    max_attempts: usize,
    stats: &mut Stats,
    label: &str,
) -> Option<GenerateResult> {
    for _ in 0..max_attempts {
        let c = compose(recipe, n, oc, rng, &mut stats.v2_compose);
        let mut fp = fill_options(&c.types, &c.solution, c.n, oc, rng, false);
        // compose should never produce a malformed answer key: references are
        // in range by construction, fill_options encodes values in range, and
        // NoOtherHasAnswer's `cover_all` keeps every letter present (the one
        // construct-side hazard). Assert it — a malformed key panics deep in
        // validate_and_repair otherwise, and a silent reject would mask the bug.
        let form_errors = check_form(&fp, Some(&c.solution[..c.n]));
        assert!(
            form_errors.is_empty(),
            "compose produced a malformed answer key (n={}, oc={oc}): {form_errors:?}\n  types={:?}\n  sol={:?}",
            c.n,
            &c.types[..c.n],
            &c.solution[..c.n]
        );
        if validate_and_repair(
            &c.types,
            &c.solution,
            &mut fp,
            c.n,
            rng,
            stats,
            false,
            label,
        ) {
            return Some(GenerateResult {
                question_types: c.types,
                fp,
                n: c.n,
            });
        }
    }
    None
}

/// Required types first, then fill remaining slots from `allowed` (uniform for
/// now), respecting caps. Panics if the pool can't fill `n` slots — that's a
/// recipe misconfiguration, not a runtime condition.
fn select_kinds(
    recipe: &LevelRecipe,
    n: usize,
    rng: &mut Rng,
) -> ArrayVec<QuestionTypeKind, MAX_N> {
    let mut sel = KindSelection::new(recipe.caps);

    for &(kind, count) in recipe.required {
        for _ in 0..count {
            assert!(
                sel.room_for(kind),
                "level recipe requires {kind:?} but has a too strict cap"
            );
            sel.take(kind);
        }
    }
    assert!(
        sel.picked_kinds.len() <= n,
        "level recipe requires {} types but the level has only {n} slots",
        sel.picked_kinds.len()
    );

    // Only kinds with room left; a kind is evicted the moment it fills, so
    // every draw below places exactly one kind (no wasted picks).
    let mut eligible: ArrayVec<QuestionTypeKind, 32> = recipe
        .allowed
        .iter()
        .copied()
        .filter(|&k| sel.room_for(k))
        .collect();
    while sel.picked_kinds.len() < n {
        assert!(
            !eligible.is_empty(),
            "level recipe can't fill {n} slots: pool capped out"
        );
        let (index, kind) = rng.pick_kv(&eligible);
        sel.take(kind);
        if !sel.room_for(kind) {
            eligible.swap_remove(index);
        }
    }

    sel.picked_kinds
}

/// Running tally during selection: the chosen kinds plus per-kind counts used
/// for cap checks.
#[derive(Default)]
struct KindSelection {
    picked_kinds: ArrayVec<QuestionTypeKind, MAX_N>,
    count_per_kind: [u8; 32],
    cap_per_kind: [u8; 32],
}

impl KindSelection {
    fn new(cap_per_kind: [u8; 32]) -> KindSelection {
        KindSelection {
            picked_kinds: ArrayVec::default(),
            count_per_kind: [0; 32],
            cap_per_kind,
        }
    }

    fn room_for(&self, k: QuestionTypeKind) -> bool {
        self.count_per_kind[k as usize] < self.cap_per_kind[k as usize]
    }

    fn take(&mut self, k: QuestionTypeKind) {
        self.picked_kinds.push(k);
        self.count_per_kind[k as usize] += 1;
    }
}

/// Scratch for the first compose phase: decides the answer key and the kind per
/// slot. `build` consumes it and returns a `SolutionAndKinds`; `parametrize`
/// turns those kinds into full `QuestionType`s next. The fields below `decided`
/// are the in-progress constraints the shapes impose on the fill.
struct SolutionAndKindsBuilder {
    n: usize,
    oc: usize,
    solution: [Answer; MAX_N],
    decided: u16,                 // bit qi: solution[qi] is decided
    open: ArrayVec<usize, MAX_N>, // qi slots not yet claimed by a kind, shuffled
    banned: [bool; 5],            // letters the fill must avoid (reserved at an exact count)
    no_pairs: bool,               // ConsecIdent pinned → no new adjacent equal pairs
    cover_all: bool,              // NoOther pinned → every letter must appear
}

struct SolutionAndKinds {
    solution: [Answer; MAX_N],
    kind_of: [QuestionTypeKind; MAX_N],
}

impl SolutionAndKindsBuilder {
    fn new(n: usize, oc: usize) -> Self {
        SolutionAndKindsBuilder {
            n,
            oc,
            solution: [Answer::A; MAX_N],
            decided: 0,
            open: ArrayVec::new(),
            banned: [false; 5],
            no_pairs: false,
            cover_all: false,
        }
    }

    fn is_decided(&self, qi: usize) -> bool {
        self.decided & (1 << qi) != 0
    }
    fn put(&mut self, qi: usize, l: Answer) {
        self.solution[qi] = l;
        self.decided |= 1 << qi;
    }

    /// Whether `l` appears in any decided slot of the answer key so far.
    fn present(&self, l: Answer) -> bool {
        (0..self.n).any(|qi| self.is_decided(qi) && self.solution[qi] == l)
    }

    /// A letter not yet present and not banned, within the option range.
    fn fresh_letter(&self) -> Option<Answer> {
        (0..self.oc as u8)
            .map(Answer::from)
            .find(|&l| !self.present(l) && !self.banned[l.idx()])
    }

    /// Decide the answer key and the kind per slot (returned as `SolutionAndKinds`).
    /// One pass over the selected kinds trickiest-first: a shape-type seeds its
    /// structure and claims its own slot; any other kind (or a shape that can't
    /// seed) takes a leftover slot and gets a randomized answer. The constraints
    /// the shapes impose — reserved letters (`banned`), no new adjacent pairs
    /// (`no_pairs`, ConsecIdent), every letter present (`cover_all`, NoOther) —
    /// are locals, threaded into the fill.
    fn build(
        mut self,
        kinds: &[QuestionTypeKind],
        rng: &mut Rng,
        fb: &mut FallbackCounts,
    ) -> SolutionAndKinds {
        let mut kind_of = [QuestionTypeKind::AnswerIsSelf; MAX_N];
        self.open = (0..self.n).collect();
        rng.shuffle(&mut self.open);

        // Seed shapes trickiest-first; ConsecIdent before the pair-sharers
        // (Next/Prev) so it owns the single pair they reuse. Next/Prev share a
        // rank — they reuse the same pair, so their order doesn't matter.
        let mut ordered: ArrayVec<QuestionTypeKind, MAX_N> = kinds.iter().copied().collect();
        ordered.sort_by_key(|&k| match k {
            OnlySame | NoOtherHasAnswer => 0,
            ConsecIdent => 1,
            NextSame | PrevSame => 2,
            _ => 3,
        });
        for &k in &ordered {
            let qi = match k {
                OnlySame => self.seed_only_same(),
                NoOtherHasAnswer => self.seed_no_other(),
                ConsecIdent => self.seed_consec(rng),
                NextSame => self.seed_next_same(rng),
                PrevSame => self.seed_prev_same(rng),
                _ => Some(self.fill_one(rng)),
            };

            if let Some(qi) = qi {
                kind_of[qi] = k;
            } else {
                let qi = self.fill_one(rng);
                kind_of[qi] = AnswerOf;
                fb.assign_kinds += 1;
            }
        }
        SolutionAndKinds {
            solution: self.solution,
            kind_of,
        }
    }

    /// Claim a leftover open slot for a non-shape kind and decide its answer —
    /// unless a shape already pre-set it (OnlySame's partner, a pair slot). The
    /// answer avoids banned letters and (under `no_pairs`) new adjacent pairs;
    /// when coverage still owes letters and the free slots are running out, it
    /// forces a still-absent one. Returns the slot.
    fn fill_one(&mut self, rng: &mut Rng) -> usize {
        let qi = self.open.pop().expect("a leftover slot per kind");
        if !self.is_decided(qi) {
            let missing: ArrayVec<Answer, 5> = if self.cover_all {
                (0..self.oc as u8)
                    .map(Answer::from)
                    .filter(|&l| !self.present(l))
                    .collect()
            } else {
                ArrayVec::new()
            };
            // `open` now holds only future fill slots, so its unset count plus
            // this slot is exactly how many free choices remain for coverage.
            let free_left = self.open.iter().filter(|&&s| !self.is_decided(s)).count() + 1;
            let must_cover = free_left <= missing.len();
            let l = self.pick_answer(qi, must_cover, &missing, rng);
            self.put(qi, l);
        }
        qi
    }

    /// Seed OnlySame: a fresh letter on two still-open slots (banned → it stays
    /// at exactly two). Returns the host to pin; the partner keeps its answer and
    /// is left open for a later kind.
    fn seed_only_same(&mut self) -> Option<usize> {
        let l = self.fresh_letter()?;
        let mut free = self.open.iter().copied().filter(|&s| !self.is_decided(s));
        let host = free.next()?;
        let partner = free.next()?;
        self.put(host, l);
        self.put(partner, l);
        self.banned[l.idx()] = true;
        self.open.retain(|s| *s != host);
        Some(host)
    }

    /// Seed NoOtherHasAnswer: a fresh unique letter (banned → stays at one), and
    /// set `cover_all` so pass 2 places every other letter (else an absent letter
    /// is also vacuously "held by no other" → ambiguous). Returns the host to pin.
    fn seed_no_other(&mut self) -> Option<usize> {
        let l = self.fresh_letter()?;
        let host = self.open.iter().copied().find(|&s| !self.is_decided(s))?;
        self.put(host, l);
        self.banned[l.idx()] = true;
        self.cover_all = true;
        self.open.retain(|s| *s != host);
        Some(host)
    }

    /// Seed ConsecIdent: ensure one adjacent equal pair (or none ~10% → answer
    /// None), pinned on a *separate* host whose own answer doesn't start a pair.
    /// `no_pairs` then stops pass 2 (and the pair-sharers) adding another, so the
    /// pair count stays ≤ 1. Returns the host to pin.
    fn seed_consec(&mut self, rng: &mut Rng) -> Option<usize> {
        let want_none = rng.next_f64() < 0.10;
        let host = if want_none {
            self.open.iter().copied().find(|&s| !self.is_decided(s))?
        } else {
            let a = self.ensure_pair(rng)?;
            self.open
                .iter()
                .copied()
                .find(|&s| s != a && s != a + 1 && !self.is_decided(s))?
        };
        let l = (0..self.oc as u8)
            .map(Answer::from)
            .find(|&l| !self.banned[l.idx()] && !self.would_pair(host, l))
            .unwrap_or(Answer::A);
        self.put(host, l);
        self.no_pairs = true;
        self.open.retain(|s| *s != host);
        Some(host)
    }

    /// Seed NextSame on an open slot with room for `oc` distinct options. Its
    /// answer is the nearest *later* same-answer question (or "none"), drawn from
    /// `{qi+1..n}` plus "none" = `n - qi` values, so the host needs `qi <= n - oc`.
    /// No referent is planted — "none" is a fine answer. None if oc > n.
    fn seed_next_same(&mut self, rng: &mut Rng) -> Option<usize> {
        assert!(
            self.oc <= self.n,
            "NextSame is impossible at oc={} > n={} — bad recipe",
            self.oc,
            self.n
        );
        self.seed_positional(0, self.n - self.oc, rng)
    }

    /// Seed PrevSame: mirror of [`Self::seed_next_same`], referent on the *left*.
    /// Options are `{0..qi}` plus "none" = `qi + 1` values, so `qi >= oc - 1`.
    fn seed_prev_same(&mut self, rng: &mut Rng) -> Option<usize> {
        assert!(
            self.oc <= self.n,
            "PrevSame is impossible at oc={} > n={} — bad recipe",
            self.oc,
            self.n
        );
        self.seed_positional(self.oc - 1, self.n - 1, rng)
    }

    /// Claim a random open, undecided slot in `lo..=hi` and give it a plain
    /// non-banned answer (the position-referent shapes only need enough referent
    /// slots on one side; whether a match exists is left to chance). None if no
    /// open slot falls in range.
    fn seed_positional(&mut self, lo: usize, hi: usize, rng: &mut Rng) -> Option<usize> {
        let cands: ArrayVec<usize, MAX_N> = self
            .open
            .iter()
            .copied()
            .filter(|&qi| (lo..=hi).contains(&qi) && !self.is_decided(qi))
            .collect();
        if cands.is_empty() {
            return None;
        }
        let qi = rng.pick(&cands);
        let l = self.pick_answer(qi, false, &[], rng);
        self.put(qi, l);
        self.open.retain(|s| *s != qi);
        Some(qi)
    }

    /// Pick an answer for an open slot under the active constraints: never a
    /// banned letter, no new adjacent pair while `no_pairs`, and — when coverage
    /// still owes letters and the free slots are running out — a `missing` one.
    fn pick_answer(
        &self,
        qi: usize,
        must_cover: bool,
        missing: &[Answer],
        rng: &mut Rng,
    ) -> Answer {
        let mut cands: ArrayVec<Answer, 5> = (0..self.oc as u8)
            .map(Answer::from)
            .filter(|&l| !(self.banned[l.idx()] || self.no_pairs && self.would_pair(qi, l)))
            .collect();
        if must_cover {
            cands = cands
                .iter()
                .copied()
                .filter(|l| missing.contains(l))
                .collect();
        }
        if cands.is_empty() {
            // Relax the no-pair rule (if the new pair breaks a shape, parametrize's
            // satisfy-check drops it to AnswerOf); a still-owed coverage letter keeps priority.
            cands = if must_cover {
                missing.iter().copied().collect()
            } else {
                (0..self.oc as u8)
                    .map(Answer::from)
                    .filter(|&l| !self.banned[l.idx()])
                    .collect()
            };
        }
        // Always non-empty after the relax: only OnlySame + NoOther ban a letter
        // (≤2), and oc ≥ 3, so an unbanned letter always remains.
        assert!(!cands.is_empty(), "no candidate answer (oc={})", self.oc);
        rng.pick(&cands)
    }

    /// The left index of an adjacent equal pair shared by the pair-shapes: an
    /// existing one if any, else a new one on two open slots (its letter chosen
    /// so it doesn't extend a neighbour into a triple). None if none exists and
    /// `no_pairs` forbids making one (ConsecIdent chose "no pair"). The position
    /// is randomized — a fixed pair slot biases which slot each pair-shape lands
    /// on, which in turn skews their survival through validation.
    fn ensure_pair(&mut self, rng: &mut Rng) -> Option<usize> {
        if let Some(a) = self.find_pair() {
            return Some(a);
        }
        if self.no_pairs {
            return None;
        }
        let mut starts: ArrayVec<usize, MAX_N> = (0..self.n.saturating_sub(1))
            .filter(|&a| !self.is_decided(a) && !self.is_decided(a + 1))
            .collect();
        rng.shuffle(&mut starts);
        for a in starts {
            let b = a + 1;
            let l = (0..self.oc as u8).map(Answer::from).find(|&l| {
                !self.banned[l.idx()]
                    && (a == 0 || !self.is_decided(a - 1) || self.solution[a - 1] != l)
                    && (b + 1 >= self.n || !self.is_decided(b + 1) || self.solution[b + 1] != l)
            });
            if let Some(l) = l {
                self.put(a, l);
                self.put(b, l);
                return Some(a);
            }
        }
        None
    }

    /// Left index of an existing adjacent equal pair among the decided answers.
    fn find_pair(&self) -> Option<usize> {
        (0..self.n.saturating_sub(1)).find(|&a| {
            self.is_decided(a) && self.is_decided(a + 1) && self.solution[a] == self.solution[a + 1]
        })
    }

    fn would_pair(&self, qi: usize, l: Answer) -> bool {
        (qi > 0 && self.is_decided(qi - 1) && self.solution[qi - 1] == l)
            || (qi + 1 < self.n && self.is_decided(qi + 1) && self.solution[qi + 1] == l)
    }
}

/// PARAMETRIZE: turn each slot's kind into a full QuestionType against the
/// finished answer key. Each kind is drawn against the solution (parameter-free
/// shapes map straight to their unit variant); a kind that won't fit its slot
/// (e.g. ClosestAfter on the last slot) is replaced with a fresh AnswerOf
/// (tallied into `fb.parametrize`). Shape-types were authored to fit, so they
/// never fall back here.
fn parametrize(
    n: usize,
    oc: usize,
    sol: &[Answer; MAX_N],
    kind_of: &[QuestionTypeKind; MAX_N],
    rng: &mut Rng,
    fb: &mut FallbackCounts,
) -> [QuestionType; MAX_N] {
    // Every answer is decided now, so reference types may target any question.
    let all = if n >= 16 { u16::MAX } else { (1u16 << n) - 1 };
    let mut types = [QuestionType::AnswerIsSelf; MAX_N];
    let mut placed: ArrayVec<QuestionType, MAX_N> = ArrayVec::new();
    for qi in 0..n {
        let kind = kind_of[qi];
        let mut chosen = None;
        // The kind must *fit* this slot (e.g. MostCommon needs a unique extreme);
        // if not, it's replaced rather than relocated.
        if solution_fits_type(kind, qi, sol, n, oc) {
            // measured: 10 draws saturates the fallback rate; more won't help.
            for _ in 0..10 {
                if let Some(qt) = random_type_params(kind, qi, n, oc, sol, all, rng)
                    && !placed.contains(&qt)
                    && solution_satisfies_type(&qt, qi, sol, n, oc)
                {
                    chosen = Some(qt);
                    break;
                }
            }
        }
        let qt = chosen.unwrap_or_else(|| {
            fb.parametrize += 1;
            fresh_safe_type(n, qi, &placed, rng)
        });
        types[qi] = qt;
        placed.push(qt);
    }
    types
}

/// A satisfiable, not-yet-placed reference type for `qi`: a random AnswerOf to
/// another question, falling back to a random LetterDist once every AnswerOf
/// target is taken (small `n`). Both are unconditionally solution-satisfiable
/// (AnswerOf trivially; LetterDist's answer is `|sol[qi] - sol[j]| < oc`, always
/// in range). The exit is unreachable: blocking a target needs *both* its
/// AnswerOf and LetterDist placed, i.e. `2·(n-1)` types, but `placed` holds only
/// `qi ≤ n-1` here — so some target is always free.
fn fresh_safe_type(n: usize, qi: usize, placed: &[QuestionType], rng: &mut Rng) -> QuestionType {
    for make_qt in [make_answer_of, make_letter_dist] {
        let mut targets: ArrayVec<u8, MAX_N> =
            (0..n).filter(|&i| i != qi).map(|i| i as u8).collect();
        while !targets.is_empty() {
            let (k, target_qi) = rng.pick_kv_arrayvec(&targets);
            let qt = make_qt(target_qi);
            if !placed.contains(&qt) {
                return qt;
            }
            targets.swap_remove(k);
        }
    }
    unreachable!("not possible: no free AnswerOf/LetterDist target");

    fn make_answer_of(target: u8) -> QuestionType {
        QuestionType::AnswerOf {
            question_index: target,
        }
    }
    fn make_letter_dist(target: u8) -> QuestionType {
        QuestionType::LetterDist {
            question_index: target,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::difficulty::PROFILES;

    #[test]
    fn compose_is_internally_consistent() {
        use crate::build::solution_satisfies_type;
        for level in 0..6 {
            let p = &PROFILES[level];
            let mut rng = Rng::new(level as u32 * 7919 + 1);
            for _ in 0..500 {
                let c = compose(
                    &RECIPES[level],
                    p.question_count,
                    p.option_count,
                    &mut rng,
                    &mut ComposeStats::default(),
                );
                assert_eq!(c.n, p.question_count);
                // Every placed type must be satisfied by the composed answer
                // key — otherwise fill_options couldn't build a valid puzzle.
                for qi in 0..c.n {
                    assert!(
                        solution_satisfies_type(&c.types[qi], qi, &c.solution, c.n, p.option_count),
                        "L{} slot {qi}: {:?} unsatisfied",
                        level + 1,
                        c.types[qi],
                    );
                    // No two questions may be identical (same kind + params).
                    for qj in 0..qi {
                        assert_ne!(
                            c.types[qi],
                            c.types[qj],
                            "L{} slots {qj}/{qi}: identical question {:?}",
                            level + 1,
                            c.types[qi],
                        );
                    }
                }
            }
        }
    }
}
