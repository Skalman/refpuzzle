use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const MAX_N: usize = 16;
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    Unique,
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
    Unique,
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

impl QuestionType {
    pub fn is_constrained(&self) -> bool {
        matches!(self, QuestionType::Unique | QuestionType::AnswerIsSelf)
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
                | QuestionType::Unique
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
                | QuestionType::Unique
                | QuestionType::EqualCount { .. }
                | QuestionType::TrueStmt
                | QuestionType::OnlySame
                | QuestionType::ConsecIdent
                | QuestionType::OnlyOdd { .. }
                | QuestionType::OnlyEven { .. }
        )
    }
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
}
