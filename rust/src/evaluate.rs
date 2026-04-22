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
        if let Some(a) = answers[i] {
            if a.is_vowel() {
                c += 1;
            }
        }
    }
    c
}

fn count_consonants(answers: &[Option<Answer>], n: usize) -> i16 {
    let mut c: i16 = 0;
    for i in 0..n {
        if let Some(a) = answers[i] {
            if !a.is_vowel() {
                c += 1;
            }
        }
    }
    c
}

fn fill_counts(answers: &[Option<Answer>], n: usize) -> [i16; 5] {
    let mut counts = [0i16; 5];
    for i in 0..n {
        if let Some(a) = answers[i] {
            counts[a.idx()] += 1;
        }
    }
    counts
}

pub fn evaluate(
    fp: &FlatPuzzle,
    qi: usize,
    selected: Answer,
    answers: &[Option<Answer>; MAX_N],
) -> bool {
    let si = selected.idx();
    let on = fp.option_nums[qi][si];
    let n = fp.n;

    match fp.rules[qi] {
        Rule::CountAnswer { answer } => count_answer(answers, answer, 0, n) == on,

        Rule::CountAnswerBefore {
            answer,
            before_index,
        } => count_answer(answers, answer, 0, before_index as usize) == on,

        Rule::CountAnswerAfter {
            answer,
            after_index,
        } => count_answer(answers, answer, after_index as usize + 1, n) == on,

        Rule::CountVowel => count_vowels(answers, n) == on,

        Rule::CountConsonant => count_consonants(answers, n) == on,

        Rule::MostCommonCount => {
            let c = fill_counts(answers, n);
            let max = c.iter().copied().max().unwrap_or(0);
            max == on
        }

        Rule::ClosestAfter {
            after_index,
            answer,
        } => {
            for i in (after_index as usize + 1)..n {
                if answers[i] == Some(answer) {
                    return (i as i16 + 1) == on;
                }
            }
            on == NONE_VAL
        }

        Rule::ClosestBefore {
            before_index,
            answer,
        } => {
            for i in (0..before_index as usize).rev() {
                if answers[i] == Some(answer) {
                    return (i as i16 + 1) == on;
                }
            }
            on == NONE_VAL
        }

        Rule::FirstWith { answer } => {
            for i in 0..n {
                if answers[i] == Some(answer) {
                    return (i as i16 + 1) == on;
                }
            }
            on == NONE_VAL
        }

        Rule::LastWith { answer } => {
            for i in (0..n).rev() {
                if answers[i] == Some(answer) {
                    return (i as i16 + 1) == on;
                }
            }
            on == NONE_VAL
        }

        Rule::PrevSame => {
            for i in (0..qi).rev() {
                if answers[i] == Some(selected) {
                    return (i as i16 + 1) == on;
                }
            }
            on == NONE_VAL
        }

        Rule::NextSame => {
            for i in (qi + 1)..n {
                if answers[i] == Some(selected) {
                    return (i as i16 + 1) == on;
                }
            }
            on == NONE_VAL
        }

        Rule::OnlySame => {
            let mut match_count = 0u8;
            let mut first_match: i16 = 0;
            for i in 0..n {
                if i != qi && answers[i] == Some(selected) {
                    match_count += 1;
                    if match_count == 1 {
                        first_match = i as i16 + 1;
                    }
                }
            }
            match_count == 1 && first_match == on
        }

        Rule::SameAs => {
            let target_q = on - 1;
            if target_q < 0 || target_q >= n as i16 {
                return false;
            }
            match answers[target_q as usize] {
                Some(a) => a == selected,
                None => false,
            }
        }

        Rule::OnlyOdd { answer } => {
            let mut match_count = 0u8;
            let mut first_match: i16 = 0;
            for i in 0..n {
                if (i + 1) % 2 == 1 && answers[i] == Some(answer) {
                    match_count += 1;
                    if match_count == 1 {
                        first_match = i as i16 + 1;
                    }
                }
            }
            match_count == 1 && first_match == on
        }

        Rule::ConsecIdent => {
            let mut pair_count = 0u8;
            let mut pair_first: i16 = -1;
            for i in 0..n.saturating_sub(1) {
                if let (Some(a), Some(b)) = (answers[i], answers[i + 1]) {
                    if a == b {
                        pair_count += 1;
                        if pair_count == 1 {
                            pair_first = i as i16;
                        }
                    }
                }
            }
            pair_count == 1 && pair_first == on
        }

        Rule::AnswerOf { question_index } => {
            let ov_answer = fp.option_answers[qi][si];
            match answers[question_index as usize] {
                Some(other) => other as u8 == ov_answer,
                None => false,
            }
        }

        Rule::LeastCommon => {
            let c = fill_counts(answers, n);
            let min = c.iter().copied().min().unwrap_or(0);
            c[si] == min
        }

        Rule::MostCommon => {
            let c = fill_counts(answers, n);
            let max = c.iter().copied().max().unwrap_or(0);
            c[si] == max
        }

        Rule::Unique => count_answer(answers, selected, 0, n) == 1,

        Rule::EqualCount { answer } => {
            let ref_count = count_answer(answers, answer, 0, n);
            let sel_count = count_answer(answers, selected, 0, n);
            selected != answer && ref_count == sel_count
        }

        Rule::AnswerIsSelf => true,

        Rule::LetterDist {
            other_question_index,
        } => match answers[other_question_index as usize] {
            Some(other) => {
                let dist = (si as i16 - other.idx() as i16).abs();
                dist == on
            }
            None => false,
        },

        Rule::TrueStmt => {
            let claims = &fp.option_claims[qi];
            let mut true_count = 0u8;
            let mut selected_is_true = false;
            for i in 0..5usize {
                if claims[i] == Claim::None {
                    continue;
                }
                let is_true = evaluate_claim(&claims[i], answers, n);
                if is_true {
                    true_count += 1;
                }
                if i == si && is_true {
                    selected_is_true = true;
                }
            }
            selected_is_true && true_count == 1
        }
    }
}

pub fn evaluate_claim(claim: &Claim, answers: &[Option<Answer>; MAX_N], n: usize) -> bool {
    match *claim {
        Claim::None => false,
        Claim::CountAnswerEquals { answer, value } => {
            count_answer(answers, answer, 0, n) == value as i16
        }
        Claim::CountConsonantEquals { value } => count_consonants(answers, n) == value as i16,
        Claim::CountVowelEquals { value } => count_vowels(answers, n) == value as i16,
        Claim::CountAnswerAfterEquals {
            answer,
            after_index,
            value,
        } => count_answer(answers, answer, after_index as usize + 1, n) == value as i16,
        Claim::CountAnswerBeforeEquals {
            answer,
            before_index,
            value,
        } => count_answer(answers, answer, 0, before_index as usize) == value as i16,
    }
}
