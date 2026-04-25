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
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Answer::A),
            1 => Some(Answer::B),
            2 => Some(Answer::C),
            3 => Some(Answer::D),
            4 => Some(Answer::E),
            _ => None,
        }
    }
    pub fn as_char(self) -> char {
        (b'A' + self as u8) as char
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RuleKind {
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
    ConsecIdent,
    AnswerOf,
    LeastCommon,
    MostCommon,
    Unique,
    EqualCount,
    AnswerIsSelf,
    LetterDist,
    TrueStmt,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rule {
    CountAnswer { answer: Answer },
    CountAnswerBefore { answer: Answer, before_index: u8 },
    CountAnswerAfter { answer: Answer, after_index: u8 },
    CountVowel,
    CountConsonant,
    MostCommonCount,
    ClosestAfter { after_index: u8, answer: Answer },
    ClosestBefore { before_index: u8, answer: Answer },
    FirstWith { answer: Answer },
    LastWith { answer: Answer },
    PrevSame,
    NextSame,
    OnlySame,
    SameAs,
    OnlyOdd { answer: Answer },
    ConsecIdent,
    AnswerOf { question_index: u8 },
    LeastCommon,
    MostCommon,
    Unique,
    EqualCount { answer: Answer },
    AnswerIsSelf,
    LetterDist { other_question_index: u8 },
    TrueStmt,
}

impl Rule {
    pub fn is_constrained(&self) -> bool {
        matches!(
            self,
            Rule::Unique | Rule::EqualCount { .. } | Rule::AnswerIsSelf
        )
    }

    pub fn is_global(&self) -> bool {
        matches!(
            self,
            Rule::CountAnswer { .. }
                | Rule::CountVowel
                | Rule::CountConsonant
                | Rule::LeastCommon
                | Rule::MostCommon
                | Rule::MostCommonCount
                | Rule::Unique
                | Rule::EqualCount { .. }
                | Rule::TrueStmt
                | Rule::OnlySame
                | Rule::ConsecIdent
                | Rule::OnlyOdd { .. }
                | Rule::FirstWith { .. }
                | Rule::LastWith { .. }
                | Rule::SameAs
        )
    }

    pub fn is_solver_global(&self) -> bool {
        matches!(
            self,
            Rule::CountAnswer { .. }
                | Rule::CountVowel
                | Rule::CountConsonant
                | Rule::LeastCommon
                | Rule::MostCommon
                | Rule::MostCommonCount
                | Rule::Unique
                | Rule::EqualCount { .. }
                | Rule::TrueStmt
                | Rule::OnlySame
                | Rule::ConsecIdent
                | Rule::OnlyOdd { .. }
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Claim {
    None,
    CountAnswerEquals {
        answer: Answer,
        value: u8,
    },
    CountConsonantEquals {
        value: u8,
    },
    CountVowelEquals {
        value: u8,
    },
    CountAnswerAfterEquals {
        answer: Answer,
        after_index: u8,
        value: u8,
    },
    CountAnswerBeforeEquals {
        answer: Answer,
        before_index: u8,
        value: u8,
    },
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
    pub rules: [Rule; MAX_N],
    pub option_nums: [[i16; 5]; MAX_N],
    pub option_answers: [[u8; 5]; MAX_N],
    pub option_claims: [[Claim; 5]; MAX_N],
    pub affected_by: [SmallList; MAX_N],
    pub global_indices: SmallList,
    pub n: usize,
}

impl FlatPuzzle {
    pub fn build_deps(rules: &[Rule], n: usize) -> ([SmallList; MAX_N], SmallList) {
        let mut affected_by: [SmallList; MAX_N] = std::array::from_fn(|_| SmallList::new());
        let mut global_indices = SmallList::new();

        for i in 0..n {
            let r = &rules[i];
            if r.is_global() {
                global_indices.push(i as u8);
            } else {
                match *r {
                    Rule::AnswerOf { question_index } => {
                        affected_by[question_index as usize].push(i as u8);
                    }
                    Rule::LetterDist {
                        other_question_index,
                    } => {
                        affected_by[other_question_index as usize].push(i as u8);
                    }
                    Rule::ClosestAfter { after_index, .. }
                    | Rule::CountAnswerAfter { after_index, .. } => {
                        for j in (after_index as usize + 1)..n {
                            affected_by[j].push(i as u8);
                        }
                    }
                    Rule::ClosestBefore { before_index, .. }
                    | Rule::CountAnswerBefore { before_index, .. } => {
                        for j in 0..before_index as usize {
                            affected_by[j].push(i as u8);
                        }
                    }
                    Rule::PrevSame => {
                        for j in 0..i {
                            affected_by[j].push(i as u8);
                        }
                    }
                    Rule::NextSame => {
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
