use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const MAX_N: usize = 12;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(transparent)]
pub struct OptionValue(u8);

impl OptionValue {
    pub const NONE: Self = Self(0xFE);
    pub const UNUSED: Self = Self(0xFF);

    pub fn num(v: u8) -> Self {
        debug_assert!(v < 0xFE);
        Self(v)
    }

    pub fn is_none(self) -> bool {
        self == Self::NONE
    }
    pub fn is_unused(self) -> bool {
        self == Self::UNUSED
    }
    pub fn is_num(self) -> bool {
        self.0 < 0xFE
    }

    /// Numeric payload. Asserts `is_num()` in debug — call sites must
    /// pre-check (or handle NONE / UNUSED) before using.
    pub fn value(self) -> u8 {
        debug_assert!(self.is_num());
        self.0
    }
}

// Wire format for OptionValue: JSON `null` for NONE, integer for Num(v).
// UNUSED never serializes — it's a storage-only artifact of fixed arrays.
impl Serialize for OptionValue {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if self.is_none() {
            s.serialize_none()
        } else if self.is_num() {
            s.serialize_u8(self.value())
        } else {
            // UNUSED slipped through — serialize as null defensively rather
            // than panicking from the debug_assert in value().
            s.serialize_none()
        }
    }
}

impl<'de> Deserialize<'de> for OptionValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Option::<u8>::deserialize(d)?;
        match v {
            None => Ok(Self::NONE),
            Some(n) if n < 0xFE => Ok(Self::num(n)),
            Some(n) => Err(serde::de::Error::custom(format!(
                "invalid option value: {n}"
            ))),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[repr(u8)]
pub enum Answer {
    A = 0,
    B = 1,
    C = 2,
    D = 3,
    E = 4,
}

pub const LETTERS: [Answer; 5] = [Answer::A, Answer::B, Answer::C, Answer::D, Answer::E];

/// Bitmask of all five option slots (`0b11111`). A puzzle's real options are the
/// low `option_count` bits; the high `5 - option_count` "phantom" bits are
/// pre-eliminated in `State::initial`. The engine masks option bitsets with this
/// so phantom slots never leak into counts, eliminations, or forces.
pub const ALL_OPTIONS_MASK: u8 = 0b11111;

impl Answer {
    pub fn idx(self) -> usize {
        self as usize
    }
    pub fn is_vowel(self) -> bool {
        matches!(self, Answer::A | Answer::E)
    }
    pub fn as_char(self) -> char {
        (b'A' + self as u8) as char
    }
}

impl std::fmt::Display for Answer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_char())
    }
}

impl From<u8> for Answer {
    /// Infallible conversion for internal callers that pass a known-valid index
    /// (`0..5`): option/answer loop indices, `OptionValue::value()` of a real
    /// letter option. Panics with the offending value if `v >= 5`. Wire/parse
    /// paths never use this — they go through the fallible `Deserialize` impl.
    fn from(v: u8) -> Answer {
        match v {
            0 => Answer::A,
            1 => Answer::B,
            2 => Answer::C,
            3 => Answer::D,
            4 => Answer::E,
            _ => panic!("invalid answer index: {v}"),
        }
    }
}

