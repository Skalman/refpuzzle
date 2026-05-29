use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const MAX_N: usize = 12;
pub const NONE_VAL: i16 = -1;
pub const NAN_VAL: i16 = i16::MIN;

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
    SameAsWhich,
}

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
    pub fn all_flat() -> &'static [QuestionTypeKind] {
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
        let pm = 0b11111u8 & !((1u8 << option_count) - 1);
        State {
            answers: [None; MAX_N],
            eliminated: [pm; MAX_N],
        }
    }
}

#[derive(Clone, Copy)]
pub struct OptionPos {
    pub qi: usize,
    pub oi: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Claim {
    #[serde(flatten)]
    pub question_type: QuestionType,
    #[serde(rename = "v")]
    pub value: i16,
}

#[derive(Clone)]
pub struct SmallList {
    pub data: [u8; MAX_N],
    pub len: u8,
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
}

pub struct FlatPuzzle {
    pub question_types: [QuestionType; MAX_N],
    pub option_nums: [[i16; 5]; MAX_N],
    pub option_answers: [[u8; 5]; MAX_N],
    pub option_claims: [[Option<Claim>; 5]; MAX_N],
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

    pub fn phantom_mask(&self) -> u8 {
        self.initial_state.eliminated[0]
    }
}
