use crate::types::*;

fn count_answer(answers: &[Option<Answer>], target: Answer, from: usize, to: usize) -> i16 {
    let mut c: i16 = 0;
    for i in from..to {
        if answers[i] == Some(target) {
            c += 1;
        }
    }
    c
}

fn count_vowels(answers: &[Option<Answer>], n: usize) -> i16 {
    let mut c: i16 = 0;
    for i in 0..n {
        if let Some(a) = answers[i]
            && a.is_vowel()
        {
            c += 1;
        }
    }
    c
}

fn count_consonants(answers: &[Option<Answer>], n: usize) -> i16 {
    let mut c: i16 = 0;
    for i in 0..n {
        if let Some(a) = answers[i]
            && !a.is_vowel()
        {
            c += 1;
        }
    }
    c
}

pub fn evaluate_claim(
    claim: &Claim,
    qi: usize,
    answers: &[Option<Answer>; MAX_N],
    n: usize,
) -> bool {
    let value = claim.value;
    match claim.question_type {
        QuestionType::CountAnswer { answer } => count_answer(answers, answer, 0, n) == value,
        QuestionType::CountConsonant => count_consonants(answers, n) == value,
        QuestionType::CountVowel => count_vowels(answers, n) == value,
        QuestionType::CountAnswerAfter {
            answer,
            after_index,
        } => count_answer(answers, answer, after_index as usize + 1, n) == value,
        QuestionType::CountAnswerBefore {
            answer,
            before_index,
        } => count_answer(answers, answer, 0, before_index as usize) == value,
        QuestionType::AnswerOf { question_index } => {
            value >= 0
                && value <= 4
                && answers[question_index as usize].map(|a| a.idx() as i16) == Some(value)
        }
        QuestionType::FirstWith { answer } => {
            for i in 0..n {
                if answers[i] == Some(answer) {
                    return i as i16 == value;
                }
            }
            false
        }
        QuestionType::LastWith { answer } => {
            let mut last: i16 = NONE_VAL;
            for i in 0..n {
                if answers[i] == Some(answer) {
                    last = i as i16;
                }
            }
            last == value
        }
        QuestionType::MostCommon => {
            if !(0..=4).contains(&value) {
                return false;
            }
            let mut counts = [0i16; 5];
            for i in 0..n {
                if let Some(a) = answers[i] {
                    counts[a.idx()] += 1;
                }
            }
            let max = *counts.iter().max().unwrap_or(&0);
            counts[value as usize] == max && counts.iter().filter(|&&c| c == max).count() == 1
        }
        _ => evaluate_claim_extended(claim, qi, answers, n),
    }
}