impl Serialize for Answer {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for Answer {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = u8::deserialize(d)?;
        match v {
            0 => Ok(Answer::A),
            1 => Ok(Answer::B),
            2 => Ok(Answer::C),
            3 => Ok(Answer::D),
            4 => Ok(Answer::E),
            _ => Err(serde::de::Error::custom(format!("invalid answer: {v}"))),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
pub enum QuestionTypeKind {
    CountAnswer,
    CountAnswerBefore,
    CountAnswerAfter,
    CountVowel,
    CountConsonant,
    MostCommonCount,
    ClosestAfter,
    ClosestBefore,
    FirstWith,
    LastWith,
    PrevSame,
    NextSame,
    OnlySame,
    SameAs,
    OnlyOdd,
    OnlyEven,
    ConsecIdent,
    AnswerOf,
    LeastCommon,
    MostCommon,
    NoOtherHasAnswer,
    EqualCount,
    AnswerIsSelf,
    LetterDist,
    TrueStmt,
    /// Must stay last: `QUESTION_KIND_COUNT` derives from `SameAsWhich as usize + 1`.
    SameAsWhich,
}

/// Number of [`QuestionTypeKind`] variants — the length of a per-kind array
/// (recipe caps, selection counts). Derives from the last variant.
pub const QUESTION_KIND_COUNT: usize = QuestionTypeKind::SameAsWhich as usize + 1;

/// Coarse "families" of question kinds that read as similar to a solver. Used
/// only by generation to dampen picking a *second* kind from the same family
/// (see `DEFAULT_DAMPING`), so a puzzle is less likely to stack near-synonyms
/// (three `Count*`s, both parities). Kinds that stand alone — those already
/// capped at one occurrence, plus `LetterDist` — have no group and are never
/// damped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum QuestionGroup {
    AnswerCount,
    LetterClass,
    Histogram,
    Closest,
    FirstLast,
    Sameness,
    Parity,
    /// Must stay last: `QUESTION_GROUP_COUNT` derives from `AnswerOf as usize + 1`.
    AnswerOf,
}

/// Number of [`QuestionGroup`] variants — the length of a per-group array.
pub const QUESTION_GROUP_COUNT: usize = QuestionGroup::AnswerOf as usize + 1;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum QuestionType {
    CountAnswer {
        #[serde(rename = "a")]
        answer: Answer,
    },
    CountAnswerBefore {
        #[serde(rename = "a")]
        answer: Answer,
        #[serde(rename = "q")]
        before_index: u8,
    },
    CountAnswerAfter {
        #[serde(rename = "a")]
        answer: Answer,
        #[serde(rename = "q")]
        after_index: u8,
    },
    CountVowel,
    CountConsonant,
    MostCommonCount,
    ClosestAfter {
        #[serde(rename = "q")]
        after_index: u8,
        #[serde(rename = "a")]
        answer: Answer,
    },
    ClosestBefore {
        #[serde(rename = "q")]
        before_index: u8,
        #[serde(rename = "a")]
        answer: Answer,
    },
    FirstWith {
        #[serde(rename = "a")]
        answer: Answer,
    },
    LastWith {
        #[serde(rename = "a")]
        answer: Answer,
    },
    PrevSame,
    NextSame,
    OnlySame,
    SameAs,
    OnlyOdd {
        #[serde(rename = "a")]
        answer: Answer,
    },
    OnlyEven {
        #[serde(rename = "a")]
        answer: Answer,
    },
    ConsecIdent,
    AnswerOf {
        #[serde(rename = "q")]
        question_index: u8,
    },
    LeastCommon,
    MostCommon,
    NoOtherHasAnswer,
    EqualCount {
        #[serde(rename = "a")]
        answer: Answer,
    },
    AnswerIsSelf,
    LetterDist {
        #[serde(rename = "q")]
        question_index: u8,
    },
    TrueStmt,
    SameAsWhich {
        #[serde(rename = "q")]
        question_index: u8,
    },
}

impl QuestionTypeKind {
    pub fn all() -> &'static [QuestionTypeKind] {
        use QuestionTypeKind::*;
        &[
            CountAnswer,
            CountAnswerBefore,
            CountAnswerAfter,
            CountVowel,
            CountConsonant,
            MostCommonCount,
            ClosestAfter,
            ClosestBefore,
            FirstWith,
            LastWith,
            PrevSame,
            NextSame,
            OnlySame,
            SameAs,
            OnlyOdd,
            OnlyEven,
            ConsecIdent,
            AnswerOf,
            LeastCommon,
            MostCommon,
            NoOtherHasAnswer,
            EqualCount,
            AnswerIsSelf,
            LetterDist,
            TrueStmt,
            SameAsWhich,
        ]
    }

