//! Puzzle generation pipeline.
//!
//! `generate_skeleton()` picks the question kinds, decides the answer key and a kind per
//! slot (shapes seed the structure they need), then turns each kind into a full
//! `QuestionType` against that key. `generate()` wraps that with `fill_options`
//! and `validate_and_repair`. The shared question-type helpers (fit checks,
//! parametrization, claim JSON) live at the bottom of this file.

mod repair;

use arrayvec::ArrayVec;

use crate::build::{
    FallbackCounts, GenerateResult, SkeletonStats, Stats, assert_accepted, count_letter,
    fill_options, run_hint_engine, solution_satisfies_type, to_optional,
};
use crate::check_answer::{check_answer, check_unambiguous_answer};
use crate::check_answerable::{answerable, check_answerable};
use crate::check_form::check_form;
use crate::rng::Rng;
use crate::solve_brute::solve;
use crate::types::QuestionTypeKind::*;
use crate::types::*;

/// Per-level recipe: `required` + `allowed` + `caps` fully describe how a level's
/// question set is selected.
pub struct LevelRecipe {
    /// Types that must appear, with how many of each.
    pub required: &'static [(QuestionTypeKind, usize)],
    /// The pool the remaining slots are filled from.
    pub allowed: &'static [QuestionTypeKind],
    /// Per-type max occurrences (default 3; unit variants 1 — see `DEFAULT_CAPS`).
    pub caps: [u8; 32],
    /// Lookahead search depth the engine may use to accept a puzzle (the
    /// `stop_deducing_after_n_results` budget). 0 = pure deduction only; larger
    /// admits harder puzzles. Ramps from intro (shallow) to late (deep).
    pub lookahead_depth: usize,
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
    // question, so cap them at 1.
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

/// Per-level recipes, tuned via type-stats. Indexed by level-1.
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
        lookahead_depth: 1,
    },
    // L2
    LevelRecipe {
        required: &[],
        allowed: &[CountAnswer, AnswerOf, AnswerIsSelf, FirstWith, LastWith],
        caps: DEFAULT_CAPS,
        lookahead_depth: 1,
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
        lookahead_depth: 6,
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
        lookahead_depth: 6,
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
        lookahead_depth: 6,
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
        lookahead_depth: 6,
    },
];

pub struct Skeleton {
    pub types: [QuestionType; MAX_N],
    pub solution: [Answer; MAX_N],
    pub n: usize,
}

/// Pick the kinds, give each a slot and answer, then turn each kind into a full
/// `QuestionType` against the finished answer key. Always succeeds (fallbacks
/// guarantee a result); uniqueness is checked later by the caller.
/// Telemetry (skeleton count + per-phase AnswerOf fallbacks) is tallied into `skeleton_stats`.
pub fn generate_skeleton(
    recipe: &LevelRecipe,
    n: usize,
    oc: usize,
    rng: &mut Rng,
    skeleton_stats: &mut SkeletonStats,
) -> Skeleton {
    skeleton_stats.count += 1;
    let fallbacks = &mut skeleton_stats.fallbacks;
    let kinds = select_kinds(recipe, n, rng);
    let SolutionAndKinds { solution, kind_of } =
        SolutionAndKindsBuilder::new(n, oc).build(&kinds, rng, fallbacks);
    let types = parametrize(recipe, n, oc, &solution, &kind_of, rng, fallbacks);

    Skeleton { types, solution, n }
}

/// Like [`generate_skeleton`], but reuses a fixed answer key instead of authoring a new one.
/// Selects fresh question kinds and assigns them to slots the given solution
/// already supports ([`assign_kinds_to_solution`]), then parametrizes against it.
/// A kind the solution can't host as a shape is demoted to `AnswerOf`; the answer
/// key itself is never touched. `generate` uses this for every attempt after the
/// first, so retries vary the questions while the solution stays fixed.
pub fn regenerate_skeleton(
    recipe: &LevelRecipe,
    n: usize,
    oc: usize,
    solution: &[Answer; MAX_N],
    rng: &mut Rng,
    skeleton_stats: &mut SkeletonStats,
) -> Skeleton {
    skeleton_stats.count += 1;
    let fallbacks = &mut skeleton_stats.fallbacks;
    let kinds = select_kinds(recipe, n, rng);
    let kind_of = assign_kinds_to_solution(n, oc, solution, &kinds, rng, fallbacks);
    let types = parametrize(recipe, n, oc, solution, &kind_of, rng, fallbacks);

    Skeleton {
        types,
        solution: *solution,
        n,
    }
}

