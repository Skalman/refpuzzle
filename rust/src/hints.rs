use crate::types::*;

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Force { qi: usize, answer: Answer },
    Eliminate { qi: usize, oi: usize },
    Contradiction { #[allow(dead_code)] qi: usize },
}

pub fn apply_action(
    action: &Action,
    answers: &mut [Option<Answer>; MAX_N],
    eliminated: &mut [u8; MAX_N],
) {
    match *action {
        Action::Force { qi, answer } => {
            let oi = answer.idx();
            eliminated[qi] = 0b11111 ^ (1 << oi);
            answers[qi] = Some(answer);
        }
        Action::Eliminate { qi, oi } => {
            eliminated[qi] |= 1 << oi;
        }
        Action::Contradiction { .. } => {}
    }
}

fn remaining_count(eliminated: u8) -> u32 {
    (!eliminated & 0b11111u8).count_ones()
}

struct CountResult {
    count: i16,
    remaining: i16,
}

#[derive(Clone, Copy)]
enum CountPred {
    IsAnswer(Answer),
    IsVowel,
    IsConsonant,
}

impl CountPred {
    #[inline(always)]
    fn matches(self, a: Answer) -> bool {
        match self {
            CountPred::IsAnswer(target) => a == target,
            CountPred::IsVowel => a.is_vowel(),
            CountPred::IsConsonant => !a.is_vowel(),
        }
    }
}

fn count_matching(
    answers: &[Option<Answer>; MAX_N],
    pred: CountPred,
    from: usize,
    to: usize,
) -> CountResult {
    let mut count: i16 = 0;
    let mut remaining: i16 = 0;
    for i in from..to {
        match answers[i] {
            None => remaining += 1,
            Some(a) if pred.matches(a) => count += 1,
            _ => {}
        }
    }
    CountResult { count, remaining }
}

fn count_pred(r: &Rule) -> Option<CountPred> {
    match *r {
        Rule::CountAnswer { answer }
        | Rule::CountAnswerBefore { answer, .. }
        | Rule::CountAnswerAfter { answer, .. } => Some(CountPred::IsAnswer(answer)),
        Rule::CountVowel => Some(CountPred::IsVowel),
        Rule::CountConsonant => Some(CountPred::IsConsonant),
        _ => None,
    }
}

fn count_range(r: &Rule, n: usize) -> (usize, usize) {
    match *r {
        Rule::CountAnswerBefore { before_index, .. } => (0, before_index as usize),
        Rule::CountAnswerAfter { after_index, .. } => (after_index as usize + 1, n),
        _ => (0, n),
    }
}

fn count_answer_simple(answers: &[Option<Answer>; MAX_N], target: Answer, from: usize, to: usize) -> i16 {
    let mut c: i16 = 0;
    for i in from..to {
        if answers[i] == Some(target) {
            c += 1;
        }
    }
    c
}

