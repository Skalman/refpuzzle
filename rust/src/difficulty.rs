use crate::rng::Rng;
use crate::types::QuestionTypeKind;
use crate::types::QuestionTypeKind::*;
use Dist::{Choices, Fixed};

/// A per-puzzle sampled value. `Fixed` never draws; `Choices` picks one listed
/// value uniformly — repeat a value to weight it (e.g. `[1, 1, 1, 2]` is a 25%
/// chance of 2).
pub enum Dist<T: Copy + 'static> {
    Fixed(T),
    Choices(&'static [T]),
}

impl<T: Copy> Dist<T> {
    pub fn sample(&self, rng: &mut Rng) -> T {
        match self {
            Dist::Fixed(v) => *v,
            Dist::Choices(vs) => rng.pick(vs),
        }
    }
}

pub struct DifficultyProfile {
    pub question_count: usize,
    pub option_count: usize,

    // ── Type budget: what may appear and how often ──
    pub allowed_types: &'static [QuestionTypeKind],
    /// Per-type max occurrences, indexed by `QuestionTypeKind`. Built at
    /// compile time from sparse overrides via `expand_caps` (default 3).
    pub caps: [u8; 32],
    /// Max CountAnswer questions sharing one answer letter.
    pub count_letter_cap: Dist<u8>,
    /// Cap on the vowel/consonant question group.
    pub vowel_consonant_cap: Dist<u8>,

    // ── Placement recipe ──
    pub phase_1_n_counting: Dist<usize>,
    /// AnswerOf self-reference chain length. A sampled value of 3 is the "long
    /// chain": phase 2 raises the AnswerOf cap to fit it and suppresses LetterDist.
    pub phase_2_n_answer_of: Dist<usize>,
    /// Attempts to place one PrevSame/NextSame at an edge slot (phase 3).
    pub phase_3_n_adjacent_same: Dist<usize>,
    pub phase_4_n_positional: Dist<usize>,
    /// Rare types to try to include at least once (phase 5), in order.
    pub phase_5_featured: &'static [QuestionTypeKind],
}

/// Expand sparse per-type cap overrides into a dense `[u8; 32]` (default 3),
/// evaluated at compile time so `new()` just copies the array.
const fn expand_caps(overrides: &[(QuestionTypeKind, u8)]) -> [u8; 32] {
    let mut caps = [3u8; 32];
    let mut i = 0;
    while i < overrides.len() {
        caps[overrides[i].0 as usize] = overrides[i].1;
        i += 1;
    }
    caps
}

// Shared recipe values (identical across levels for now).
const DEFAULT_CAPS: [u8; 32] = expand_caps(&[(LetterDist, 1), (AnswerOf, 2)]);
const COUNT_LETTER_CAP: Dist<u8> = Choices(&[1, 1, 1, 2]); // 25% chance of 2
const ADJACENT_SAME: Dist<usize> = Choices(&[0, 1]); // 50% attempt one

pub static PROFILES: [DifficultyProfile; 6] = [
    // Level 1: Intro
    DifficultyProfile {
        question_count: 3,
        option_count: 3,
        caps: DEFAULT_CAPS,
        count_letter_cap: COUNT_LETTER_CAP,
        vowel_consonant_cap: Fixed(1),
        phase_1_n_counting: Choices(&[0, 1]),
        phase_2_n_answer_of: Choices(&[0, 1]),
        phase_3_n_adjacent_same: ADJACENT_SAME,
        phase_4_n_positional: Fixed(0),
        phase_5_featured: &[],
        allowed_types: &[
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
    },
    // Level 2: Beginner
    DifficultyProfile {
        question_count: 4,
        option_count: 5,
        caps: DEFAULT_CAPS,
        count_letter_cap: COUNT_LETTER_CAP,
        vowel_consonant_cap: Fixed(1),
        phase_1_n_counting: Fixed(1),
        phase_2_n_answer_of: Choices(&[1, 2]),
        phase_3_n_adjacent_same: ADJACENT_SAME,
        phase_4_n_positional: Fixed(2),
        phase_5_featured: &[],
        allowed_types: &[CountAnswer, AnswerOf, AnswerIsSelf, FirstWith, LastWith],
    },
    DifficultyProfile {
        question_count: 5,
        option_count: 5,
        caps: DEFAULT_CAPS,
        count_letter_cap: COUNT_LETTER_CAP,
        vowel_consonant_cap: Fixed(1),
        phase_1_n_counting: Fixed(1),
        phase_2_n_answer_of: Choices(&[1, 2]),
        phase_3_n_adjacent_same: ADJACENT_SAME,
        phase_4_n_positional: Fixed(2),
        phase_5_featured: &[],
        allowed_types: &[
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
    },
    DifficultyProfile {
        question_count: 8,
        option_count: 5,
        caps: DEFAULT_CAPS,
        count_letter_cap: COUNT_LETTER_CAP,
        vowel_consonant_cap: Fixed(1),
        phase_1_n_counting: Fixed(1),
        phase_2_n_answer_of: Fixed(2),
        phase_3_n_adjacent_same: ADJACENT_SAME,
        phase_4_n_positional: Fixed(2),
        phase_5_featured: &[],
        allowed_types: &[
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
    },
    DifficultyProfile {
        question_count: 10,
        option_count: 5,
        caps: DEFAULT_CAPS,
        count_letter_cap: COUNT_LETTER_CAP,
        vowel_consonant_cap: Choices(&[1, 2]),
        phase_1_n_counting: Fixed(1),
        phase_2_n_answer_of: Choices(&[2, 2, 2, 3]),
        phase_3_n_adjacent_same: ADJACENT_SAME,
        phase_4_n_positional: Fixed(2),
        phase_5_featured: &[LetterDist, ConsecIdent],
        allowed_types: &[
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
    },
    DifficultyProfile {
        question_count: 12,
        option_count: 5,
        caps: DEFAULT_CAPS,
        count_letter_cap: COUNT_LETTER_CAP,
        vowel_consonant_cap: Choices(&[1, 2]),
        phase_1_n_counting: Fixed(1),
        phase_2_n_answer_of: Choices(&[2, 2, 2, 3]),
        phase_3_n_adjacent_same: ADJACENT_SAME,
        phase_4_n_positional: Fixed(2),
        phase_5_featured: &[LetterDist, TrueStmt, ConsecIdent],
        allowed_types: &[
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
    },
];