/// Full generation: decide the answer key once, then search for questions that
/// make it solvable + unique. The first skeleton fixes the key; every later
/// attempt `regenerate_skeleton`s — same key, fresh questions — so retries vary
/// only the questions, never the solution.
///
/// That ordering is deliberate: the key is decided by the first skeleton's RNG
/// draws (`select_kinds` + `build`) before any validation runs, so it's a pure
/// function of (seed, RNG, `RECIPES`, `build`) — independent of `fill_options`,
/// `validate_and_repair`, `deduce`, lookahead, and repair. Engine improvements
/// therefore never rewrite an existing seed's answer key.
///
/// Each attempt encodes option values (`fill_options`) and validates
/// (`validate_and_repair`). `None` if no accepted puzzle turns up within the
/// budget — the caller decides whether that's fatal (the daily bake panics;
/// diagnostics count it).
pub fn generate(
    recipe: &LevelRecipe,
    n: usize,
    oc: usize,
    rng: &mut Rng,
    max_regenerations: usize,
    stats: &mut Stats,
    label: &str,
) -> Option<GenerateResult> {
    let mut solution: Option<[Answer; MAX_N]> = None;
    // Iterate `1 + max_regenerations` times: the first builds the key-fixing
    // skeleton, each retry regenerates only the questions for that key.
    for _ in 0..=max_regenerations {
        let skeleton = match solution {
            None => generate_skeleton(recipe, n, oc, rng, &mut stats.v2_skeleton),
            Some(sol) => regenerate_skeleton(recipe, n, oc, &sol, rng, &mut stats.v2_skeleton),
        };
        solution = Some(skeleton.solution);
        let mut fp = fill_options(
            &skeleton.types,
            &skeleton.solution,
            skeleton.n,
            oc,
            rng,
            false,
        );
        // generate_skeleton/regenerate_skeleton should never produce a malformed answer key: references
        // are in range by construction, fill_options encodes values in range, and
        // NoOtherHasAnswer's `cover_all` keeps every letter present (the one
        // construct-side hazard). Assert it — a malformed key panics deep in
        // validate_and_repair otherwise, and a silent reject would mask the bug.
        let form_errors = check_form(&fp, Some(&skeleton.solution[..skeleton.n]));
        assert!(
            form_errors.is_empty(),
            "generate_skeleton/regenerate_skeleton produced a malformed answer key (n={}, oc={oc}): {form_errors:?}\n  types={:?}\n  sol={:?}",
            skeleton.n,
            &skeleton.types[..skeleton.n],
            &skeleton.solution[..skeleton.n]
        );
        if let Verdict::Accepted = validate_and_repair(
            &mut fp,
            &skeleton.solution,
            skeleton.n,
            recipe.lookahead_depth,
            rng,
            stats,
            label,
        ) {
            // Gate: an accepted puzzle must have a unique answer per question — the
            // builder skips count-type ties, fill avoids SameAs sharer-distractors,
            // and repair's keep-gate refuses ambiguous edits. Assert it rather than
            // emit an ambiguous puzzle (a silent emit would mask a gen/repair bug).
            let sol = &skeleton.solution[..skeleton.n];
            for qi in 0..skeleton.n {
                if let Some(reason) = check_unambiguous_answer(&fp, qi, sol)
                    .or_else(|| check_answerable(&fp, qi, sol))
                {
                    panic!(
                        "[{label}] emitted an ambiguous puzzle at Q{}: {reason}",
                        qi + 1
                    );
                }
            }
            // Repair only edits options, so types are unchanged from `skeleton.types`;
            // read them off `fp`, the value we return anyway.
            return Some(GenerateResult {
                question_types: fp.question_types,
                fp,
                n: skeleton.n,
            });
        }
    }
    None
}

/// Outcome of [`validate_and_repair`]. `Stuck` carries the engine's partial
/// state (which questions it answered) — read by the `stuck` diagnostic.
pub enum Verdict {
    Accepted,
    Stuck { solved: usize, state: State },
}

/// Validate a generated puzzle, repairing it into an accepted one where possible.
/// The answer key is held fixed throughout — repair only edits distractors, so the
/// caller never has to re-author the key.
///
/// Engine-only first: assert the key is self-consistent, run deduce+lookahead, and
/// on a full solve confirm uniqueness. If the engine stalls, distractor repair runs
/// ([`repair::repair_distractors`]): it mutates a stuck question's distractors to
/// values its own rules can refute, gated by a cheap single-question `deduce` probe
/// so the full engine run is only paid when an edit looks promising. Brute `solve`
/// fires once on a completed puzzle and confirms uniqueness — rejecting (so the
/// caller regenerates) if a resume-from-state shortcut produced a non-unique result.
/// The key stays valid by construction (the correct option is never touched). A
/// puzzle repair can't crack is reported `Stuck`.
pub(crate) fn validate_and_repair(
    fp: &mut FlatPuzzle,
    solution: &[Answer; MAX_N],
    n: usize,
    lookahead_depth: usize,
    rng: &mut Rng,
    stats: &mut Stats,
    label: &str,
) -> Verdict {
    stats.attempts += 1;

    // The answer key must be self-consistent — a generation bug otherwise.
    let key_state = State {
        answers: to_optional(solution, n),
        eliminated: [fp.phantom_mask(); MAX_N],
    };
    for qi in 0..n {
        assert!(
            check_answer(fp, key_state, qi).is_valid(),
            "BUG: check_answer failed for Q{} type={:?} answer={:?} solution={:?}",
            qi + 1,
            fp.question_types[qi],
            solution[qi],
            &solution[..n]
        );
    }

    let (did_solve, state) = run_hint_engine(fp, stats, false, lookahead_depth);
    if did_solve {
        let solutions = solve(fp, None, 2);
        assert_accepted(fp, solution, n, solutions.len(), label);
        return Verdict::Accepted;
    }

    // Distractor repair, advancing `state` (the working stuck position) and accepting
    // the moment the puzzle completes.
    let mut state = state;
    if repair::repair_distractors(
        fp,
        solution,
        n,
        lookahead_depth,
        rng,
        stats,
        &mut state,
        label,
    ) {
        return Verdict::Accepted;
    }

    let solved = solved_count(&state, n);
    stats.fail_solve += 1;
    if solved == 0 {
        stats.fail_solve_zero_progress += 1;
    }
    Verdict::Stuck { solved, state }
}

fn solved_count(state: &State, n: usize) -> usize {
    (0..n).filter(|&qi| state.answers[qi].is_some()).count()
}