    /// The similarity family this kind belongs to, or `None` if it stands
    /// alone (never damped during generation). See [`QuestionGroup`].
    pub fn group(self) -> Option<QuestionGroup> {
        use QuestionGroup as G;
        use QuestionTypeKind::*;
        Some(match self {
            CountAnswer | CountAnswerBefore | CountAnswerAfter => G::AnswerCount,
            CountVowel | CountConsonant => G::LetterClass,
            MostCommonCount | LeastCommon | MostCommon | EqualCount => G::Histogram,
            ClosestAfter | ClosestBefore => G::Closest,
            FirstWith | LastWith => G::FirstLast,
            PrevSame | NextSame | OnlySame | SameAs => G::Sameness,
            OnlyOdd | OnlyEven => G::Parity,
            AnswerOf => G::AnswerOf,
            ConsecIdent | NoOtherHasAnswer | AnswerIsSelf | LetterDist | TrueStmt | SameAsWhich => {
                return None;
            }
        })
    }
}

impl QuestionType {
    pub fn kind(&self) -> QuestionTypeKind {
        match self {
            QuestionType::CountAnswer { .. } => QuestionTypeKind::CountAnswer,
            QuestionType::CountAnswerBefore { .. } => QuestionTypeKind::CountAnswerBefore,
            QuestionType::CountAnswerAfter { .. } => QuestionTypeKind::CountAnswerAfter,
            QuestionType::CountVowel => QuestionTypeKind::CountVowel,
            QuestionType::CountConsonant => QuestionTypeKind::CountConsonant,
            QuestionType::MostCommonCount => QuestionTypeKind::MostCommonCount,
            QuestionType::ClosestAfter { .. } => QuestionTypeKind::ClosestAfter,
            QuestionType::ClosestBefore { .. } => QuestionTypeKind::ClosestBefore,
            QuestionType::FirstWith { .. } => QuestionTypeKind::FirstWith,
            QuestionType::LastWith { .. } => QuestionTypeKind::LastWith,
            QuestionType::PrevSame => QuestionTypeKind::PrevSame,
            QuestionType::NextSame => QuestionTypeKind::NextSame,
            QuestionType::OnlySame => QuestionTypeKind::OnlySame,
            QuestionType::SameAs => QuestionTypeKind::SameAs,
            QuestionType::OnlyOdd { .. } => QuestionTypeKind::OnlyOdd,
            QuestionType::OnlyEven { .. } => QuestionTypeKind::OnlyEven,
            QuestionType::ConsecIdent => QuestionTypeKind::ConsecIdent,
            QuestionType::AnswerOf { .. } => QuestionTypeKind::AnswerOf,
            QuestionType::LeastCommon => QuestionTypeKind::LeastCommon,
            QuestionType::MostCommon => QuestionTypeKind::MostCommon,
            QuestionType::NoOtherHasAnswer => QuestionTypeKind::NoOtherHasAnswer,
            QuestionType::EqualCount { .. } => QuestionTypeKind::EqualCount,
            QuestionType::AnswerIsSelf => QuestionTypeKind::AnswerIsSelf,
            QuestionType::LetterDist { .. } => QuestionTypeKind::LetterDist,
            QuestionType::TrueStmt => QuestionTypeKind::TrueStmt,
            QuestionType::SameAsWhich { .. } => QuestionTypeKind::SameAsWhich,
        }
    }

    pub fn has_identity_options(&self) -> bool {
        matches!(
            self,
            QuestionType::NoOtherHasAnswer | QuestionType::AnswerIsSelf
        )
    }

    pub fn is_global(&self) -> bool {
        matches!(
            self,
            QuestionType::CountAnswer { .. }
                | QuestionType::CountVowel
                | QuestionType::CountConsonant
                | QuestionType::LeastCommon
                | QuestionType::MostCommon
                | QuestionType::MostCommonCount
                | QuestionType::NoOtherHasAnswer
                | QuestionType::EqualCount { .. }
                | QuestionType::TrueStmt
                | QuestionType::OnlySame
                | QuestionType::ConsecIdent
                | QuestionType::OnlyOdd { .. }
                | QuestionType::OnlyEven { .. }
                | QuestionType::FirstWith { .. }
                | QuestionType::LastWith { .. }
                | QuestionType::SameAs
                | QuestionType::SameAsWhich { .. }
        )
    }

    pub fn is_solver_global(&self) -> bool {
        matches!(
            self,
            QuestionType::CountAnswer { .. }
                | QuestionType::CountVowel
                | QuestionType::CountConsonant
                | QuestionType::LeastCommon
                | QuestionType::MostCommon
                | QuestionType::MostCommonCount
                | QuestionType::NoOtherHasAnswer
                | QuestionType::EqualCount { .. }
                | QuestionType::TrueStmt
                | QuestionType::OnlySame
                | QuestionType::ConsecIdent
                | QuestionType::OnlyOdd { .. }
                | QuestionType::OnlyEven { .. }
        )
    }
}

#[derive(Clone, Copy)]
pub struct State {
    pub answers: [Option<Answer>; MAX_N],
    pub eliminated: [u8; MAX_N],
}

impl State {
    pub fn initial(option_count: usize) -> Self {
        let initial_eliminated_mask = ALL_OPTIONS_MASK & !((1u8 << option_count) - 1);
        State {
            answers: [None; MAX_N],
            eliminated: [initial_eliminated_mask; MAX_N],
        }
    }
}

#[derive(Clone, Copy)]
pub struct OptionPos {
    pub qi: usize,
    pub oi: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Claim {
    pub question_type: QuestionType,
    pub value: OptionValue,
}

#[derive(Clone)]
pub struct SmallList {
    data: [u8; MAX_N],
    len: u8,
}

impl SmallList {
    pub fn new() -> Self {
        SmallList {
            data: [0; MAX_N],
            len: 0,
        }
    }