fn evaluate_claim_extended(
    claim: &Claim,
    qi: usize,
    answers: &[Option<Answer>; MAX_N],
    n: usize,
) -> bool {
    let value = claim.value;
    match claim.question_type {
        QuestionType::ClosestAfter {
            after_index,
            answer,
        } => {
            for i in (after_index as usize + 1)..n {
                if answers[i] == Some(answer) {
                    return i as i16 == value;
                }
            }
            value == NONE_VAL
        }
        QuestionType::ClosestBefore {
            before_index,
            answer,
        } => {
            for i in (0..before_index as usize).rev() {
                if answers[i] == Some(answer) {
                    return i as i16 == value;
                }
            }
            value == NONE_VAL
        }
        QuestionType::MostCommonCount => {
            let mut counts = [0i16; 5];
            for i in 0..n {
                if let Some(a) = answers[i] {
                    counts[a.idx()] += 1;
                }
            }
            *counts.iter().max().unwrap_or(&0) == value
        }
        QuestionType::LeastCommon => {
            if !(0..=4).contains(&value) {
                return false;
            }
            let mut counts = [0i16; 5];
            for i in 0..n {
                if let Some(a) = answers[i] {
                    counts[a.idx()] += 1;
                }
            }
            let min = *counts.iter().min().unwrap_or(&0);
            counts[value as usize] == min && counts.iter().filter(|&&c| c == min).count() == 1
        }
        QuestionType::Unique => {
            if !(0..=4).contains(&value) {
                return false;
            }
            let mut counts = [0i16; 5];
            for i in 0..n {
                if let Some(a) = answers[i] {
                    counts[a.idx()] += 1;
                }
            }
            counts[value as usize] == 1 && counts.iter().filter(|&&c| c == 1).count() == 1
        }
        QuestionType::EqualCount { answer } => {
            if !(0..=4).contains(&value) {
                return false;
            }
            let ref_count = count_answer(answers, answer, 0, n);
            let mut counts = [0i16; 5];
            for i in 0..n {
                if let Some(a) = answers[i] {
                    counts[a.idx()] += 1;
                }
            }
            counts[value as usize] == ref_count && value as usize != answer.idx()
        }
        QuestionType::ConsecIdent => {
            for i in 0..n.saturating_sub(1) {
                if answers[i].is_some() && answers[i] == answers[i + 1] {
                    return i as i16 == value;
                }
            }
            value == NONE_VAL
        }
        QuestionType::OnlyOdd { answer } | QuestionType::OnlyEven { answer } => {
            let parity = matches!(claim.question_type, QuestionType::OnlyEven { .. }) as usize;
            let mut found: i16 = NONE_VAL;
            let mut count = 0;
            for i in 0..n {
                if i % 2 == parity && answers[i] == Some(answer) {
                    found = i as i16;
                    count += 1;
                }
            }
            count == 1 && found == value
        }
        QuestionType::PrevSame => {
            let self_ans = match answers[qi] {
                Some(a) => a,
                None => return false,
            };
            for i in (0..qi).rev() {
                if answers[i] == Some(self_ans) {
                    return i as i16 == value;
                }
            }
            value == NONE_VAL
        }
        QuestionType::NextSame => {
            let self_ans = match answers[qi] {
                Some(a) => a,
                None => return false,
            };
            for i in (qi + 1)..n {
                if answers[i] == Some(self_ans) {
                    return i as i16 == value;
                }
            }
            value == NONE_VAL
        }
        QuestionType::OnlySame => {
            let self_ans = match answers[qi] {
                Some(a) => a,
                None => return false,
            };
            let mut found: i16 = NONE_VAL;
            let mut count = 0;
            for i in 0..n {
                if i != qi && answers[i] == Some(self_ans) {
                    found = i as i16;
                    count += 1;
                }
            }
            if count == 0 {
                value == NONE_VAL
            } else {
                count == 1 && found == value
            }
        }
        QuestionType::SameAs => {
            let self_ans = match answers[qi] {
                Some(a) => a,
                None => return false,
            };
            value >= 0
                && (value as usize) < n
                && value as usize != qi
                && answers[value as usize] == Some(self_ans)
        }
        QuestionType::SameAsWhich { question_index } => {
            let ref_ans = match answers[question_index as usize] {
                Some(a) => a,
                None => return false,
            };
            value >= 0
                && (value as usize) < n
                && value as usize != qi
                && value as usize != question_index as usize
                && answers[value as usize] == Some(ref_ans)
        }
        QuestionType::LetterDist { question_index } => {
            let self_ans = match answers[qi] {
                Some(a) => a,
                None => return false,
            };
            let other = match answers[question_index as usize] {
                Some(a) => a,
                None => return false,
            };
            (self_ans.idx() as i16 - other.idx() as i16).abs() == value
        }
        // AnswerIsSelf and TrueStmt are not valid as claims; return false.
        QuestionType::AnswerIsSelf | QuestionType::TrueStmt => false,
        // Already handled in main evaluate_claim:
        QuestionType::CountAnswer { .. }
        | QuestionType::CountAnswerBefore { .. }
        | QuestionType::CountAnswerAfter { .. }
        | QuestionType::CountVowel
        | QuestionType::CountConsonant
        | QuestionType::AnswerOf { .. }
        | QuestionType::FirstWith { .. }
        | QuestionType::LastWith { .. }
        | QuestionType::MostCommon => unreachable!(),
    }
}
