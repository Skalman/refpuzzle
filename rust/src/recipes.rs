//! Per-level generation recipes: the static table that decides a level's board
//! size, question mix, and difficulty knobs, plus the board-size lookups.
//!
//! A leaf module — it names no engine or generator code, so anything that only
//! needs a level's parameters (the wasm solve, `check`, `type-stats`, option
//! filling's NONE calibration) can read it without pulling in `construct`.
//! `solve_deduce` hangs the [`EngineConfig`](crate::solve_deduce::EngineConfig)
//! constructors off [`LevelRecipe`] from its own side, for the same reason.

use crate::types::QuestionTypeKind::*;
use crate::types::{QUESTION_GROUP_COUNT, QUESTION_KIND_COUNT, QuestionGroup, QuestionTypeKind};

/// Per-level recipe: the board size plus everything that decides the question
/// set (`required` + `allowed` + `caps`) and how hard the level may get.
pub struct LevelRecipe {
    /// Own index in [`RECIPES`], so a recipe in hand knows its level without a search.
    /// 0-based: level 1 is `level_index` 0.
    pub level_index: usize,
    /// Questions on the board.
    pub question_count: usize,
    /// Options per question, 3..=5.
    pub option_count: usize,
    /// Types that must appear, with how many of each.
    pub required: &'static [(QuestionTypeKind, usize)],
    /// The pool the remaining slots are filled from.
    pub allowed: &'static [QuestionTypeKind],
    /// Per-type max occurrences (default 3; unit variants 1 — see `DEFAULT_CAPS`).
    /// Built from `question_count` via `caps_max_answer_of`, which must agree with
    /// the field above.
    pub caps: [u8; QUESTION_KIND_COUNT],
    /// Per-group damping applied during kind selection (see `DEFAULT_DAMPING`);
    /// indexed by `QuestionGroup as usize`.
    pub damping: [f64; QUESTION_GROUP_COUNT],
    /// AnswerOf-count distribution as `(count, weight)` pairs: each puzzle samples a
    /// count (probability ∝ weight) and forces exactly that many AnswerOfs — so a
    /// few puzzles are reference-heavy while most have few/none. Counts must be
    /// ≤ n-1. Weights are percentages (sum 100) by convention. Empty = no forcing
    /// (AnswerOf drawn normally, bounded by `caps`).
    pub answer_of_counts: &'static [(u8, u16)],
    /// How far the engine's lookahead may deduce when accepting a puzzle: within
    /// each hypothesis it deduces until the chain reaches this many results, then
    /// stops probing. 0 = pure deduction only (no lookahead); larger admits harder
    /// puzzles. Ramps from intro (shallow) to late (deep).
    pub lookahead_deduce_until: usize,
}

/// Best-effort recipe for a puzzle with `n` questions, for callers without a recipe
/// in hand (the wasm `solve`, the playground). Maps question count to the nearest
/// level and always returns something. Shipped puzzles hit their level exactly
/// (counts 3/4/5/8/10/12); an odd `n` falls to a neighbor, with `fallback` as the net.
/// Only the returned recipe's difficulty knobs apply to such a puzzle — every field on it,
/// `question_count`/`option_count`/`level_index` included, describes the level it landed on
/// rather than the puzzle at hand.
pub fn guess_recipe(n: usize) -> &'static LevelRecipe {
    let level_index = match n {
        0..=3 => 0,
        4 => 1,
        5..=7 => 2,
        8..=9 => 3,
        10..=11 => 4,
        _ => 5,
    };
    &RECIPES[level_index]
}

/// The recipe whose board is exactly `n` questions, or `None` if no level uses that count.
/// The strict counterpart to [`guess_recipe`], for callers that have to tell a real level
/// from an off-recipe board rather than round to the nearest one.
///
/// Well-defined only because the recipes' question counts are distinct (3/4/5/8/10/12);
/// two levels sharing a count would make the answer arbitrary.
pub(crate) fn exact_recipe(n: usize) -> Option<&'static LevelRecipe> {
    let recipe = match n {
        3 => 0,
        4 => 1,
        5 => 2,
        8 => 3,
        10 => 4,
        12 => 5,
        _ => return None,
    };
    Some(&RECIPES[recipe])
}

const fn caps_with(overrides: &[(QuestionTypeKind, u8)]) -> [u8; QUESTION_KIND_COUNT] {
    let mut c = [3u8; QUESTION_KIND_COUNT];
    let mut i = 0;
    while i < overrides.len() {
        c[overrides[i].0 as usize] = overrides[i].1;
        i += 1;
    }
    c
}