/// Required types first, then fill remaining slots by uniform random draw from
/// `allowed`, respecting caps. Panics if the pool can't fill `n` slots — that's a
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
        // Only caps are non-default; picked_kinds starts empty and counts at zero.
        KindSelection {
            cap_per_kind,
            ..Default::default()
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

/// The candidate answers — letters `A..` within the option range.
fn letters(oc: usize) -> impl Iterator<Item = Answer> {
    (0..oc as u8).map(Answer::from)
}

/// Scratch for the first skeleton phase: decides the answer key and the kind per
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
    no_pairs: bool,               // ConsecIdent seeded → no new adjacent equal pairs
    cover_all: bool,              // NoOther seeded → every letter must appear
}

struct SolutionAndKinds {
    solution: [Answer; MAX_N],
    kind_of: [QuestionTypeKind; MAX_N],
}

/// Placement order for kind assignment (lower = placed first). The structural
/// shapes come before everything else; among them, ConsecIdent precedes the
/// pair-sharers (Next/PrevSame) so it owns the single adjacent pair they reuse.
/// `build` (author) places shapes first to seed them while structure is free;
/// `assign_kinds_to_solution` (fixed key) places them first because they fit the
/// fewest slots. Same order, both reasons.
fn shape_rank(k: QuestionTypeKind) -> u8 {
    match k {
        OnlySame | NoOtherHasAnswer => 0,
        ConsecIdent => 1,
        NextSame | PrevSame => 2,
        _ => 3,
    }
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
    fn decide(&mut self, qi: usize, l: Answer) {
        self.solution[qi] = l;
        self.decided |= 1 << qi;
    }

    /// Whether `l` appears in any decided slot of the answer key so far.
    fn present(&self, l: Answer) -> bool {
        (0..self.n).any(|qi| self.is_decided(qi) && self.solution[qi] == l)
    }

    /// A letter not yet present and not banned, within the option range.
    fn fresh_letter(&self) -> Option<Answer> {
        letters(self.oc).find(|&l| !self.present(l) && !self.banned[l.idx()])
    }

    /// Decide the answer key and the kind per slot (returned as `SolutionAndKinds`).
    /// One pass over the selected kinds trickiest-first: a shape seeds its
    /// structure and claims its own slot; any other kind (or a shape that can't
    /// seed) takes a leftover slot and gets a randomized answer. The constraints
    /// the shapes impose — reserved letters (`banned`), no new adjacent pairs
    /// (`no_pairs`, ConsecIdent), every letter present (`cover_all`, NoOther) —
    /// are locals, threaded into the fill.
    fn build(
        mut self,
        kinds: &[QuestionTypeKind],
        rng: &mut Rng,
        fallbacks: &mut FallbackCounts,
    ) -> SolutionAndKinds {
        let mut kind_of = [QuestionTypeKind::AnswerIsSelf; MAX_N];
        self.open = (0..self.n).collect();
        rng.shuffle(&mut self.open);

        // Seed shapes trickiest-first (see `shape_rank`).
        let mut ordered: ArrayVec<QuestionTypeKind, MAX_N> = kinds.iter().copied().collect();
        ordered.sort_by_key(|&k| shape_rank(k));
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
                fallbacks.assign_kinds += 1;
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
    /// when coverage still needs letters and the free slots are running out, it
    /// forces a still-absent one. Returns the slot.
    fn fill_one(&mut self, rng: &mut Rng) -> usize {
        let qi = self.open.pop().expect("a leftover slot per kind");
        if !self.is_decided(qi) {
            let missing: ArrayVec<Answer, 5> = if self.cover_all {
                letters(self.oc).filter(|&l| !self.present(l)).collect()
            } else {
                ArrayVec::new()
            };
            // `open` now holds only future fill slots, so its unset count plus
            // this slot is exactly how many free choices remain for coverage.
            let free_left = self.open.iter().filter(|&&x| !self.is_decided(x)).count() + 1;
            let must_cover = free_left <= missing.len();
            let l = self.pick_answer(qi, must_cover, &missing, rng);
            self.decide(qi, l);
        }
        qi
    }

    /// Seed OnlySame: a fresh letter on two still-open slots (banned → it stays
    /// at exactly two). Returns the host slot; the partner keeps its answer and
    /// is left open for a later kind.
    fn seed_only_same(&mut self) -> Option<usize> {
        let l = self.fresh_letter()?;
        let mut free = self.open.iter().copied().filter(|&x| !self.is_decided(x));
        let host = free.next()?;
        let partner = free.next()?;
        self.decide(host, l);
        self.decide(partner, l);
        self.banned[l.idx()] = true;
        self.open.retain(|x| *x != host);
        Some(host)
    }

    /// Seed NoOtherHasAnswer: a fresh unique letter (banned → stays at one), and
    /// set `cover_all` so pass 2 places every other letter (else an absent letter
    /// is also vacuously "held by no other" → ambiguous). Returns the host slot.
    fn seed_no_other(&mut self) -> Option<usize> {
        let l = self.fresh_letter()?;
        let host = self.open.iter().copied().find(|&x| !self.is_decided(x))?;
        self.decide(host, l);
        self.banned[l.idx()] = true;
        self.cover_all = true;
        self.open.retain(|x| *x != host);
        Some(host)
    }

    /// Seed ConsecIdent: ensure one adjacent equal pair (or none ~10% → answer
    /// None), seeded on a *separate* host whose own answer doesn't start a pair.
    /// `no_pairs` then stops pass 2 (and the pair-sharers) adding another, so the
    /// pair count stays ≤ 1. Returns the host slot.
    fn seed_consec(&mut self, rng: &mut Rng) -> Option<usize> {
        let want_none = rng.next_f64() < 0.10;
        let host = if want_none {
            self.open.iter().copied().find(|&x| !self.is_decided(x))?
        } else {
            let a = self.ensure_pair(rng)?;
            self.open
                .iter()
                .copied()
                .find(|&x| x != a && x != a + 1 && !self.is_decided(x))?
        };
        let l = letters(self.oc)
            .find(|&l| !self.banned[l.idx()] && !self.would_pair(host, l))
            .unwrap_or(Answer::A);
        self.decide(host, l);
        self.no_pairs = true;
        self.open.retain(|x| *x != host);
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

    /// Seed PrevSame: mirror of [`Self::seed_next_same`], referent *earlier* in the sequence.
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
        let candidates: ArrayVec<usize, MAX_N> = self
            .open
            .iter()
            .copied()
            .filter(|&qi| (lo..=hi).contains(&qi) && !self.is_decided(qi))
            .collect();
        if candidates.is_empty() {
            return None;
        }
        let qi = rng.pick(&candidates);
        let l = self.pick_answer(qi, false, &[], rng);
        self.decide(qi, l);
        self.open.retain(|x| *x != qi);
        Some(qi)
    }

    /// Pick an answer for an open slot under the active constraints: never a
    /// banned letter, no new adjacent pair while `no_pairs`, and — when coverage
    /// still needs letters and the free slots are running out — a `missing` one.
    fn pick_answer(
        &self,
        qi: usize,
        must_cover: bool,
        missing: &[Answer],
        rng: &mut Rng,
    ) -> Answer {
        // Candidate answers: legal letters, narrowed below as coverage tightens.
        let mut cands: ArrayVec<Answer, 5> = letters(self.oc)
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
            // satisfy-check drops it to AnswerOf); a still-missing coverage letter keeps priority.
            cands = if must_cover {
                missing.iter().copied().collect()
            } else {
                letters(self.oc)
                    .filter(|&l| !self.banned[l.idx()])
                    .collect()
            };
        }
        // Always non-empty after the relax: only OnlySame + NoOther ban a letter
        // (≤2), and oc ≥ 3, so an unbanned letter always remains.
        assert!(!cands.is_empty(), "no candidate answer (oc={})", self.oc);
        rng.pick(&cands)
    }

    /// The start index of an adjacent equal pair shared by the pair-shapes: an
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
            let l = letters(self.oc).find(|&l| {
                !self.banned[l.idx()]
                    && (a == 0 || !self.is_decided(a - 1) || self.solution[a - 1] != l)
                    && (b + 1 >= self.n || !self.is_decided(b + 1) || self.solution[b + 1] != l)
            });
            if let Some(l) = l {
                self.decide(a, l);
                self.decide(b, l);
                return Some(a);
            }
        }
        None
    }

    /// Start index of an existing adjacent equal pair among the decided answers.
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