    pub fn push(&mut self, val: u8) {
        self.data[self.len as usize] = val;
        self.len += 1;
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Iterate stored values as usize (the typical use is as a question
    /// index). Storage is still u8 internally; this widens at the boundary.
    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.data[..self.len()].iter().map(|&i| i as usize)
    }
}

impl std::ops::Index<usize> for SmallList {
    type Output = u8;
    fn index(&self, i: usize) -> &u8 {
        debug_assert!(i < self.len());
        &self.data[i]
    }
}

pub struct FlatPuzzle {
    pub question_types: [QuestionType; MAX_N],
    /// Per-option values. For TrueStmt rows this stores the per-claim values;
    /// the matching claim question types live in `true_stmt_question_types`.
    pub options: [[OptionValue; 5]; MAX_N],
    /// Question types for the TrueStmt's claims, indexed by option. `Some` iff
    /// the puzzle has exactly one TrueStmt question. Slots beyond
    /// `option_count` are unused.
    pub true_stmt_question_types: Option<[QuestionType; 5]>,
    pub affected_by: [SmallList; MAX_N],
    pub global_indices: SmallList,
    pub n: usize,
    pub option_count: usize,
    pub initial_state: State,
}

impl FlatPuzzle {
    pub fn build_deps(
        question_types: &[QuestionType],
        n: usize,
    ) -> ([SmallList; MAX_N], SmallList) {
        let mut affected_by: [SmallList; MAX_N] = std::array::from_fn(|_| SmallList::new());
        let mut global_indices = SmallList::new();

        for i in 0..n {
            let t = &question_types[i];
            if t.is_global() {
                global_indices.push(i as u8);
            } else {
                match *t {
                    QuestionType::AnswerOf { question_index } => {
                        affected_by[question_index as usize].push(i as u8);
                    }
                    QuestionType::LetterDist { question_index } => {
                        affected_by[question_index as usize].push(i as u8);
                    }
                    QuestionType::ClosestAfter { after_index, .. }
                    | QuestionType::CountAnswerAfter { after_index, .. } => {
                        for j in (after_index as usize + 1)..n {
                            affected_by[j].push(i as u8);
                        }
                    }
                    QuestionType::ClosestBefore { before_index, .. }
                    | QuestionType::CountAnswerBefore { before_index, .. } => {
                        for j in 0..before_index as usize {
                            affected_by[j].push(i as u8);
                        }
                    }
                    QuestionType::PrevSame => {
                        for j in 0..i {
                            affected_by[j].push(i as u8);
                        }
                    }
                    QuestionType::NextSame => {
                        for j in (i + 1)..n {
                            affected_by[j].push(i as u8);
                        }
                    }
                    _ => {} // AnswerIsSelf or similar
                }
            }
            affected_by[i].push(i as u8);
        }

        (affected_by, global_indices)
    }

    /// The value every question's `eliminated` starts at: the "phantom" option
    /// slots ≥ `option_count`, pre-eliminated because they aren't real options.
    /// (No real eliminations exist yet in the initial state, so this is the whole
    /// mask.)
    pub fn initial_eliminated_mask(&self) -> u8 {
        self.initial_state.eliminated[0]
    }

    /// Reconstruct the claim at TrueStmt option `(qi, oi)`. Returns `None` if
    /// `qi` is not the TrueStmt question, if the puzzle has no TrueStmt, or if
    /// the option slot is unused (e.g. `oi >= option_count`).
    #[inline]
    pub fn claim_at(&self, qi: usize, oi: usize) -> Option<Claim> {
        if !matches!(self.question_types[qi], QuestionType::TrueStmt) {
            return None;
        }
        let types = self.true_stmt_question_types.as_ref()?;
        let value = self.options[qi][oi];
        if value.is_unused() {
            return None;
        }
        Some(Claim {
            question_type: types[oi],
            value,
        })
    }
}