pub fn find_action_fast(
    fp: &FlatPuzzle,
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
) -> Option<Action> {
    let n = fp.n;

    // ── Contradictions ──
    for qi in 0..n {
        let Some(a) = answers[qi] else { continue };
        let r = &fp.rules[qi];
        let ai = a.idx();
        let on = fp.option_nums[qi][ai];

        match *r {
            Rule::CountAnswer { answer }
            | Rule::CountAnswerBefore { answer, .. }
            | Rule::CountAnswerAfter { answer, .. } => {
                if on != NAN_VAL {
                    let (from, to) = count_range(r, n);
                    let cr = count_matching(answers, CountPred::IsAnswer(answer), from, to);
                    if cr.count > on || cr.count + cr.remaining < on {
                        return Some(Action::Contradiction { qi });
                    }
                }
            }
            Rule::CountVowel | Rule::CountConsonant => {
                if on != NAN_VAL {
                    let pred = if matches!(*r, Rule::CountVowel) { CountPred::IsVowel } else { CountPred::IsConsonant };
                    let cr = count_matching(answers, pred, 0, n);
                    if cr.count > on || cr.count + cr.remaining < on {
                        return Some(Action::Contradiction { qi });
                    }
                }
            }
            Rule::AnswerOf { question_index } => {
                let ov = fp.option_answers[qi][ai];
                if let Some(target) = answers[question_index as usize] {
                    if target as u8 != ov {
                        return Some(Action::Contradiction { qi });
                    }
                }
            }
            Rule::LetterDist { other_question_index } => {
                if let Some(other) = answers[other_question_index as usize] {
                    let dist = (ai as i16 - other.idx() as i16).abs();
                    if dist != on {
                        return Some(Action::Contradiction { qi });
                    }
                }
            }
            Rule::Unique => {
                if count_answer_simple(answers, a, 0, n) > 1 {
                    return Some(Action::Contradiction { qi });
                }
            }
            Rule::ClosestAfter { answer, .. } | Rule::FirstWith { answer } => {
                let scan_start: usize = match *r {
                    Rule::ClosestAfter { after_index, .. } => after_index as usize + 1,
                    _ => 0,
                };
                if on != NONE_VAL {
                    let pos = on - 1;
                    if pos >= 0 && (pos as usize) < n {
                        if let Some(pa) = answers[pos as usize] {
                            if pa != answer {
                                return Some(Action::Contradiction { qi });
                            }
                        }
                    }
                    if pos > 0 {
                        for j in scan_start..(pos as usize) {
                            if answers[j] == Some(answer) {
                                return Some(Action::Contradiction { qi });
                            }
                        }
                    }
                } else {
                    for j in scan_start..n {
                        if answers[j] == Some(answer) {
                            return Some(Action::Contradiction { qi });
                        }
                    }
                }
            }
            Rule::ClosestBefore { answer, .. } | Rule::LastWith { answer } => {
                let before_idx: usize = match *r {
                    Rule::ClosestBefore { before_index, .. } => before_index as usize,
                    _ => n,
                };
                if on != NONE_VAL {
                    let pos = on - 1;
                    if pos >= 0 && (pos as usize) < n {
                        if let Some(pa) = answers[pos as usize] {
                            if pa != answer {
                                return Some(Action::Contradiction { qi });
                            }
                        }
                    }
                    if pos >= 0 {
                        for j in ((pos as usize + 1)..before_idx).rev() {
                            if answers[j] == Some(answer) {
                                return Some(Action::Contradiction { qi });
                            }
                        }
                    }
                } else {
                    for j in 0..before_idx {
                        if answers[j] == Some(answer) {
                            return Some(Action::Contradiction { qi });
                        }
                    }
                }
            }
            Rule::SameAs => {
                let tq = on - 1;
                if tq >= 0 && (tq as usize) < n {
                    if let Some(ta) = answers[tq as usize] {
                        if ta != a {
                            return Some(Action::Contradiction { qi });
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // ── Forced values ──
    for qi in 0..n {
        if answers[qi].is_some() { continue; }
        let r = &fp.rules[qi];

        if remaining_count(eliminated[qi]) == 1 {
            let oi = (!eliminated[qi] & 0b11111).trailing_zeros();
            return Some(Action::Force { qi, answer: LETTERS[oi as usize] });
        }

        if let Rule::AnswerOf { question_index } = *r {
            if let Some(target) = answers[question_index as usize] {
                for oi in 0..5usize {
                    if fp.option_answers[qi][oi] == target as u8 {
                        return Some(Action::Force { qi, answer: LETTERS[oi] });
                    }
                }
            }
        }

        for other in 0..n {
            let Some(other_ans) = answers[other] else { continue };
            if let Rule::AnswerOf { question_index } = fp.rules[other] {
                if question_index as usize == qi {
                    let implied = fp.option_answers[other][other_ans.idx()];
                    if implied <= 4 {
                        return Some(Action::Force { qi, answer: LETTERS[implied as usize] });
                    }
                }
            }
            if let Rule::SameAs = fp.rules[other] {
                let target_q = fp.option_nums[other][other_ans.idx()] - 1;
                if target_q >= 0 && target_q as usize == qi {
                    return Some(Action::Force { qi, answer: other_ans });
                }
            }
        }

        if let Rule::LetterDist { other_question_index } = *r {
            if let Some(other_ans) = answers[other_question_index as usize] {
                let other_idx = other_ans.idx();
                let mut valid_count = 0u8;
                let mut valid_letter = Answer::A;
                for oi in 0..5usize {
                    if (eliminated[qi] >> oi) & 1 == 1 { continue; }
                    let dist = (oi as i16 - other_idx as i16).abs();
                    if dist == fp.option_nums[qi][oi] {
                        valid_count += 1;
                        valid_letter = LETTERS[oi];
                    }
                }
                if valid_count == 1 {
                    return Some(Action::Force { qi, answer: valid_letter });
                }
            }
        }

        if let Some(pred) = count_pred(r) {
            let (from, to) = count_range(r, n);
            let cr = count_matching(answers, pred, from, to);
            if cr.remaining == 0 {
                for oi in 0..5usize {
                    if (eliminated[qi] >> oi) & 1 == 1 { continue; }
                    if fp.option_nums[qi][oi] == cr.count {
                        return Some(Action::Force { qi, answer: LETTERS[oi] });
                    }
                }
            }
        }
    }

    // ── Eliminations ──
    for qi in 0..n {
        if answers[qi].is_some() { continue; }
        let r = &fp.rules[qi];

        for oi in 0..5usize {
            if (eliminated[qi] >> oi) & 1 == 1 { continue; }
            let on = fp.option_nums[qi][oi];

            match *r {
                Rule::CountAnswer { answer }
                | Rule::CountAnswerBefore { answer, .. }
                | Rule::CountAnswerAfter { answer, .. } => {
                    if on != NAN_VAL {
                        let (from, to) = count_range(r, n);
                        let cr = count_matching(answers, CountPred::IsAnswer(answer), from, to);
                        if cr.count > on || cr.count + cr.remaining < on {
                            return Some(Action::Eliminate { qi, oi });
                        }
                    }
                }
                Rule::CountVowel | Rule::CountConsonant => {
                    if on != NAN_VAL {
                        let pred = if matches!(*r, Rule::CountVowel) { CountPred::IsVowel } else { CountPred::IsConsonant };
                        let cr = count_matching(answers, pred, 0, n);
                        if cr.count > on || cr.count + cr.remaining < on {
                            return Some(Action::Eliminate { qi, oi });
                        }
                    }
                }
                Rule::AnswerOf { question_index } => {
                    let ov = fp.option_answers[qi][oi];
                    if let Some(target) = answers[question_index as usize] {
                        if target as u8 != ov {
                            return Some(Action::Eliminate { qi, oi });
                        }
                    }
                }
                Rule::LetterDist { other_question_index } => {
                    if let Some(other) = answers[other_question_index as usize] {
                        let dist = (oi as i16 - other.idx() as i16).abs();
                        if dist != on {
                            return Some(Action::Eliminate { qi, oi });
                        }
                    }
                }
                Rule::ClosestAfter { answer, .. } | Rule::FirstWith { answer } => {
                    let scan_start: usize = match *r {
                        Rule::ClosestAfter { after_index, .. } => after_index as usize + 1,
                        _ => 0,
                    };
                    if on != NONE_VAL {
                        let pos = on - 1;
                        if pos >= 0 && (pos as usize) < n {
                            if let Some(pa) = answers[pos as usize] {
                                if pa != answer {
                                    return Some(Action::Eliminate { qi, oi });
                                }
                            }
                        }
                        if pos > 0 {
                            for j in scan_start..(pos as usize) {
                                if answers[j] == Some(answer) {
                                    return Some(Action::Eliminate { qi, oi });
                                }
                            }
                        }
                    } else {
                        for j in scan_start..n {
                            if answers[j] == Some(answer) {
                                return Some(Action::Eliminate { qi, oi });
                            }
                        }
                    }
                }
                Rule::ClosestBefore { answer, .. } | Rule::LastWith { answer } => {
                    let before_idx: usize = match *r {
                        Rule::ClosestBefore { before_index, .. } => before_index as usize,
                        _ => n,
                    };
                    if on != NONE_VAL {
                        let pos = on - 1;
                        if pos >= 0 && (pos as usize) < n {
                            if let Some(pa) = answers[pos as usize] {
                                if pa != answer {
                                    return Some(Action::Eliminate { qi, oi });
                                }
                            }
                        }
                        if pos >= 0 {
                            for j in ((pos as usize + 1)..before_idx).rev() {
                                if answers[j] == Some(answer) {
                                    return Some(Action::Eliminate { qi, oi });
                                }
                            }
                        }
                    } else {
                        for j in 0..before_idx {
                            if answers[j] == Some(answer) {
                                return Some(Action::Eliminate { qi, oi });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    None
}

pub fn find_lookahead_action(
    fp: &FlatPuzzle,
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
) -> Option<Action> {
    let n = fp.n;
    for qi in 0..n {
        if answers[qi].is_some() { continue; }
        for oi in 0..5u8 {
            if (eliminated[qi] >> oi) & 1 == 1 { continue; }
            if trace_leads_to_contradiction(fp, answers, eliminated, qi, oi) {
                return Some(Action::Eliminate { qi, oi: oi as usize });
            }
        }
    }
    None
}

fn trace_leads_to_contradiction(
    fp: &FlatPuzzle,
    orig_answers: &[Option<Answer>; MAX_N],
    orig_eliminated: &[u8; MAX_N],
    assume_qi: usize,
    assume_oi: u8,
) -> bool {
    let n = fp.n;
    let mut answers = *orig_answers;
    let mut eliminated = *orig_eliminated;

    answers[assume_qi] = Some(LETTERS[assume_oi as usize]);
    eliminated[assume_qi] = 0b11111 ^ (1 << assume_oi);

    for _ in 0..n * 5 {
        match find_action_fast(fp, &answers, &eliminated) {
            Some(Action::Contradiction { .. }) => return true,
            Some(Action::Force { qi, answer }) => {
                let oi = answer.idx();
                eliminated[qi] = 0b11111 ^ (1 << oi);
                answers[qi] = Some(answer);
            }
            Some(Action::Eliminate { qi, oi }) => {
                eliminated[qi] |= 1 << oi;
            }
            None => break,
        }
    }
    false
}