/// Assign each selected kind to a slot of a *fixed* answer key — the companion of
/// [`SolutionAndKindsBuilder::build`] for the reuse path, deciding only kinds,
/// never the key. Mirrors `build`'s shape: walk the kinds in [`shape_rank`] order
/// (shapes first — here because they fit the fewest slots of the fixed key) and
/// claim the first shuffled slot the solution supports (`solution_fits_kind`).
/// A kind with no fitting slot left is demoted to `AnswerOf` on a leftover
/// (`fallbacks.assign_kinds`), which fits anywhere once the key is decided.
fn assign_kinds_to_solution(
    n: usize,
    oc: usize,
    solution: &[Answer; MAX_N],
    kinds: &[QuestionTypeKind],
    rng: &mut Rng,
    fallbacks: &mut FallbackCounts,
) -> [QuestionTypeKind; MAX_N] {
    let mut kind_of = [QuestionTypeKind::AnswerIsSelf; MAX_N];
    let mut open: ArrayVec<usize, MAX_N> = (0..n).collect();
    rng.shuffle(&mut open);

    let mut ordered: ArrayVec<QuestionTypeKind, MAX_N> = kinds.iter().copied().collect();
    ordered.sort_by_key(|&k| shape_rank(k));

    for &k in &ordered {
        // `open` is shuffled, so the first supported slot is a random fitting one.
        match open
            .iter()
            .position(|&qi| solution_fits_kind(k, qi, solution, n, oc))
        {
            Some(idx) => {
                kind_of[open[idx]] = k;
                open.swap_remove(idx);
            }
            None => {
                let qi = open.pop().expect("a leftover slot per kind");
                kind_of[qi] = AnswerOf;
                fallbacks.assign_kinds += 1;
            }
        }
    }
    kind_of
}

/// Turn each slot's kind into a full QuestionType against the finished answer key.
/// Each kind is drawn against the solution (parameter-free shapes map straight to
/// their unit variant). A kind that won't fit its slot (e.g. ClosestAfter on the
/// last slot, or MostCommon with a tied extreme) is replaced — first by another
/// fitting kind from the level's pool (`fallbacks.reserve`), and only if none fit
/// by a generic AnswerOf/LetterDist (`fallbacks.backstop`). Shape-types were
/// authored to fit, so they never fall back here.
fn parametrize(
    recipe: &LevelRecipe,
    n: usize,
    oc: usize,
    sol: &[Answer; MAX_N],
    kind_of: &[QuestionTypeKind; MAX_N],
    rng: &mut Rng,
    fallbacks: &mut FallbackCounts,
) -> [QuestionType; MAX_N] {
    // Every answer is decided now, so reference types may target any question.
    let all_targets = if n >= 16 { u16::MAX } else { (1u16 << n) - 1 };
    let mut types = [QuestionType::AnswerIsSelf; MAX_N];
    let mut placed: ArrayVec<QuestionType, MAX_N> = ArrayVec::new();

    for qi in 0..n {
        // Three tiers, in order of preference: the slot's planned kind; else a
        // fitting kind from the level's own pool; else the generic backstop
        // (AnswerOf/LetterDist, which always fits). The two fallbacks are counted
        // separately.
        let qt = if let Some(qt) =
            try_parametrize_kind(kind_of[qi], qi, n, oc, sol, all_targets, &placed, rng)
        {
            qt
        } else if let Some(qt) =
            pick_reserve(recipe, qi, n, oc, sol, all_targets, kind_of, &placed, rng)
        {
            fallbacks.reserve += 1;
            qt
        } else {
            fallbacks.backstop += 1;
            fresh_fallback_type(n, qi, &placed, rng)
        };
        types[qi] = qt;
        placed.push(qt);
    }
    types
}

