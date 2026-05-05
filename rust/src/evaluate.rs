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

pub fn evaluate_claim(claim: &Claim, answers: &[Option<Answer>; MAX_N], n: usize) -> bool {
    match *claim {
        Claim::None => false,
        Claim::CountAnswer { answer, value } => count_answer(answers, answer, 0, n) == value as i16,
        Claim::CountConsonant { value } => count_consonants(answers, n) == value as i16,
        Claim::CountVowel { value } => count_vowels(answers, n) == value as i16,
        Claim::CountAnswerAfter {
            answer,
            after_index,
            value,
        } => count_answer(answers, answer, after_index as usize + 1, n) == value as i16,
        Claim::CountAnswerBefore {
            answer,
            before_index,
            value,
        } => count_answer(answers, answer, 0, before_index as usize) == value as i16,
        Claim::AnswerOf {
            question_index,
            value,
        } => answers[question_index as usize] == Some(value),
        Claim::FirstWith {
            question_index,
            value,
        } => {
            for i in 0..n {
                if answers[i] == Some(value) {
                    return i == question_index as usize;
                }
            }
            false
        }
        Claim::LastWith {
            question_index,
            value,
        } => {
            let mut last = None;
            for i in 0..n {
                if answers[i] == Some(value) {
                    last = Some(i);
                }
            }
            last == Some(question_index as usize)
        }
        Claim::MostCommon { value } => {
            let mut counts = [0i16; 5];
            for i in 0..n {
                if let Some(a) = answers[i] {
                    counts[a.idx()] += 1;
                }
            }
            let max = *counts.iter().max().unwrap_or(&0);
            counts[value.idx()] == max && counts.iter().filter(|&&c| c == max).count() == 1
        }
    }
}
