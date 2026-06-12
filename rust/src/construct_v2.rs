//! v2 generation pipeline — parallel track. See docs/generation-redesign.md.
//!
//! `compose()` = SELECT + FORCE + PLACE, returning a puzzle skeleton (slotted
//! types + forced solution) *before* `fill_options` / `validate` / `repair`.
//! Built alongside the live generator (`construct.rs`); not yet wired into
//! production.
//!
//! Scaffold status: SELECT and PLACE work; FORCE is randomize-only — per-type
//! solution forcing (ConsecIdent pair, OnlySame exactly-twice, …) lands here
//! next. Until then the shape-types will be under-represented; that's the
//! signal the FORCE work will close.
#![allow(dead_code)] // scaffold; wired into the CLI / type-stats next

use arrayvec::ArrayVec;

use crate::build::solution_satisfies_type;
use crate::construct::{random_type_params, solution_fits_type};
use crate::rng::Rng;
use crate::types::QuestionTypeKind::*;
use crate::types::*;

/// v2 per-level recipe — selection-shaped, deliberately separate from
/// `DifficultyProfile`. The fill pool is the level's `allowed_types` (passed to
/// `compose`) for now; weighted overrides land here when we tune.
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

/// SELECT → FORCE → PLACE. Always succeeds (fallbacks guarantee a skeleton);
/// uniqueness is checked later by the caller.
pub fn compose(recipe: &LevelRecipe, n: usize, oc: usize, rng: &mut Rng) -> Composed {
    let kinds = select_kinds(recipe, n, rng);

    // FORCE: author the answer-key vector + pin the shape-types it can satisfy;
    // anything it can't pin flows to PLACE as `unpinned`.
    let mut b = Builder::new(n, oc);
    let mut unpinned = ArrayVec::<QuestionTypeKind, MAX_N>::new();
    for &k in &kinds {
        if !b.try_pin_shape(k, rng) {
            unpinned.push(k);
        }
    }
    b.fill_answers(rng); // decide the remaining answers without breaking forced ones
    b.place_unpinned_kinds(&unpinned, rng); // PLACE the rest into the unpinned slots
    b.replace_unsatisfiable_types(); // demote any slot the (rare) fill_answers corner left unsatisfiable

    Composed {
        types: b.types,
        solution: b.sol,
        n,
    }
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

/// Hardest-to-place-first ranking (see docs): edge-preferring Prev/Next before
/// the place-anywhere types. (OnlySame/NoOther are FORCE-pinned, so they don't
/// reach PLACE.)
fn place_rank(kind: QuestionTypeKind) -> u8 {
    match kind {
        PrevSame | NextSame => 0,
        _ => 1,
    }
}

/// Builds the answer-key vector and the type per slot. FORCE authors the answer
/// key + pins the shape-types it can satisfy; `fill_answers` decides the rest of the
/// answers without breaking the forced structure; `place_rest` assigns the rest.
struct Builder {
    n: usize,
    oc: usize,
    sol: [Answer; MAX_N],
    set: u16,    // bit p: sol[p] is decided
    pinned: u16, // bit p: types[p] is a forced shape-type
    types: [QuestionType; MAX_N],
    used: [u8; 5],                         // letter -> count among decided answers
    banned: [bool; 5], // letter `fill_answers` must avoid (reserved exact-count letter)
    suppress_pairs: bool, // ConsecIdent pinned → fill_answers adds no new adjacent pairs
    placed: ArrayVec<QuestionType, MAX_N>, // every assigned type — no duplicates
}

impl Builder {
    fn new(n: usize, oc: usize) -> Self {
        Builder {
            n,
            oc,
            sol: [Answer::A; MAX_N],
            set: 0,
            pinned: 0,
            types: [QuestionType::AnswerIsSelf; MAX_N],
            used: [0; 5],
            banned: [false; 5],
            suppress_pairs: false,
            placed: ArrayVec::new(),
        }
    }

    fn is_set(&self, p: usize) -> bool {
        self.set & (1 << p) != 0
    }
    fn is_pinned(&self, p: usize) -> bool {
        self.pinned & (1 << p) != 0
    }
    fn put(&mut self, p: usize, l: Answer) {
        self.sol[p] = l;
        self.set |= 1 << p;
        self.used[l.idx()] += 1;
    }
    fn pin(&mut self, p: usize, qt: QuestionType) {
        self.types[p] = qt;
        self.pinned |= 1 << p;
        self.placed.push(qt);
    }

    /// A letter not yet used and not banned, within the option range.
    fn fresh_letter(&self) -> Option<Answer> {
        (0..self.oc as u8)
            .map(Answer::from)
            .find(|&l| self.used[l.idx()] == 0 && !self.banned[l.idx()])
    }

    /// Positions with neither a decided answer nor a pinned type, shuffled.
    fn open_positions(&self, rng: &mut Rng) -> ArrayVec<usize, MAX_N> {
        let mut v: ArrayVec<usize, MAX_N> = (0..self.n)
            .filter(|&p| !self.is_set(p) && !self.is_pinned(p))
            .collect();
        rng.shuffle(&mut v);
        v
    }

    /// Pin a selected shape-type by authoring the answer key so the kind holds,
    /// then writing the type to a slot. These kinds depend on the global answer
    /// distribution (counts, adjacency), so they can't be slotted into an
    /// arbitrary solution — they must be built in here. Returns false for any
    /// kind we don't (yet) build, so the caller routes it to PLACE instead.
    fn try_pin_shape(&mut self, k: QuestionTypeKind, rng: &mut Rng) -> bool {
        match k {
            OnlySame => self.pin_only_same(rng),
            NoOtherHasAnswer => self.pin_no_other(rng),
            ConsecIdent => self.pin_consec(rng),
            _ => false,
        }
    }

    /// OnlySame holds when exactly one *other* slot shares this answer. Put a
    /// fresh letter on two open slots, ban it from the rest of `fill_answers` (keeping
    /// the count at exactly two), and pin OnlySame on one of them.
    fn pin_only_same(&mut self, rng: &mut Rng) -> bool {
        let Some(l) = self.fresh_letter() else {
            return false;
        };
        let open = self.open_positions(rng);
        if open.len() < 2 {
            return false;
        }
        self.put(open[0], l);
        self.put(open[1], l);
        self.banned[l.idx()] = true; // keep it at exactly two
        self.pin(open[0], QuestionType::OnlySame);
        true
    }

    /// NoOtherHasAnswer holds when this answer is unique. Put a fresh letter on
    /// one open slot, ban it everywhere else, and pin NoOtherHasAnswer there.
    fn pin_no_other(&mut self, rng: &mut Rng) -> bool {
        let Some(l) = self.fresh_letter() else {
            return false;
        };
        let open = self.open_positions(rng);
        let Some(&host) = open.first() else {
            return false;
        };
        self.put(host, l);
        self.banned[l.idx()] = true;
        self.pin(host, QuestionType::NoOtherHasAnswer);
        true
    }

    /// ConsecIdent needs at most one adjacent-equal pair: its answer is that
    /// pair, or None when there are zero. ~10% of the time we leave the solution
    /// pair-free (answer None); otherwise we seed exactly one pair (`make_pair`).
    /// Either way `suppress_pairs` then stops `fill_answers` adding more. Pins on `host`.
    fn pin_consec(&mut self, rng: &mut Rng) -> bool {
        let open = self.open_positions(rng);
        let Some(&host) = open.first() else {
            return false;
        };
        let want_none = rng.next_f64() < 0.10;
        if !want_none && !self.make_pair(host) {
            return false;
        }
        self.suppress_pairs = true;
        self.pin(host, QuestionType::ConsecIdent);
        true
    }

    /// Set an adjacent open pair (≠ `avoid`) to a letter chosen so it doesn't
    /// touch an existing equal neighbour (which would create a second pair).
    fn make_pair(&mut self, avoid: usize) -> bool {
        for i in 0..self.n.saturating_sub(1) {
            let (a, b) = (i, i + 1);
            if a == avoid || b == avoid || self.is_set(a) || self.is_set(b) {
                continue;
            }
            if self.is_pinned(a) || self.is_pinned(b) {
                continue;
            }
            let l = (0..self.oc as u8).map(Answer::from).find(|&l| {
                !self.banned[l.idx()]
                    && (a == 0 || !self.is_set(a - 1) || self.sol[a - 1] != l)
                    && (b + 1 >= self.n || !self.is_set(b + 1) || self.sol[b + 1] != l)
            });
            let Some(l) = l else { continue };
            self.put(a, l);
            self.put(b, l);
            return true;
        }
        false
    }

    /// Decide every still-open answer, avoiding banned letters. Adjacent equal
    /// answers are suppressed only when ConsecIdent is pinned (`suppress_pairs`),
    /// to hold its pair count; otherwise repeats are left to chance.
    fn fill_answers(&mut self, rng: &mut Rng) {
        for p in 0..self.n {
            if self.is_set(p) {
                continue;
            }
            let mut cands: ArrayVec<Answer, 5> = (0..self.oc as u8)
                .map(Answer::from)
                .filter(|&l| {
                    !(self.banned[l.idx()] || self.suppress_pairs && self.would_pair(p, l))
                })
                .collect();
            if cands.is_empty() {
                // relax the no-new-pair rule rather than fail (`repair` catches
                // any constraint this breaks)
                cands = (0..self.oc as u8)
                    .map(Answer::from)
                    .filter(|&l| !self.banned[l.idx()])
                    .collect();
            }
            let l = if cands.is_empty() {
                Answer::A
            } else {
                rng.pick(&cands)
            };
            self.put(p, l);
        }
    }

    fn would_pair(&self, p: usize, l: Answer) -> bool {
        (p > 0 && self.is_set(p - 1) && self.sol[p - 1] == l)
            || (p + 1 < self.n && self.is_set(p + 1) && self.sol[p + 1] == l)
    }

    /// Safety net: if the (rare) fill_answers corner left a slot's type unsatisfiable
    /// against the final answer key, demote it to a fresh AnswerOf (every slot
    /// is assigned by now, so we dedup against the live types directly).
    fn replace_unsatisfiable_types(&mut self) {
        for qi in 0..self.n {
            if !solution_satisfies_type(&self.types[qi], qi, &self.sol, self.n, self.oc) {
                let qt = (0..self.n)
                    .filter(|&j| j != qi)
                    .map(|j| QuestionType::AnswerOf {
                        question_index: j as u8,
                    })
                    .find(|qt| !self.types[..self.n].contains(qt))
                    .expect("a broken slot always has a free AnswerOf target (AnswerOf count ≪ n)");
                self.types[qi] = qt;
            }
        }
    }

    /// PLACE the kinds FORCE didn't pin into the unpinned slots, hardest-first,
    /// with a fresh AnswerOf as the backup when a kind won't fit.
    fn place_unpinned_kinds(&mut self, unpinned: &[QuestionTypeKind], rng: &mut Rng) {
        let mut order: ArrayVec<QuestionTypeKind, MAX_N> = unpinned.iter().copied().collect();
        order.sort_unstable_by_key(|&k| place_rank(k));

        let mut free: ArrayVec<usize, MAX_N> =
            (0..self.n).filter(|&p| !self.is_pinned(p)).collect();
        rng.shuffle(&mut free);

        let mut assigned = self.pinned;
        for kind in order {
            if self.try_assign(kind, &mut free, &mut assigned, rng) {
                continue;
            }
            // Backup: a fresh AnswerOf — always satisfiable, never a duplicate.
            let qi = free.pop().expect("a free slot per unpinned kind");
            let qt = self.fresh_safe_type(qi);
            self.types[qi] = qt;
            self.placed.push(qt);
            assigned |= 1 << qi;
        }
    }

    /// A satisfiable, not-yet-placed type for `qi`: a fresh AnswerOf to some
    /// other question (always valid — `solution_satisfies` is unconditional for
    /// it). At most a handful of AnswerOf are ever placed, so a free target
    /// always exists for any `n`.
    fn fresh_safe_type(&self, qi: usize) -> QuestionType {
        (0..self.n)
            .filter(|&j| j != qi)
            .map(|j| QuestionType::AnswerOf {
                question_index: j as u8,
            })
            .find(|qt| !self.placed.contains(qt))
            .expect("a free AnswerOf target always exists (AnswerOf count ≪ n)")
    }

    /// First unpinned free slot where `kind` fits and a random parametrization
    /// satisfies the (now fixed) solution. Consumes the slot on success.
    fn try_assign(
        &mut self,
        kind: QuestionTypeKind,
        free: &mut ArrayVec<usize, MAX_N>,
        assigned: &mut u16,
        rng: &mut Rng,
    ) -> bool {
        for idx in 0..free.len() {
            let qi = free[idx];
            if !solution_fits_type(kind, qi, &self.sol, self.n, self.oc) {
                continue;
            }
            for _ in 0..10 {
                if let Some(qt) =
                    random_type_params(kind, qi, self.n, self.oc, &self.sol, *assigned, rng)
                    && !self.placed.contains(&qt)
                    && solution_satisfies_type(&qt, qi, &self.sol, self.n, self.oc)
                {
                    self.types[qi] = qt;
                    self.placed.push(qt);
                    *assigned |= 1 << qi;
                    free.swap_remove(idx);
                    return true;
                }
            }
        }
        false
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
                let c = compose(&RECIPES[level], p.question_count, p.option_count, &mut rng);
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