const DEFAULT_CAPS: [u8; QUESTION_KIND_COUNT] = caps_with(&[
    (QuestionTypeKind::LetterDist, 1),
    // AnswerOf: no entry here — each recipe caps it at n-1 (its structural max,
    // since wire needs one non-AnswerOf question to root the chains) via
    // `caps_max_answer_of`.
    // Parameter-free variants carry no position, so two instances would be the
    // same `QuestionType` value — a literal duplicate the pairwise-distinctness
    // check rejects. Cap them at 1.
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

/// `DEFAULT_CAPS` but with AnswerOf capped at `question_count - 1` — its
/// structural maximum, since `wire_answer_of_targets` needs at least one
/// non-AnswerOf question to root the reference chains. Each recipe sets this so a
/// hard-to-fill skeleton can lean on the always-fits AnswerOf all the way up.
const fn caps_max_answer_of(question_count: usize) -> [u8; QUESTION_KIND_COUNT] {
    let mut c = DEFAULT_CAPS;
    c[QuestionTypeKind::AnswerOf as usize] = (question_count - 1) as u8;
    c
}

const fn damping_with(overrides: &[(QuestionGroup, f64)]) -> [f64; QUESTION_GROUP_COUNT] {
    let mut d = [1.0f64; QUESTION_GROUP_COUNT];
    let mut i = 0;
    while i < overrides.len() {
        d[overrides[i].0 as usize] = overrides[i].1;
        i += 1;
    }
    d
}

/// Per-group damping for `select_kinds`: once a kind from a group has been
/// picked, each later same-group candidate is kept only with this probability
/// (else redrawn), so near-synonymous questions cluster less. Flat, not
/// compounding — a third same-group pick is no less likely than the second —
/// so "extreme" puzzles stay possible, just rarer. Groups omitted here (and the
/// ungrouped kinds) default to 1.0 = no damping.
const DEFAULT_DAMPING: [f64; QUESTION_GROUP_COUNT] = damping_with(&[
    (QuestionGroup::AnswerCount, 0.5),
    (QuestionGroup::LetterClass, 0.3),
    (QuestionGroup::Histogram, 0.4),
    (QuestionGroup::Closest, 0.6),
    (QuestionGroup::FirstLast, 0.4),
    (QuestionGroup::Sameness, 0.9),
    (QuestionGroup::Parity, 0.5),
    (QuestionGroup::AnswerOf, 1.0),
]);

/// Per-level recipes, tuned via type-stats. Indexed by level-1.
pub static RECIPES: [LevelRecipe; 6] = [
    LevelRecipe {
        level_index: 0,
        question_count: 3,
        option_count: 3,
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
        caps: caps_max_answer_of(3),
        damping: DEFAULT_DAMPING,
        answer_of_counts: &[(0, 50), (1, 40), (2, 10)],
        // L1 is the tutorial level: accept only pure-deduction puzzles (no
        // lookahead), so the scripted walk never needs "what if" reasoning.
        lookahead_deduce_until: 0,
    },
    LevelRecipe {
        level_index: 1,
        question_count: 4,
        option_count: 4,
        required: &[],
        allowed: &[CountAnswer, AnswerOf, AnswerIsSelf, FirstWith, LastWith],
        caps: caps_max_answer_of(4),
        damping: DEFAULT_DAMPING,
        answer_of_counts: &[(0, 42), (1, 35), (2, 20), (3, 3)],
        lookahead_deduce_until: 1,
    },
    LevelRecipe {
        level_index: 2,
        question_count: 5,
        option_count: 5,
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
        caps: caps_max_answer_of(5),
        damping: DEFAULT_DAMPING,
        answer_of_counts: &[(0, 31), (1, 33), (2, 25), (3, 10), (4, 1)],
        lookahead_deduce_until: 6,
    },
    LevelRecipe {
        level_index: 3,
        question_count: 8,
        option_count: 5,
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
        caps: caps_max_answer_of(8),
        damping: DEFAULT_DAMPING,
        answer_of_counts: &[(0, 22), (1, 28), (2, 31), (3, 15), (4, 3), (5, 1)],
        lookahead_deduce_until: 6,
    },
    LevelRecipe {
        level_index: 4,
        question_count: 10,
        option_count: 5,
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
        caps: caps_max_answer_of(10),
        damping: DEFAULT_DAMPING,
        answer_of_counts: &[(0, 16), (1, 25), (2, 31), (3, 19), (4, 7), (5, 2)],
        lookahead_deduce_until: 6,
    },
    LevelRecipe {
        level_index: 5,
        question_count: 12,
        option_count: 5,
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
        caps: caps_max_answer_of(12),
        damping: DEFAULT_DAMPING,
        answer_of_counts: &[(0, 13), (1, 24), (2, 24), (3, 26), (4, 9), (5, 3), (6, 1)],
        lookahead_deduce_until: 6,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// `exact_recipe` resolves a level by question count, so the counts have to be distinct
    /// — two levels sharing one would make it return whichever came first. Also pins
    /// `level_index` to the recipe's actual position, which nothing else enforces.
    #[test]
    fn recipe_question_counts_are_distinct() {
        let counts: Vec<usize> = RECIPES.iter().map(|r| r.question_count).collect();
        let mut seen = counts.clone();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(
            seen.len(),
            counts.len(),
            "levels {counts:?} share a question count, so exact_recipe is ambiguous"
        );
        for (level, &count) in counts.iter().enumerate() {
            let recipe = exact_recipe(count).expect("own count resolves");
            assert!(std::ptr::eq(recipe, &RECIPES[level]), "L{}", level + 1);
            assert_eq!(recipe.level_index, level, "L{} level_index", level + 1);
        }
        assert!(exact_recipe(0).is_none());
        assert!(exact_recipe(7).is_none(), "7 is between L3 and L4");
    }

    /// A recipe's `caps` is built from its own `question_count`; a mismatch would
    /// silently cap (or over-cap) AnswerOf.
    #[test]
    fn recipes_are_self_consistent() {
        for (level, recipe) in RECIPES.iter().enumerate() {
            assert!(
                (3..=5).contains(&recipe.option_count),
                "L{}: option_count out of range",
                level + 1,
            );
            assert_eq!(
                recipe.caps,
                caps_max_answer_of(recipe.question_count),
                "L{}: caps disagree with question_count",
                level + 1,
            );
            for &(count, _) in recipe.answer_of_counts {
                assert!(
                    usize::from(count) < recipe.question_count,
                    "L{}: AnswerOf count {count} exceeds n-1",
                    level + 1,
                );
            }
        }
    }
}