/// A slot's planned kind didn't fit (e.g. MostCommon with a tied extreme). Pick a
/// different kind from the level's pool that *does* fit here — so the slot keeps a
/// level-appropriate question instead of degrading to the generic backstop. Skips
/// kinds already at their cap; `None` if nothing in the pool fits.
fn pick_reserve(
    recipe: &LevelRecipe,
    qi: usize,
    n: usize,
    oc: usize,
    sol: &[Answer; MAX_N],
    all_targets: u16,
    kind_of: &[QuestionTypeKind; MAX_N],
    placed: &[QuestionType],
    rng: &mut Rng,
) -> Option<QuestionType> {
    let mut pool: ArrayVec<QuestionTypeKind, 32> = recipe.allowed.iter().copied().collect();
    rng.shuffle(&mut pool);
    for r in pool {
        // How many `r`s are already committed: placed so far, plus those still
        // planned in later slots. Adding one here must keep the total within cap.
        let committed = placed.iter().filter(|qt| qt.kind() == r).count()
            + kind_of[qi + 1..n].iter().filter(|&&k| k == r).count();
        if committed >= usize::from(recipe.caps[r as usize]) {
            continue;
        }
        if let Some(qt) = try_parametrize_kind(r, qi, n, oc, sol, all_targets, placed, rng) {
            return Some(qt);
        }
    }
    None
}

/// Turn `kind` into a concrete `QuestionType` for slot `qi`, or `None` if it can't:
/// the kind must *fit* the solution shape (e.g. MostCommon needs a unique extreme),
/// and a random parametrization must be unique (not already `placed`), satisfied by
/// the key, and leave a unique answer.
fn try_parametrize_kind(
    kind: QuestionTypeKind,
    qi: usize,
    n: usize,
    oc: usize,
    sol: &[Answer; MAX_N],
    all_targets: u16,
    placed: &[QuestionType],
    rng: &mut Rng,
) -> Option<QuestionType> {
    if !solution_fits_kind(kind, qi, sol, n, oc) {
        return None;
    }
    // measured: 10 draws saturates the fallback rate; more won't help.
    for _ in 0..10 {
        if let Some(qt) = random_type_params(kind, qi, n, oc, sol, all_targets, rng)
            && !placed.contains(&qt)
            && solution_satisfies_type(&qt, qi, sol, n, oc)
            && answerable(&qt, sol, n, oc)
        {
            return Some(qt);
        }
    }
    None
}

/// A satisfiable, not-yet-placed reference type for `qi`: a random AnswerOf to
/// another question, falling back to a random LetterDist once every AnswerOf
/// target is taken (small `n`).
fn fresh_fallback_type(
    n: usize,
    qi: usize,
    placed: &[QuestionType],
    rng: &mut Rng,
) -> QuestionType {
    // Both candidate types are unconditionally solution-satisfiable: AnswerOf
    // trivially, and LetterDist's answer `|sol[qi] - sol[j]| < oc` is always in range.
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
    // Unreachable: blocking a target needs *both* its AnswerOf and LetterDist
    // placed (`2·(n-1)` types), but `placed` holds only `qi ≤ n-1` here — so some
    // target is always free.
    unreachable!("no free AnswerOf/LetterDist target");

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

// ── Shared question-type helpers (fit checks, parametrization, claim JSON) ──

pub fn format_claim_qt(qt: &QuestionType) -> serde_json::Value {
    let type_name = match qt {
        QuestionType::CountAnswer { .. } => "CountAnswer",
        QuestionType::CountConsonant => "CountConsonant",
        QuestionType::CountVowel => "CountVowel",
        QuestionType::CountAnswerAfter { .. } => "CountAnswerAfter",
        QuestionType::CountAnswerBefore { .. } => "CountAnswerBefore",
        QuestionType::AnswerOf { .. } => "AnswerOf",
        QuestionType::FirstWith { .. } => "FirstWith",
        QuestionType::LastWith { .. } => "LastWith",
        QuestionType::MostCommon => "MostCommon",
        QuestionType::LeastCommon => "LeastCommon",
        QuestionType::MostCommonCount => "MostCommonCount",
        QuestionType::NoOtherHasAnswer => "NoOtherHasAnswer",
        QuestionType::ConsecIdent => "ConsecIdent",
        QuestionType::OnlyOdd { .. } => "OnlyOdd",
        QuestionType::OnlyEven { .. } => "OnlyEven",
        QuestionType::EqualCount { .. } => "EqualCount",
        QuestionType::ClosestAfter { .. } => "ClosestAfter",
        QuestionType::ClosestBefore { .. } => "ClosestBefore",
        QuestionType::SameAsWhich { .. } => "SameAsWhich",
        _ => "Unknown",
    };
    let mut obj = serde_json::Map::new();
    obj.insert("type".into(), serde_json::json!(type_name));
    match *qt {
        QuestionType::CountAnswer { answer }
        | QuestionType::FirstWith { answer }
        | QuestionType::LastWith { answer }
        | QuestionType::OnlyOdd { answer }
        | QuestionType::OnlyEven { answer }
        | QuestionType::EqualCount { answer } => {
            obj.insert(
                "answer".into(),
                serde_json::json!(answer.as_char().to_string()),
            );
        }
        QuestionType::CountAnswerAfter {
            answer,
            after_index,
        } => {
            obj.insert(
                "answer".into(),
                serde_json::json!(answer.as_char().to_string()),
            );
            obj.insert("afterIndex".into(), serde_json::json!(after_index));
        }
        QuestionType::CountAnswerBefore {
            answer,
            before_index,
        } => {
            obj.insert(
                "answer".into(),
                serde_json::json!(answer.as_char().to_string()),
            );
            obj.insert("beforeIndex".into(), serde_json::json!(before_index));
        }
        QuestionType::ClosestAfter {
            answer,
            after_index,
        } => {
            obj.insert(
                "answer".into(),
                serde_json::json!(answer.as_char().to_string()),
            );
            obj.insert("afterIndex".into(), serde_json::json!(after_index));
        }
        QuestionType::ClosestBefore {
            answer,
            before_index,
        } => {
            obj.insert(
                "answer".into(),
                serde_json::json!(answer.as_char().to_string()),
            );
            obj.insert("beforeIndex".into(), serde_json::json!(before_index));
        }
        QuestionType::AnswerOf { question_index }
        | QuestionType::LetterDist { question_index }
        | QuestionType::SameAsWhich { question_index } => {
            obj.insert("questionIndex".into(), serde_json::json!(question_index));
        }
        _ => {}
    }
    serde_json::Value::Object(obj)
}

fn is_constrained_type(kind: QuestionTypeKind) -> bool {
    matches!(
        kind,
        QuestionTypeKind::ConsecIdent
            | QuestionTypeKind::NoOtherHasAnswer
            | QuestionTypeKind::OnlySame
            | QuestionTypeKind::OnlyOdd
            | QuestionTypeKind::OnlyEven
    )
}

/// Checks whether the solution has the properties needed for this type at this position.
pub(crate) fn solution_fits_kind(
    kind: QuestionTypeKind,
    qi: usize,
    sol: &[Answer; MAX_N],
    n: usize,
    oc: usize,
) -> bool {
    match kind {
        // MC/LC answerability (unique extreme) lives in `check_answerable` — the one
        // build-time source, shared with parametrize and `refpuzzle check`.
        QuestionTypeKind::LeastCommon => answerable(&QuestionType::LeastCommon, sol, n, oc),
        QuestionTypeKind::MostCommon => answerable(&QuestionType::MostCommon, sol, n, oc),
        QuestionTypeKind::SameAs => {
            // Pool capacity for SameAs at qi depends on how many questions share qi's answer:
            //   same_count == 1 (qi is unique): correct = null, pool = n-1 other Qs, no null.
            //   same_count >= 2: correct = a same-answer Q, pool = (n - same_count) differing-Q + 1 null.
            // We need pool >= oc - 1 (one distractor per non-correct option).
            let same_count = count_letter(sol, sol[qi], n) as usize;
            let pool = if same_count == 1 {
                n - 1
            } else {
                n - same_count + 1
            };
            pool >= oc - 1
        }
        QuestionTypeKind::SameAsWhich => true,
        QuestionTypeKind::NoOtherHasAnswer => {
            count_letter(sol, sol[qi], n) == 1
                && LETTERS[..oc]
                    .iter()
                    .all(|&l| l == sol[qi] || count_letter(sol, l, n) >= 1)
        }
        QuestionTypeKind::EqualCount => true,
        _ if is_constrained_type(kind) => solution_satisfies_type_for_kind(kind, qi, sol, n),
        _ => true,
    }
}

fn solution_satisfies_type_for_kind(
    kind: QuestionTypeKind,
    qi: usize,
    sol: &[Answer; MAX_N],
    n: usize,
) -> bool {
    match kind {
        QuestionTypeKind::ConsecIdent => {
            let mut pairs = 0;
            for i in 0..n.saturating_sub(1) {
                if sol[i] == sol[i + 1] {
                    pairs += 1;
                }
            }
            pairs <= 1
        }
        QuestionTypeKind::NoOtherHasAnswer => count_letter(sol, sol[qi], n) == 1,
        QuestionTypeKind::OnlySame => {
            let mut m = 0;
            for i in 0..n {
                if i != qi && sol[i] == sol[qi] {
                    m += 1;
                }
            }
            m <= 1
        }
        QuestionTypeKind::OnlyOdd | QuestionTypeKind::OnlyEven => {
            let parity = if kind == QuestionTypeKind::OnlyOdd {
                1
            } else {
                0
            };
            LETTERS.iter().any(|&letter| {
                let mut m = 0;
                for i in 0..n {
                    if (i + 1) % 2 == parity && sol[i] == letter {
                        m += 1;
                    }
                }
                m <= 1
            })
        }
        _ => true,
    }
}

pub(crate) fn random_type_params(
    kind: QuestionTypeKind,
    qi: usize,
    n: usize,
    option_count: usize,
    solution: &[Answer; MAX_N],
    assigned: u16,
    rng: &mut Rng,
) -> Option<QuestionType> {
    match kind {
        QuestionTypeKind::CountAnswer => Some(QuestionType::CountAnswer {
            answer: rng.pick_letter(option_count),
        }),
        QuestionTypeKind::CountAnswerBefore => {
            // Need before_index with at least oc distinct count values (0..=before_index).
            if n < option_count {
                return None;
            }
            Some(QuestionType::CountAnswerBefore {
                answer: rng.pick_letter(option_count),
                before_index: rng.int(option_count as i32 - 1, n as i32 - 1) as u8,
            })
        }
        QuestionTypeKind::CountAnswerAfter => {
            // Need after_index with at least oc distinct count values (0..=n-1-after_index).
            if n < option_count {
                return None;
            }
            Some(QuestionType::CountAnswerAfter {
                answer: rng.pick_letter(option_count),
                after_index: rng.int(0, n as i32 - option_count as i32) as u8,
            })
        }
        QuestionTypeKind::CountVowel => Some(QuestionType::CountVowel),
        QuestionTypeKind::CountConsonant => Some(QuestionType::CountConsonant),
        QuestionTypeKind::MostCommonCount => Some(QuestionType::MostCommonCount),
        QuestionTypeKind::AnswerOf => {
            let mut pool = [0u8; MAX_N];
            let mut plen = 0;
            for j in 0..n {
                if j != qi && (assigned & (1 << j)) != 0 {
                    pool[plen] = j as u8;
                    plen += 1;
                }
            }
            if plen == 0 {
                return None;
            }
            Some(QuestionType::AnswerOf {
                question_index: rng.pick(&pool[..plen]),
            })
        }
        QuestionTypeKind::LetterDist => {
            let mut pool = [0u8; MAX_N];
            let mut plen = 0;
            for j in 0..n {
                if j != qi && (assigned & (1 << j)) != 0 {
                    pool[plen] = j as u8;
                    plen += 1;
                }
            }
            if plen == 0 {
                for j in 0..n {
                    if j != qi {
                        pool[plen] = j as u8;
                        plen += 1;
                    }
                }
            }
            Some(QuestionType::LetterDist {
                question_index: rng.pick(&pool[..plen]),
            })
        }
        QuestionTypeKind::ClosestAfter => {
            // Need after_index with at least oc distinct option values
            // (positions after_index+1..n, plus null).
            if n < option_count {
                return None;
            }
            Some(QuestionType::ClosestAfter {
                after_index: rng.int(0, n as i32 - option_count as i32) as u8,
                answer: rng.pick_letter(option_count),
            })
        }
        QuestionTypeKind::ClosestBefore => {
            // Need before_index with at least oc distinct option values
            // (positions 0..before_index, plus null).
            if n < option_count {
                return None;
            }
            Some(QuestionType::ClosestBefore {
                before_index: rng.int(option_count as i32 - 1, n as i32 - 1) as u8,
                answer: rng.pick_letter(option_count),
            })
        }
        QuestionTypeKind::FirstWith => Some(QuestionType::FirstWith {
            answer: rng.pick_letter(option_count),
        }),
        QuestionTypeKind::LastWith => Some(QuestionType::LastWith {
            answer: rng.pick_letter(option_count),
        }),
        QuestionTypeKind::PrevSame => {
            // Need oc distinct option values; pool size is qi + 1 (positions [0, qi) + null).
            if qi + 1 < option_count {
                return None;
            }
            Some(QuestionType::PrevSame)
        }
        QuestionTypeKind::NextSame => {
            // Need oc distinct option values; pool size is n - qi (positions (qi, n) + null).
            if n - qi < option_count {
                return None;
            }
            Some(QuestionType::NextSame)
        }
        QuestionTypeKind::OnlySame => Some(QuestionType::OnlySame),
        QuestionTypeKind::SameAs => {
            // "none" (no other question shares this answer) is a valid answer, so there is no
            // structural requirement. Capacity: with "none" as a value, oc distinct options
            // need n >= oc (none case: oc-1 index distractors from the n-1 other questions).
            if n < option_count {
                return None;
            }
            Some(QuestionType::SameAs)
        }
        QuestionTypeKind::ConsecIdent => Some(QuestionType::ConsecIdent),
        QuestionTypeKind::OnlyOdd | QuestionTypeKind::OnlyEven => {
            let answer = rng.pick_letter(option_count);
            Some(if kind == QuestionTypeKind::OnlyOdd {
                QuestionType::OnlyOdd { answer }
            } else {
                QuestionType::OnlyEven { answer }
            })
        }
        QuestionTypeKind::LeastCommon => Some(QuestionType::LeastCommon),
        QuestionTypeKind::MostCommon => Some(QuestionType::MostCommon),
        QuestionTypeKind::NoOtherHasAnswer => Some(QuestionType::NoOtherHasAnswer),
        QuestionTypeKind::EqualCount => {
            let ref_letter = rng.pick_letter(option_count);
            let ref_count = count_letter(solution, ref_letter, n);
            let has_match = LETTERS
                .iter()
                .any(|&l| l != ref_letter && count_letter(solution, l, n) == ref_count);
            if !has_match && rng.int(0, 4) > 1 {
                return None;
            }
            Some(QuestionType::EqualCount { answer: ref_letter })
        }
        QuestionTypeKind::AnswerIsSelf => Some(QuestionType::AnswerIsSelf),
        QuestionTypeKind::TrueStmt => {
            if option_count < 5 {
                return None;
            }
            Some(QuestionType::TrueStmt)
        }
        QuestionTypeKind::SameAsWhich => {
            let mut pool = [0u8; MAX_N];
            let mut plen = 0;
            for j in 0..n {
                if j != qi && (assigned & (1 << j)) != 0 {
                    pool[plen] = j as u8;
                    plen += 1;
                }
            }
            if plen == 0 {
                return None;
            }
            let ref_qi = rng.pick(&pool[..plen]) as usize;
            if solution[ref_qi] == solution[qi] {
                return None;
            }
            // Structural: another question must share ref's answer.
            // Capacity: need at least oc-1 questions whose answer differs from ref (distractors).
            let mut has_match = false;
            let mut distractor_count = 0usize;
            for j in 0..n {
                if j == qi {
                    continue;
                }
                if solution[j] == solution[ref_qi] {
                    if j != ref_qi {
                        has_match = true;
                    }
                } else {
                    distractor_count += 1;
                }
            }
            if !has_match || distractor_count < option_count - 1 {
                return None;
            }
            Some(QuestionType::SameAsWhich {
                question_index: ref_qi as u8,
            })
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::difficulty::PROFILES;

    #[test]
    fn generate_skeleton_is_internally_consistent() {
        use crate::build::solution_satisfies_type;
        for level in 0..6 {
            let p = &PROFILES[level];
            let mut rng = Rng::new(level as u32 * 7919 + 1);
            for _ in 0..500 {
                let skeleton = generate_skeleton(
                    &RECIPES[level],
                    p.question_count,
                    p.option_count,
                    &mut rng,
                    &mut SkeletonStats::default(),
                );
                assert_eq!(skeleton.n, p.question_count);
                // Every placed type must be satisfied by the generated answer
                // key — otherwise fill_options couldn't build a valid puzzle.
                for qi in 0..skeleton.n {
                    assert!(
                        solution_satisfies_type(
                            &skeleton.types[qi],
                            qi,
                            &skeleton.solution,
                            skeleton.n,
                            p.option_count
                        ),
                        "L{} slot {qi}: {:?} unsatisfied",
                        level + 1,
                        skeleton.types[qi],
                    );
                    // No two questions may be identical (same kind + params).
                    for qj in 0..qi {
                        assert_ne!(
                            skeleton.types[qi],
                            skeleton.types[qj],
                            "L{} slots {qj}/{qi}: identical question {:?}",
                            level + 1,
                            skeleton.types[qi],
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn regenerate_skeleton_preserves_solution_and_stays_consistent() {
        use crate::build::solution_satisfies_type;
        for level in 0..6 {
            let p = &PROFILES[level];
            let mut rng = Rng::new(level as u32 * 6271 + 3);
            for _ in 0..200 {
                // Author a solution once, then regenerate the skeleton against it
                // repeatedly — each regeneration must keep the key and yield a
                // consistent puzzle.
                let base = generate_skeleton(
                    &RECIPES[level],
                    p.question_count,
                    p.option_count,
                    &mut rng,
                    &mut SkeletonStats::default(),
                );
                for _ in 0..5 {
                    let skeleton = regenerate_skeleton(
                        &RECIPES[level],
                        p.question_count,
                        p.option_count,
                        &base.solution,
                        &mut rng,
                        &mut SkeletonStats::default(),
                    );
                    assert_eq!(skeleton.n, p.question_count);
                    assert_eq!(
                        &skeleton.solution[..skeleton.n],
                        &base.solution[..skeleton.n],
                        "L{} regeneration changed the answer key",
                        level + 1,
                    );
                    for qi in 0..skeleton.n {
                        assert!(
                            solution_satisfies_type(
                                &skeleton.types[qi],
                                qi,
                                &skeleton.solution,
                                skeleton.n,
                                p.option_count
                            ),
                            "L{} slot {qi}: {:?} unsatisfied",
                            level + 1,
                            skeleton.types[qi],
                        );
                        for qj in 0..qi {
                            assert_ne!(
                                skeleton.types[qi],
                                skeleton.types[qj],
                                "L{} slots {qj}/{qi}: identical question {:?}",
                                level + 1,
                                skeleton.types[qi],
                            );
                        }
                    }
                }
            }
        }
    }

    /// Every puzzle `generate` emits must be well-posed: each question has a
    /// unique answer for the key. This is the accept-gate's invariant, checked
    /// here independently (re-deriving the key by solving) so a gap in the
    /// checks — like the `EqualCount` tie that originally slipped through —
    /// fails the suite rather than shipping.
    #[test]
    fn generate_emits_only_well_posed_puzzles() {
        for level in 0..6 {
            let p = &PROFILES[level];
            let mut rng = Rng::new(level as u32 * 4099 + 11);
            let mut produced = 0;
            for _ in 0..40 {
                let Some(result) = generate(
                    &RECIPES[level],
                    p.question_count,
                    p.option_count,
                    &mut rng,
                    100,
                    &mut Stats::default(),
                    "test",
                ) else {
                    continue;
                };
                produced += 1;
                let fp = &result.fp;
                let n = fp.n;
                // the generator emits uniquely-solvable puzzles; recover the key by solving.
                let solutions = solve(fp, None, 2);
                assert_eq!(
                    solutions.len(),
                    1,
                    "L{} emitted a non-unique puzzle",
                    level + 1
                );
                let sol = &solutions[0][..n];
                for qi in 0..n {
                    assert!(
                        check_unambiguous_answer(fp, qi, sol).is_none(),
                        "L{} Q{}: {:?}",
                        level + 1,
                        qi + 1,
                        check_unambiguous_answer(fp, qi, sol),
                    );
                    assert!(
                        check_answerable(fp, qi, sol).is_none(),
                        "L{} Q{}: {:?}",
                        level + 1,
                        qi + 1,
                        check_answerable(fp, qi, sol),
                    );
                }
            }
            assert!(produced > 0, "L{} produced no puzzles", level + 1);
        }
    }
}
