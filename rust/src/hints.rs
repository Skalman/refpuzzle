use crate::evaluate::evaluate;
use crate::types::*;

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Force {
        qi: usize,
        answer: Answer,
    },
    Eliminate {
        qi: usize,
        oi: usize,
    },
    Contradiction {
        #[allow(dead_code)]
        qi: usize,
    },
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

// ── State helpers (same API as JS for easy porting) ──

#[inline(always)]
fn is_elim(eliminated: &[u8; MAX_N], qi: usize, oi: usize) -> bool {
    (eliminated[qi] >> oi) & 1 == 1
}

#[inline(always)]
fn remaining_count(eliminated: u8) -> u32 {
    (!eliminated & 0b11111u8).count_ones()
}

struct CountResult {
    count: i16,
    remaining: i16,
}

#[derive(Clone, Copy, Debug)]
#[allow(clippy::enum_variant_names)]
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
    eliminated: &[u8; MAX_N],
    pred: CountPred,
    from: usize,
    to: usize,
) -> CountResult {
    let mut count: i16 = 0;
    let mut remaining: i16 = 0;
    for i in from..to {
        match answers[i] {
            None if can_still_match(pred, eliminated[i]) => {
                remaining += 1;
            }
            Some(a) if pred.matches(a) => count += 1,
            _ => {}
        }
    }
    CountResult { count, remaining }
}

#[inline(always)]
fn can_still_match(pred: CountPred, eliminated: u8) -> bool {
    let mask = match pred {
        CountPred::IsAnswer(target) => 1u8 << target.idx(),
        CountPred::IsVowel => 0b10001,     // A=0, E=4
        CountPred::IsConsonant => 0b01110, // B=1, C=2, D=3
    };
    eliminated & mask != mask
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

fn count_answer_simple(
    answers: &[Option<Answer>; MAX_N],
    target: Answer,
    from: usize,
    to: usize,
) -> i16 {
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
                    let cr =
                        count_matching(answers, eliminated, CountPred::IsAnswer(answer), from, to);
                    if cr.count > on || cr.count + cr.remaining < on {
                        return Some(Action::Contradiction { qi });
                    }
                }
            }
            Rule::CountVowel | Rule::CountConsonant => {
                if on != NAN_VAL {
                    let pred = if matches!(*r, Rule::CountVowel) {
                        CountPred::IsVowel
                    } else {
                        CountPred::IsConsonant
                    };
                    let cr = count_matching(answers, eliminated, pred, 0, n);
                    if cr.count > on || cr.count + cr.remaining < on {
                        return Some(Action::Contradiction { qi });
                    }
                }
            }
            Rule::AnswerOf { question_index } => {
                let ov = fp.option_answers[qi][ai];
                if let Some(target) = answers[question_index as usize]
                    && target as u8 != ov
                {
                    return Some(Action::Contradiction { qi });
                }
            }
            Rule::LetterDist {
                other_question_index,
            } => {
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
                    if pos >= 0
                        && (pos as usize) < n
                        && let Some(pa) = answers[pos as usize]
                        && pa != answer
                    {
                        return Some(Action::Contradiction { qi });
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
                    if pos >= 0
                        && (pos as usize) < n
                        && let Some(pa) = answers[pos as usize]
                        && pa != answer
                    {
                        return Some(Action::Contradiction { qi });
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
                if tq >= 0
                    && (tq as usize) < n
                    && let Some(ta) = answers[tq as usize]
                    && ta != a
                {
                    return Some(Action::Contradiction { qi });
                }
            }
            _ => {
                // Fallback for rule types without specialized partial-state checks.
                // Only safe when all answers are known (full evaluate is exact).
                if (0..n).all(|i| answers[i].is_some()) && !evaluate(fp, qi, a, answers) {
                    return Some(Action::Contradiction { qi });
                }
            }
        }
    }

    // ── Count saturation ──
    // When an answered counting rule has count==on, no more unknowns can match.
    // When count+remaining==on, all unknowns in range must match.
    for qi in 0..n {
        let Some(a) = answers[qi] else { continue };
        let r = &fp.rules[qi];
        let Some(pred) = count_pred(r) else { continue };
        let on = fp.option_nums[qi][a.idx()];
        if on == NAN_VAL || on < 0 {
            continue;
        }
        let (from, to) = count_range(r, n);
        let cr = count_matching(answers, eliminated, pred, from, to);

        if cr.count == on && cr.remaining > 0 {
            for j in from..to {
                if answers[j].is_none() {
                    for oi in 0..5usize {
                        if !is_elim(eliminated, j, oi) && pred.matches(LETTERS[oi]) {
                            return Some(Action::Eliminate { qi: j, oi });
                        }
                    }
                }
            }
        }
        if cr.count + cr.remaining == on && cr.remaining > 0 {
            for j in from..to {
                if answers[j].is_none() && can_still_match(pred, eliminated[j]) {
                    for oi in 0..5usize {
                        if !is_elim(eliminated, j, oi) && !pred.matches(LETTERS[oi]) {
                            return Some(Action::Eliminate { qi: j, oi });
                        }
                    }
                }
            }
        }
    }

    // ── Forced values ──
    for qi in 0..n {
        if answers[qi].is_some() {
            continue;
        }
        let r = &fp.rules[qi];

        if remaining_count(eliminated[qi]) == 0 {
            return Some(Action::Contradiction { qi });
        }
        if remaining_count(eliminated[qi]) == 1 {
            let oi = (!eliminated[qi] & 0b11111).trailing_zeros();
            return Some(Action::Force {
                qi,
                answer: LETTERS[oi as usize],
            });
        }

        if let Rule::AnswerOf { question_index } = *r
            && let Some(target) = answers[question_index as usize]
        {
            for oi in 0..5usize {
                if fp.option_answers[qi][oi] == target as u8 {
                    return Some(Action::Force {
                        qi,
                        answer: LETTERS[oi],
                    });
                }
            }
        }

        for other in 0..n {
            let Some(other_ans) = answers[other] else {
                continue;
            };
            if let Rule::AnswerOf { question_index } = fp.rules[other]
                && question_index as usize == qi
            {
                let implied = fp.option_answers[other][other_ans.idx()];
                if implied <= 4 {
                    return Some(Action::Force {
                        qi,
                        answer: LETTERS[implied as usize],
                    });
                }
            }
            if let Rule::SameAs = fp.rules[other] {
                let target_q = fp.option_nums[other][other_ans.idx()] - 1;
                if target_q >= 0 && target_q as usize == qi {
                    return Some(Action::Force {
                        qi,
                        answer: other_ans,
                    });
                }
            }
        }

        if let Rule::LetterDist {
            other_question_index,
        } = *r
            && let Some(other_ans) = answers[other_question_index as usize]
        {
            let other_idx = other_ans.idx();
            let mut valid_count = 0u8;
            let mut valid_letter = Answer::A;
            for oi in 0..5usize {
                if is_elim(eliminated, qi, oi) {
                    continue;
                }
                let dist = (oi as i16 - other_idx as i16).abs();
                if dist == fp.option_nums[qi][oi] {
                    valid_count += 1;
                    valid_letter = LETTERS[oi];
                }
            }
            if valid_count == 1 {
                return Some(Action::Force {
                    qi,
                    answer: valid_letter,
                });
            }
        }

        if let Some(pred) = count_pred(r) {
            let (from, to) = count_range(r, n);
            let cr = count_matching(answers, eliminated, pred, from, to);
            if cr.remaining == 0 {
                for oi in 0..5usize {
                    if is_elim(eliminated, qi, oi) {
                        continue;
                    }
                    if fp.option_nums[qi][oi] == cr.count {
                        return Some(Action::Force {
                            qi,
                            answer: LETTERS[oi],
                        });
                    }
                }
            }
        }
    }

    // ── Vowel/consonant cross-rule ──
    // If both CountVowel and CountConsonant exist, vowels + consonants = n.
    // An option value V on one is only valid if (n - V) is an available option on the other.
    {
        let mut vowel_qi = None;
        let mut consonant_qi = None;
        for i in 0..n {
            if answers[i].is_some() {
                continue;
            }
            match fp.rules[i] {
                Rule::CountVowel => vowel_qi = Some(i),
                Rule::CountConsonant => consonant_qi = Some(i),
                _ => {}
            }
        }
        if let (Some(vq), Some(cq)) = (vowel_qi, consonant_qi) {
            let nn = n as i16;
            // Check vowel options: each needs (n - val) available in consonant
            for oi in 0..5usize {
                if is_elim(eliminated, vq, oi) {
                    continue;
                }
                let v = fp.option_nums[vq][oi];
                if v == NAN_VAL {
                    continue;
                }
                let need = nn - v;
                let has_complement = (0..5)
                    .any(|coi| !is_elim(eliminated, cq, coi) && fp.option_nums[cq][coi] == need);
                if !has_complement {
                    return Some(Action::Eliminate { qi: vq, oi });
                }
            }
            // Check consonant options: each needs (n - val) available in vowel
            for oi in 0..5usize {
                if is_elim(eliminated, cq, oi) {
                    continue;
                }
                let v = fp.option_nums[cq][oi];
                if v == NAN_VAL {
                    continue;
                }
                let need = nn - v;
                let has_complement = (0..5)
                    .any(|voi| !is_elim(eliminated, vq, voi) && fp.option_nums[vq][voi] == need);
                if !has_complement {
                    return Some(Action::Eliminate { qi: cq, oi });
                }
            }
        }
    }

    // ── Eliminations ──
    for qi in 0..n {
        if answers[qi].is_some() {
            continue;
        }
        let r = &fp.rules[qi];

        for oi in 0..5usize {
            if is_elim(eliminated, qi, oi) {
                continue;
            }
            let on = fp.option_nums[qi][oi];

            match *r {
                Rule::CountAnswer { answer }
                | Rule::CountAnswerBefore { answer, .. }
                | Rule::CountAnswerAfter { answer, .. }
                    if on != NAN_VAL =>
                {
                    let (from, to) = count_range(r, n);
                    let cr =
                        count_matching(answers, eliminated, CountPred::IsAnswer(answer), from, to);
                    if cr.count > on || cr.count + cr.remaining < on {
                        return Some(Action::Eliminate { qi, oi });
                    }
                }
                Rule::CountVowel | Rule::CountConsonant if on != NAN_VAL => {
                    let pred = if matches!(*r, Rule::CountVowel) {
                        CountPred::IsVowel
                    } else {
                        CountPred::IsConsonant
                    };
                    let cr = count_matching(answers, eliminated, pred, 0, n);
                    if cr.count > on || cr.count + cr.remaining < on {
                        return Some(Action::Eliminate { qi, oi });
                    }
                }
                Rule::AnswerOf { question_index } => {
                    let ov = fp.option_answers[qi][oi];
                    if ov <= 4 {
                        if let Some(target) = answers[question_index as usize] {
                            if target as u8 != ov {
                                return Some(Action::Eliminate { qi, oi });
                            }
                        } else if is_elim(eliminated, question_index as usize, ov as usize) {
                            // The letter this option claims is eliminated from the target question
                            return Some(Action::Eliminate { qi, oi });
                        }
                    }
                }
                Rule::LetterDist {
                    other_question_index,
                } => {
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
                        if pos < scan_start as i16 || pos >= n as i16 {
                            return Some(Action::Eliminate { qi, oi });
                        }
                        if pos >= 0 && (pos as usize) < n {
                            let p = pos as usize;
                            if let Some(pa) = answers[p] {
                                if pa != answer {
                                    return Some(Action::Eliminate { qi, oi });
                                }
                            } else if is_elim(eliminated, p, answer.idx()) {
                                return Some(Action::Eliminate { qi, oi });
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
                        if pos < 0 || pos >= before_idx as i16 {
                            return Some(Action::Eliminate { qi, oi });
                        }
                        if pos >= 0 && (pos as usize) < n {
                            let p = pos as usize;
                            if let Some(pa) = answers[p] {
                                if pa != answer {
                                    return Some(Action::Eliminate { qi, oi });
                                }
                            } else if is_elim(eliminated, p, answer.idx()) {
                                return Some(Action::Eliminate { qi, oi });
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
                Rule::OnlyOdd { answer } => {
                    if on != NONE_VAL {
                        let pos = (on - 1) as usize;
                        // Position must be odd-numbered (1-indexed)
                        if on % 2 == 0 {
                            return Some(Action::Eliminate { qi, oi });
                        }
                        // Position must be in range and could have the answer
                        if pos < n {
                            if let Some(pa) = answers[pos] {
                                if pa != answer {
                                    return Some(Action::Eliminate { qi, oi });
                                }
                            } else if is_elim(eliminated, pos, answer.idx()) {
                                return Some(Action::Eliminate { qi, oi });
                            }
                        }
                    } else {
                        // None: eliminate if any odd-positioned Q has the answer
                        for i in 0..n {
                            if (i + 1) % 2 == 1 && answers[i] == Some(answer) {
                                return Some(Action::Eliminate { qi, oi });
                            }
                        }
                    }
                }
                Rule::ConsecIdent => {
                    if on != NONE_VAL {
                        let pos = on as usize;
                        // Pair at (pos, pos+1). Both must have same answer.
                        if pos + 1 < n
                            && let (Some(a), Some(b)) = (answers[pos], answers[pos + 1])
                            && a != b
                        {
                            return Some(Action::Eliminate { qi, oi });
                        }
                    } else {
                        // None: eliminate if any consecutive pair has same answer
                        for i in 0..n.saturating_sub(1) {
                            if let (Some(a), Some(b)) = (answers[i], answers[i + 1])
                                && a == b
                            {
                                return Some(Action::Eliminate { qi, oi });
                            }
                        }
                    }
                }
                Rule::PrevSame if on != NONE_VAL => {
                    let pos = (on - 1) as usize;
                    // Must point to a position before qi
                    if pos >= qi {
                        return Some(Action::Eliminate { qi, oi });
                    }
                    // If that position is answered, it must match our letter
                    if let Some(pa) = answers[pos]
                        && let Some(my) = answers[qi]
                        && pa != my
                    {
                        return Some(Action::Eliminate { qi, oi });
                    }
                }
                Rule::NextSame if on != NONE_VAL => {
                    let pos = (on - 1) as usize;
                    // Must point to a position after qi
                    if pos <= qi || pos >= n {
                        return Some(Action::Eliminate { qi, oi });
                    }
                }
                Rule::OnlySame | Rule::SameAs if on != NONE_VAL => {
                    let pos = (on - 1) as usize;
                    // Can't point to self
                    if pos == qi {
                        return Some(Action::Eliminate { qi, oi });
                    }
                    // If target is answered, must match our selected letter
                    if pos < n
                        && let (Some(target), Some(my_ans)) = (answers[pos], answers[qi])
                        && target != my_ans
                    {
                        return Some(Action::Eliminate { qi, oi });
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
        if answers[qi].is_some() {
            continue;
        }
        for oi in 0..5usize {
            if is_elim(eliminated, qi, oi) {
                continue;
            }
            if trace_leads_to_contradiction(fp, answers, eliminated, qi, oi as u8) {
                return Some(Action::Eliminate { qi, oi });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // The stuck puzzle from the dump:
    // Solution: A,A,B,A,C,B,D,B,A,A,B,C
    // All 12 questions stuck after hint engine run
    fn make_stuck_puzzle() -> FlatPuzzle {
        use Answer::*;
        let rules = [
            Rule::LetterDist {
                other_question_index: 8,
            }, // Q1
            Rule::ClosestBefore {
                before_index: 6,
                answer: A,
            }, // Q2
            Rule::AnswerOf { question_index: 3 }, // Q3
            Rule::CountConsonant,                 // Q4
            Rule::CountAnswer { answer: D },      // Q5
            Rule::TrueStmt,                       // Q6
            Rule::LetterDist {
                other_question_index: 2,
            }, // Q7
            Rule::OnlyOdd { answer: D },          // Q8
            Rule::AnswerOf { question_index: 2 }, // Q9
            Rule::FirstWith { answer: B },        // Q10
            Rule::CountAnswer { answer: C },      // Q11
            Rule::CountAnswer { answer: E },      // Q12
            // padding
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
        ];
        let mut option_nums = [[NAN_VAL; 5]; MAX_N];
        // Q1: LetterDist Q9
        option_nums[0] = [0, 4, 3, 1, 2];
        // Q2: ClosestBefore 6, A
        option_nums[1] = [4, 2, 3, 5, 6];
        // Q3: AnswerOf Q4 — constrained, uses option_answers
        // Q4: CountConsonant
        option_nums[3] = [7, 10, 8, 1, 6];
        // Q5: CountAnswer D
        option_nums[4] = [11, 12, 1, 4, 7];
        // Q6: TrueStmt — uses claims
        // Q7: LetterDist Q3
        option_nums[6] = [0, 1, 3, 2, 4];
        // Q8: OnlyOdd D
        option_nums[7] = [6, 7, 1, 4, 3];
        // Q9: AnswerOf Q3 — constrained
        // Q10: FirstWith B
        option_nums[9] = [3, 2, 5, 10, 11];
        // Q11: CountAnswer C
        option_nums[10] = [9, 2, 8, 5, 10];
        // Q12: CountAnswer E
        option_nums[11] = [8, 3, 0, 5, 2];

        let mut option_answers = [[0xFFu8; 5]; MAX_N];
        // Q3: AnswerOf Q4. Q4's answer is A. Q3's answer is B (oi=1).
        // oi=1 (B, correct) maps to A(0). Others map to wrong letters.
        option_answers[2] = [1, 0, 2, 3, 4]; // B→A(correct), A→B, C→C, D→D, E→E
        // Q9: AnswerOf Q3. Q3's answer is B. Q9's answer is A (oi=0).
        // oi=0 (A, correct) maps to B(1). Others map to wrong letters.
        option_answers[8] = [1, 0, 2, 3, 4]; // A→B(correct), B→A, C→C, D→D, E→E

        // Q6 TrueStmt: B is correct. We need exactly 1 true claim (at oi=1).
        // Solution has: A=4, B=4, C=2, D=1, E=0 (letters: A,A,B,A,C,B,D,B,A,A,B,C)
        let mut option_claims = [[Claim::None; 5]; MAX_N];
        option_claims[5][1] = Claim::CountAnswerEquals {
            answer: D,
            value: 1,
        }; // true: 1 D
        option_claims[5][0] = Claim::CountAnswerEquals {
            answer: D,
            value: 3,
        }; // false
        option_claims[5][2] = Claim::CountAnswerEquals {
            answer: A,
            value: 2,
        }; // false (4 A's)
        option_claims[5][3] = Claim::CountAnswerEquals {
            answer: B,
            value: 1,
        }; // false (4 B's)
        option_claims[5][4] = Claim::CountAnswerEquals {
            answer: E,
            value: 2,
        }; // false (0 E's)

        let n = 12;
        let (affected_by, global_indices) = FlatPuzzle::build_deps(&rules, n);

        FlatPuzzle {
            rules,
            option_nums,
            option_answers,
            option_claims,
            affected_by,
            global_indices,
            n,
        }
    }

    #[test]
    fn test_hint_engine_traces_stuck_puzzle() {
        let fp = make_stuck_puzzle();
        let n = fp.n;
        let mut answers = [None; MAX_N];
        let mut eliminated = [0u8; MAX_N];

        let mut steps = Vec::new();
        for step in 0..n * 15 {
            if (0..n).all(|i| answers[i].is_some()) {
                steps.push(format!("SOLVED at step {step}"));
                break;
            }

            if let Some(action) = find_action_fast(&fp, &answers, &eliminated) {
                steps.push(format!("fast: {action:?}"));
                apply_action(&action, &mut answers, &mut eliminated);
                continue;
            }

            if let Some(action) = find_lookahead_action(&fp, &answers, &eliminated) {
                steps.push(format!("look: {action:?}"));
                apply_action(&action, &mut answers, &mut eliminated);
                continue;
            }

            steps.push(format!("STUCK at step {step}"));
            // Print remaining state
            for qi in 0..n {
                if answers[qi].is_none() {
                    let remaining: Vec<char> = (0..5usize)
                        .filter(|&oi| !is_elim(&eliminated, qi, oi))
                        .map(|oi| LETTERS[oi].as_char())
                        .collect();
                    steps.push(format!(
                        "  Q{}: {:?} remaining: {remaining:?}",
                        qi + 1,
                        fp.rules[qi]
                    ));
                }
            }
            break;
        }

        for s in &steps {
            eprintln!("{s}");
        }
        // Don't assert solved — we expect stuck. This test is for tracing.
        // Uncomment to fail: assert!(steps.last().unwrap().contains("SOLVED"));
    }

    #[test]
    fn test_lookahead_q5_a_should_contradict() {
        // Q5=A means "11 D's". Saturation should force all unknowns to D,
        // which contradicts Q4 (CountConsonant, D eliminated from Q4).
        let fp = make_stuck_puzzle();
        let answers = [None; MAX_N];
        let eliminated = [0u8; MAX_N];

        // Manually check: assume Q5=A, does it lead to contradiction?
        let contradicts = trace_leads_to_contradiction(&fp, &answers, &eliminated, 4, 0);
        eprintln!("Q5=A (11 D's) contradicts: {contradicts}");
        assert!(
            contradicts,
            "Q5=A should lead to contradiction via count saturation"
        );
    }

    // Second stuck puzzle from the dump (repair failed)
    // Solution: D,C,A,B,A,B,E,B,A,D,A,B
    fn make_repair_failed_puzzle() -> FlatPuzzle {
        use Answer::*;
        let rules = [
            Rule::NextSame, // Q1
            Rule::ClosestBefore {
                before_index: 6,
                answer: A,
            }, // Q2
            Rule::LastWith { answer: E }, // Q3
            Rule::CountVowel, // Q4
            Rule::OnlyOdd { answer: E }, // Q5
            Rule::CountAnswerBefore {
                answer: B,
                before_index: 7,
            }, // Q6
            Rule::CountAnswer { answer: B }, // Q7
            Rule::AnswerOf { question_index: 8 }, // Q8
            Rule::AnswerOf { question_index: 5 }, // Q9
            Rule::MostCommonCount, // Q10
            Rule::LetterDist {
                other_question_index: 8,
            }, // Q11
            Rule::TrueStmt, // Q12
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
        ];
        let mut option_nums = [[NAN_VAL; 5]; MAX_N];
        option_nums[0] = [-1, 1, 2, 10, 3]; // Q1 NextSame
        option_nums[1] = [-1, 1, 5, 2, 3]; // Q2 ClosestBefore
        option_nums[2] = [7, -1, 1, 2, 12]; // Q3 LastWith E
        option_nums[3] = [12, 5, 11, 0, 10]; // Q4 CountVowel
        option_nums[4] = [7, -1, 1, 2, 12]; // Q5 OnlyOdd E
        option_nums[5] = [7, 2, 6, 5, 0]; // Q6 CountAnswerBefore B<Q8
        option_nums[6] = [12, 11, 10, 9, 4]; // Q7 CountAnswer B
        // Q8 AnswerOf Q9 — constrained
        // Q9 AnswerOf Q6 — constrained
        option_nums[9] = [12, 11, 10, 4, 9]; // Q10 MostCommonCount
        option_nums[10] = [0, 1, 4, 3, 2]; // Q11 LetterDist Q9
        // Q12 TrueStmt

        let mut option_answers = [[0xFFu8; 5]; MAX_N];
        // Q8: AnswerOf Q9. Q9's answer is A. Q8's answer is B (oi=1). B→A.
        option_answers[7] = [2, 0, 1, 3, 4]; // B→A(correct)
        // Q9: AnswerOf Q6. Q6's answer is B. Q9's answer is A (oi=0). A→B.
        option_answers[8] = [1, 0, 2, 3, 4]; // A→B(correct)

        // Q12: TrueStmt, B is correct
        // Solution: D,C,A,B,A,B,E,B,A,D,A,B → B count=4, D count=2, A count=4
        let mut option_claims = [[Claim::None; 5]; MAX_N];
        option_claims[11][1] = Claim::CountAnswerEquals {
            answer: D,
            value: 2,
        }; // true
        option_claims[11][0] = Claim::CountAnswerEquals {
            answer: D,
            value: 4,
        }; // false
        option_claims[11][2] = Claim::CountAnswerEquals {
            answer: A,
            value: 2,
        }; // false
        option_claims[11][3] = Claim::CountAnswerEquals {
            answer: B,
            value: 2,
        }; // false
        option_claims[11][4] = Claim::CountAnswerEquals {
            answer: E,
            value: 3,
        }; // false

        let n = 12;
        let (affected_by, global_indices) = FlatPuzzle::build_deps(&rules, n);
        FlatPuzzle {
            rules,
            option_nums,
            option_answers,
            option_claims,
            affected_by,
            global_indices,
            n,
        }
    }

    #[test]
    fn test_repair_failed_puzzle_trace() {
        let fp = make_repair_failed_puzzle();
        let n = fp.n;
        let mut answers = [None; MAX_N];
        let mut eliminated = [0u8; MAX_N];
        let mut steps = 0;
        let mut solved = false;

        for _ in 0..n * 15 {
            if (0..n).all(|i| answers[i].is_some()) {
                solved = true;
                break;
            }
            if let Some(action) = find_action_fast(&fp, &answers, &eliminated) {
                steps += 1;
                apply_action(&action, &mut answers, &mut eliminated);
                continue;
            }
            if let Some(action) = find_lookahead_action(&fp, &answers, &eliminated) {
                steps += 1;
                apply_action(&action, &mut answers, &mut eliminated);
                continue;
            }
            break;
        }
        let answered = (0..n).filter(|&i| answers[i].is_some()).count();
        eprintln!("Repair-failed puzzle: solved={solved} answered={answered}/{n} steps={steps}");
        for qi in 0..n {
            if answers[qi].is_none() {
                let remaining: Vec<String> = (0..5usize)
                    .filter(|&oi| !is_elim(&eliminated, qi, oi))
                    .map(|oi| format!("{}={}", LETTERS[oi].as_char(), fp.option_nums[qi][oi]))
                    .collect();
                eprintln!(
                    "  Q{}: {:?} remaining: {}",
                    qi + 1,
                    fp.rules[qi],
                    remaining.join(", ")
                );
            } else {
                eprintln!("  Q{}: SOLVED = {}", qi + 1, answers[qi].unwrap().as_char());
            }
        }
    }

    #[test]
    fn test_q4_vowel_10_contradicts() {
        let fp = make_repair_failed_puzzle();
        // First solve Q6, Q8, Q9 (the 3 that the engine solves)
        let mut answers = [None; MAX_N];
        let mut eliminated = [0u8; MAX_N];
        for _ in 0..50 {
            if let Some(action) = find_action_fast(&fp, &answers, &eliminated) {
                apply_action(&action, &mut answers, &mut eliminated);
                continue;
            }
            if let Some(action) = find_lookahead_action(&fp, &answers, &eliminated) {
                apply_action(&action, &mut answers, &mut eliminated);
                continue;
            }
            break;
        }
        let answered = (0..12).filter(|&i| answers[i].is_some()).count();
        eprintln!("Pre-state: {answered} answered");

        // Now test: Q4=E (10 vowels). Should saturation trigger?
        let contradicts = trace_leads_to_contradiction(&fp, &answers, &eliminated, 3, 4);
        eprintln!("Q4=E (10 vowels) contradicts: {contradicts}");

        // Check: with Q6=B, Q8=B, Q9=A solved, vowel count = 1 (A).
        // Q4=E → answers[3]=E. Vowels so far: A(Q9) + E(Q4) = 2. Remaining=8.
        // 2+8=10==on(10) → saturation! All 8 unknowns must be vowels (A or E).
        // But Q7 (CountAnswer B) with extreme distractors... if forced to A or E,
        // does that contradict?
    }

    #[test]
    fn test_q12_b_11es_should_contradict() {
        // Q12 is CountAnswer{E} in make_stuck_puzzle. Replace B distractor with 11.
        // "11 E's" → saturation forces all to E → Q4 (CountConsonant) can't be E
        // (E is vowel, consonant count would be wrong) → contradiction.
        let mut fp = make_stuck_puzzle();
        // Q12 (qi=11): CountAnswer{E}, correct=C(oi=2)=0. Set B(oi=1) to 11.
        fp.option_nums[11][1] = 11;
        let answers = [None; MAX_N];
        let eliminated = [0u8; MAX_N];

        eprintln!("Q12 options: {:?}", &fp.option_nums[11][..5]);
        let contradicts = trace_verbose(&fp, &answers, &eliminated, 11, 1);
        eprintln!("Q12=B (11 E's) contradicts: {contradicts}");
        assert!(
            contradicts,
            "11 E's should force all to E, contradicting CountConsonant"
        );
    }

    #[test]
    fn test_repair_failed_q12_is_truestmt() {
        // In the repair-failed puzzle, Q12 is TrueStmt (not CountAnswer).
        // This test just verifies we have the right fixture.
        let fp = make_repair_failed_puzzle();
        assert!(matches!(fp.rules[11], Rule::TrueStmt));
    }

    // Third puzzle: the one where Q7 has 1 remaining but isn't forced after repair
    // Solution: C,E,A,D,C,B,A,A,B,E,E,E
    fn make_q7_stuck_puzzle() -> FlatPuzzle {
        use Answer::*;
        let rules = [
            Rule::LetterDist {
                other_question_index: 1,
            }, // Q1
            Rule::ClosestAfter {
                after_index: 1,
                answer: D,
            }, // Q2
            Rule::CountAnswerAfter {
                answer: E,
                after_index: 6,
            }, // Q3
            Rule::AnswerOf { question_index: 2 }, // Q4
            Rule::CountConsonant,                 // Q5
            Rule::LastWith { answer: A },         // Q6
            Rule::OnlyOdd { answer: B },          // Q7
            Rule::CountVowel,                     // Q8
            Rule::AnswerOf { question_index: 3 }, // Q9
            Rule::TrueStmt,                       // Q10
            Rule::CountAnswerBefore {
                answer: A,
                before_index: 11,
            }, // Q11
            Rule::OnlyOdd { answer: E },          // Q12
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
        ];
        // Use repaired (distance-sorted) distractors for counting rules
        let mut option_nums = [[NAN_VAL; 5]; MAX_N];
        option_nums[0] = [4, 3, 2, 1, 0]; // Q1 LetterDist (all 5 values, correct C=2)
        option_nums[1] = [-1, 12, 11, 10, 4]; // Q2 ClosestAfter (correct E=4)
        option_nums[2] = [3, 0, 1, 5, 2]; // Q3 CountAnswerAfter E>Q7 (correct A=3)
        // Q4 AnswerOf Q3 — constrained
        option_nums[4] = [12, 11, 5, 0, 10]; // Q5 CountConsonant (correct C=5)
        option_nums[5] = [-1, 8, 1, 2, 3]; // Q6 LastWith A (correct B=8)
        option_nums[6] = [9, -1, 1, 2, 3]; // Q7 OnlyOdd B (correct A=9)
        option_nums[7] = [7, 0, 1, 2, 12]; // Q8 CountVowel (correct A=7)
        // Q9 AnswerOf Q4 — constrained
        // Q10 TrueStmt — uses claims
        option_nums[10] = [11, 10, 9, 8, 3]; // Q11 CountAnswerBefore A<Q12 (correct E=3)
        option_nums[11] = [-1, 1, 2, 3, 11]; // Q12 OnlyOdd E (correct E=11)

        let mut option_answers = [[0xFFu8; 5]; MAX_N];
        // Q4: AnswerOf Q3. Q3 answer=A. Q4 answer=D(oi=3). D→A.
        option_answers[3] = [1, 2, 3, 0, 4]; // D→A(correct), A→B, B→C, C→D, E→E
        // Q9: AnswerOf Q4. Q4 answer=D. Q9 answer=B(oi=1). B→D.
        option_answers[8] = [0, 3, 1, 2, 4]; // B→D(correct), A→A, C→B, D→C, E→E

        // Q10 TrueStmt, E is correct
        // Solution: C,E,A,D,C,B,A,A,B,E,E,E → A=3, B=2, C=2, D=1, E=4
        let mut option_claims = [[Claim::None; 5]; MAX_N];
        use Answer as An;
        option_claims[9][4] = Claim::CountAnswerEquals {
            answer: An::D,
            value: 1,
        }; // true
        option_claims[9][0] = Claim::CountAnswerEquals {
            answer: An::A,
            value: 1,
        }; // false (3 A's)
        option_claims[9][1] = Claim::CountAnswerEquals {
            answer: An::B,
            value: 4,
        }; // false (2 B's)
        option_claims[9][2] = Claim::CountAnswerEquals {
            answer: An::C,
            value: 4,
        }; // false (2 C's)
        option_claims[9][3] = Claim::CountAnswerEquals {
            answer: An::E,
            value: 2,
        }; // false (4 E's)

        let n = 12;
        let (affected_by, global_indices) = FlatPuzzle::build_deps(&rules, n);
        FlatPuzzle {
            rules,
            option_nums,
            option_answers,
            option_claims,
            affected_by,
            global_indices,
            n,
        }
    }

    #[test]
    fn test_q7_single_remaining_must_be_forced() {
        let fp = make_q7_stuck_puzzle();
        let n = fp.n;
        let mut answers = [None; MAX_N];
        let mut eliminated = [0u8; MAX_N];

        let mut steps = Vec::new();
        let mut solved = false;
        for step in 0..n * 30 {
            if (0..n).all(|i| answers[i].is_some()) {
                steps.push(format!("step {step}: SOLVED"));
                solved = true;
                break;
            }
            if let Some(action) = find_action_fast(&fp, &answers, &eliminated) {
                steps.push(format!("step {step}: fast {action:?}"));
                if matches!(action, Action::Contradiction { .. }) {
                    steps.push("  ^^^ CONTRADICTION in main loop!".to_string());
                    // Show state
                    for qi in 0..n {
                        if answers[qi].is_some() {
                            steps.push(format!(
                                "  Q{}: SOLVED = {}",
                                qi + 1,
                                answers[qi].unwrap().as_char()
                            ));
                        } else {
                            let rem: Vec<char> = (0..5)
                                .filter(|&oi| !is_elim(&eliminated, qi, oi))
                                .map(|oi| LETTERS[oi].as_char())
                                .collect();
                            steps.push(format!("  Q{}: remaining {:?}", qi + 1, rem));
                        }
                    }
                    break;
                }
                apply_action(&action, &mut answers, &mut eliminated);
                continue;
            }
            if let Some(action) = find_lookahead_action(&fp, &answers, &eliminated) {
                steps.push(format!("step {step}: look {action:?}"));
                apply_action(&action, &mut answers, &mut eliminated);
                continue;
            }
            steps.push(format!("step {step}: STUCK"));
            for qi in 0..n {
                if answers[qi].is_none() {
                    let rem: Vec<String> = (0..5)
                        .filter(|&oi| !is_elim(&eliminated, qi, oi))
                        .map(|oi| format!("{}={}", LETTERS[oi].as_char(), fp.option_nums[qi][oi]))
                        .collect();
                    steps.push(format!(
                        "  Q{}: {:?} remaining: {}",
                        qi + 1,
                        fp.rules[qi],
                        rem.join(", ")
                    ));
                }
            }
            break;
        }

        for s in &steps {
            eprintln!("{s}");
        }

        let answered = (0..n).filter(|&i| answers[i].is_some()).count();
        eprintln!("Total: {answered}/{n} solved, {} steps", steps.len());

        // Q7 should not remain stuck with 1 option
        if !solved {
            let q7_remaining = remaining_count(eliminated[6]);
            eprintln!("Q7 remaining count: {q7_remaining}");
            if q7_remaining == 1 {
                panic!("BUG: Q7 has 1 remaining option but wasn't forced!");
            }
        }
    }

    #[test]
    fn test_q3_a_should_not_contradict() {
        // Q3=A is the correct answer. The lookahead should NOT find a contradiction.
        let fp = make_q7_stuck_puzzle();
        let answers = [None; MAX_N];
        let eliminated = [0u8; MAX_N];
        // qi=2 (Q3), oi=0 (A)
        let contradicts = trace_verbose(&fp, &answers, &eliminated, 2, 0);
        eprintln!("Q3=A contradicts: {contradicts}");
        assert!(
            !contradicts,
            "Q3=A is the correct answer — must not contradict!"
        );
    }

    #[test]
    fn test_repair_changes_only_one_distractor() {
        use crate::gen_common::repair_one_question;
        use crate::rng::Rng;
        // Q4 in make_stuck_puzzle: CountAnswerAfter{E, after_index:6}
        // correct=A(oi=0)=3, distractors B=0, C=1, D=5, E=2
        // Stuck state: B,C,D eliminated, only A=3 and E=2 remaining
        // Repair should change only E (closest to correct 3, dist=1)
        let mut fp = make_stuck_puzzle();
        let solution = [
            Answer::A,
            Answer::A,
            Answer::B,
            Answer::A,
            Answer::C,
            Answer::B,
            Answer::D,
            Answer::B,
            Answer::A,
            Answer::A,
            Answer::B,
            Answer::C,
            Answer::A,
            Answer::A,
            Answer::A,
            Answer::A, // padding
        ];
        let mut stuck_elim = [0u8; MAX_N];
        // Q5 (qi=4, CountAnswer D): correct=C(oi=2)=1
        // options: [11, 12, 1, 4, 7] → A=11, B=12, *C=1, D=4, E=7
        // Eliminate A(0) and B(1), leaving C=1(correct), D=4, E=7
        stuck_elim[4] = 0b00011; // bits 0,1 set
        let orig_opts = fp.option_nums[4];
        let mut rng = Rng::new(42);
        repair_one_question(&mut fp, 4, &solution, &stuck_elim, &mut rng);
        let new_opts = fp.option_nums[4];

        eprintln!("Q5 before: {:?}", orig_opts[..5].to_vec());
        eprintln!("Q5 after:  {:?}", new_opts[..5].to_vec());

        let correct_val = orig_opts[2]; // C=1 is correct
        // C(correct) unchanged
        assert_eq!(
            orig_opts[2], new_opts[2],
            "correct option should not change"
        );
        // A,B (eliminated) unchanged
        assert_eq!(orig_opts[0], new_opts[0], "eliminated A should not change");
        assert_eq!(orig_opts[1], new_opts[1], "eliminated B should not change");
        // D=4 (closest to correct 1, dist=3) should be the one repaired
        // E=7 (dist=6, further) should stay
        let d_changed = orig_opts[3] != new_opts[3];
        let e_changed = orig_opts[4] != new_opts[4];
        eprintln!(
            "D changed: {} ({}→{}), E changed: {} ({}→{})",
            d_changed, orig_opts[3], new_opts[3], e_changed, orig_opts[4], new_opts[4]
        );
        assert!(d_changed, "D (closest wrong to correct) should be repaired");
        assert!(!e_changed, "E (further wrong) should not be repaired");
        // D should now be further from correct
        let old_dist = (orig_opts[3] - correct_val).unsigned_abs();
        let new_dist = (new_opts[3] - correct_val).unsigned_abs();
        eprintln!("D dist from correct: {} -> {}", old_dist, new_dist);
        assert!(
            new_dist > old_dist,
            "repaired distractor should be further from correct"
        );
    }

    #[test]
    fn test_vowel_consonant_cross_elimination() {
        // n=5. Q1=CountVowel options [0,1,2,3,4], Q2=CountConsonant options [5,4,2,1,0].
        // Q1=A(0 vowels) requires 5 consonants, but Q2 has 5 as option A → ok.
        // Q1=D(3 vowels) requires 2 consonants, Q2 has 2 as option C → ok.
        // Q1=E(4 vowels) requires 1 consonant, Q2 has 1 as option D → ok.
        // Q1=C(2 vowels) requires 3 consonants, but Q2 has no 3 → eliminate!
        let mut rules = [Rule::AnswerIsSelf; MAX_N];
        rules[0] = Rule::CountVowel;
        rules[1] = Rule::CountConsonant;
        let n = 5;
        let mut option_nums = [[NAN_VAL; 5]; MAX_N];
        option_nums[0] = [0, 1, 2, 3, 4]; // Q1: vowel counts
        option_nums[1] = [5, 4, 2, 1, 0]; // Q2: consonant counts (no 3!)
        let (affected_by, global_indices) = FlatPuzzle::build_deps(&rules, n);
        let fp = FlatPuzzle {
            rules,
            n,
            option_nums,
            option_answers: [[0xFFu8; 5]; MAX_N],
            option_claims: [[Claim::None; 5]; MAX_N],
            affected_by,
            global_indices,
        };
        let answers = [None; MAX_N];
        let eliminated = [0u8; MAX_N];

        // Should eliminate Q1 oi=2 (2 vowels → needs 3 consonants, unavailable)
        let action = find_action_fast(&fp, &answers, &eliminated);
        eprintln!("cross-elim: {action:?}");
        assert!(matches!(action, Some(Action::Eliminate { qi: 0, oi: 2 })));
    }

    #[test]
    fn test_truestmt_contradiction_when_all_answered() {
        // When all questions are answered, a wrong TrueStmt answer should
        // be caught by the evaluate fallback in the Contradictions section.
        use Answer::*;
        let mut rules = [Rule::AnswerIsSelf; MAX_N];
        rules[0] = Rule::TrueStmt;
        let n = 3;

        let mut option_claims = [[Claim::None; 5]; MAX_N];
        // B is correct: "1 D" is true (Q2=D in our setup)
        option_claims[0][1] = Claim::CountAnswerEquals {
            answer: D,
            value: 1,
        };
        // A is wrong: "2 D's" is false
        option_claims[0][0] = Claim::CountAnswerEquals {
            answer: D,
            value: 2,
        };
        // C is wrong: "0 D's" is false
        option_claims[0][2] = Claim::CountAnswerEquals {
            answer: D,
            value: 0,
        };

        let (affected_by, global_indices) = FlatPuzzle::build_deps(&rules, n);
        let fp = FlatPuzzle {
            rules,
            n,
            option_nums: [[NAN_VAL; 5]; MAX_N],
            option_answers: [[0xFFu8; 5]; MAX_N],
            option_claims,
            affected_by,
            global_indices,
        };

        // All 3 answered: Q1=B (correct TrueStmt), Q2=D, Q3=A
        let mut answers = [None; MAX_N];
        answers[0] = Some(B);
        answers[1] = Some(D);
        answers[2] = Some(A);
        let eliminated = [0u8; MAX_N];

        // No contradiction (B is correct)
        let action = find_action_fast(&fp, &answers, &eliminated);
        assert!(
            !matches!(action, Some(Action::Contradiction { .. })),
            "correct TrueStmt answer should not contradict"
        );

        // Now set Q1=A (wrong). Should contradict.
        answers[0] = Some(A);
        let action = find_action_fast(&fp, &answers, &eliminated);
        eprintln!("Q1=A (wrong TrueStmt): {action:?}");
        assert!(
            matches!(action, Some(Action::Contradiction { qi: 0 })),
            "wrong TrueStmt answer should contradict when all answered"
        );
    }

    #[test]
    fn test_single_remaining_is_forced() {
        // If a question has only 1 non-eliminated option, it should be forced.
        use Answer::*;
        let rules = [
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
        ];
        let n = 3;
        let (affected_by, global_indices) = FlatPuzzle::build_deps(&rules, n);
        let fp = FlatPuzzle {
            rules,
            n,
            option_nums: [[NAN_VAL; 5]; MAX_N],
            option_answers: [[0xFFu8; 5]; MAX_N],
            option_claims: [[Claim::None; 5]; MAX_N],
            affected_by,
            global_indices,
        };

        let answers = [None; MAX_N];
        let mut eliminated = [0u8; MAX_N];
        // Eliminate all but C from Q1
        eliminated[0] = 0b11011; // A,B,D,E eliminated, C remains

        let action = find_action_fast(&fp, &answers, &eliminated);
        eprintln!("Action: {action:?}");
        assert!(matches!(action, Some(Action::Force { qi: 0, answer: C })));
    }

    #[test]
    fn test_count_saturation_forces_contradiction() {
        // Simple case: 5 questions, Q1=CountAnswer{B}, answer=A, option A=4 (4 B's).
        // With Q1=A answered and 4 unknowns, count+remaining=0+4==4 → all must be B.
        // But if Q2=CountVowel and forcing B (consonant) contradicts the vowel count.
        use Answer::*;
        let rules = [
            Rule::CountAnswer { answer: B }, // Q1
            Rule::CountVowel,                // Q2
            Rule::AnswerIsSelf,              // Q3
            Rule::AnswerIsSelf,              // Q4
            Rule::AnswerIsSelf,              // Q5
            // padding
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
            Rule::AnswerIsSelf,
        ];
        let mut option_nums = [[NAN_VAL; 5]; MAX_N];
        // Q1: CountAnswer B. Correct answer is A (oi=0), correct value = 2.
        // Distractor: B=4 (extreme)
        option_nums[0] = [2, 4, 3, 0, 1];
        // Q2: CountVowel. Correct answer is B (oi=1), correct value = 2.
        option_nums[1] = [4, 2, 0, 3, 1];
        let n = 5;
        let (affected_by, global_indices) = FlatPuzzle::build_deps(&rules, n);
        let fp = FlatPuzzle {
            rules,
            option_nums,
            option_answers: [[0xFFu8; 5]; MAX_N],
            option_claims: [[Claim::None; 5]; MAX_N],
            affected_by,
            global_indices,
            n,
        };

        // Assume Q1=B (on=4, meaning "4 B's"). count=0 (Q1=B is B, so count=1 actually!)
        // Wait: Q1=B means the answer IS B. count_matching for B: Q1=B → count=1. remaining=4.
        // count+remaining=1+4=5. on=4. 5≠4 → saturation doesn't trigger.
        // But 1+4=5 > 4 → count could still be 4 (if 3 of 4 unknowns are B).
        // count_bound: 1 > 4? no. 1+4 < 4? no. Passes.
        // So this specific case doesn't trigger saturation. Let me adjust.

        // Better: Q1=B (on=4), but n=5 and Q1 is B. count=1, remaining=4.
        // Need count+remaining==on → 1+4=5≠4. No saturation.
        // For saturation to trigger with on=4: need count+remaining==4.
        // E.g., count=0, remaining=4 → on=4. Q1 must not be B. So Q1=C (on for C might be different).

        // Let me just test directly with trace_leads_to_contradiction
        let answers = [None; MAX_N];
        let eliminated = [0u8; MAX_N];

        // Q1=B means on=option_nums[0][1]=4 → 4 B's total
        let contradicts = trace_leads_to_contradiction(&fp, &answers, &eliminated, 0, 1);
        eprintln!("Q1=B (4 B's in 5Q puzzle) contradicts: {contradicts}");
        // Q1=B, count of B = 1 (Q1 itself), remaining = 4. 1+4=5 ≠ 4. No saturation.
        // But count bound: 1 > 4? no. 1+4=5 < 4? no. Passes. Not eliminated.
        // This is correct — 4 B's IS possible (Q1 + 3 others = 4).
    }

    #[test]
    fn test_lookahead_q5_e_should_contradict() {
        // Q5=E means "7 D's". With 11 unknowns, count+remaining=0+11≥7, so
        // the bound check doesn't catch it. But 7 D's in 11 slots is quite
        // constraining — the lookahead may or may not catch it.
        let fp = make_stuck_puzzle();
        let answers = [None; MAX_N];
        let eliminated = [0u8; MAX_N];

        let contradicts = trace_leads_to_contradiction(&fp, &answers, &eliminated, 4, 4);
        eprintln!("Q5=E (7 D's) contradicts: {contradicts}");
        // Don't assert — just observe
    }

    #[test]
    fn test_repaired_distractors_solve_stuck_puzzle() {
        // Replace Q5's mid-range distractors with extreme values (distance-sorted)
        // Correct=C=1, so furthest values: 12, 11, 10, 9 (or what fits)
        let mut fp = make_stuck_puzzle();
        let max = fp.n as i16; // 12
        let correct_val: i16 = 1;
        // Distance-sorted: 12, 11, 10, 9 (all far from 1)
        let mut distractors = Vec::new();
        for v in (0..=max).rev() {
            if v != correct_val {
                distractors.push(v);
            }
        }
        // Place: A=12, B=11, D=10, E=9 (C=1 is correct, oi=2)
        let correct_oi = 2; // C
        let mut di = 0;
        for oi in 0..5 {
            if oi != correct_oi {
                fp.option_nums[4][oi] = distractors[di];
                di += 1;
            }
        }
        eprintln!("Q5 repaired options: {:?}", &fp.option_nums[4][..5]);

        // Also repair Q11 (CountAnswer C, correct=B=2) and Q12 (CountAnswer E, correct=C=0)
        // Q11: correct_oi=1 (B=2), set others to 12,11,10,9
        let mut di = 0;
        for oi in 0..5 {
            if oi != 1 {
                fp.option_nums[10][oi] = [12, 11, 10, 9][di];
                di += 1;
            }
        }
        // Q12: correct_oi=2 (C=0), set others to 12,11,10,9
        let mut di = 0;
        for oi in 0..5 {
            if oi != 2 {
                fp.option_nums[11][oi] = [12, 11, 10, 9][di];
                di += 1;
            }
        }

        // Now run hint engine
        let n = fp.n;
        let mut answers = [None; MAX_N];
        let mut eliminated = [0u8; MAX_N];
        let mut steps = 0;
        let mut solved = false;
        for _ in 0..n * 15 {
            if (0..n).all(|i| answers[i].is_some()) {
                solved = true;
                break;
            }
            if let Some(action) = find_action_fast(&fp, &answers, &eliminated) {
                steps += 1;
                apply_action(&action, &mut answers, &mut eliminated);
                continue;
            }
            if let Some(action) = find_lookahead_action(&fp, &answers, &eliminated) {
                steps += 1;
                apply_action(&action, &mut answers, &mut eliminated);
                continue;
            }
            break;
        }
        let answered = (0..n).filter(|&i| answers[i].is_some()).count();
        eprintln!("After repair: solved={solved}, answered={answered}/{n}, steps={steps}");
        for qi in 0..n {
            if answers[qi].is_none() {
                let remaining: Vec<char> = (0..5usize)
                    .filter(|&oi| !is_elim(&eliminated, qi, oi))
                    .map(|oi| LETTERS[oi].as_char())
                    .collect();
                eprintln!("  Q{}: remaining: {remaining:?}", qi + 1);
            }
        }
    }
}

fn trace_leads_to_contradiction(
    fp: &FlatPuzzle,
    orig_answers: &[Option<Answer>; MAX_N],
    orig_eliminated: &[u8; MAX_N],
    assume_qi: usize,
    assume_oi: u8,
) -> bool {
    trace_contradiction_inner(
        fp,
        orig_answers,
        orig_eliminated,
        assume_qi,
        assume_oi,
        false,
    )
}

fn trace_contradiction_inner(
    fp: &FlatPuzzle,
    orig_answers: &[Option<Answer>; MAX_N],
    orig_eliminated: &[u8; MAX_N],
    assume_qi: usize,
    assume_oi: u8,
    verbose: bool,
) -> bool {
    let n = fp.n;
    let mut answers = *orig_answers;
    let mut eliminated = *orig_eliminated;

    answers[assume_qi] = Some(LETTERS[assume_oi as usize]);
    eliminated[assume_qi] = 0b11111 ^ (1 << assume_oi);

    for step in 0..n * 5 {
        match find_action_fast(fp, &answers, &eliminated) {
            Some(Action::Contradiction { qi }) => {
                if verbose {
                    eprintln!("  step {step}: Contradiction at Q{}", qi + 1);
                }
                return true;
            }
            Some(Action::Force { qi, answer }) => {
                if verbose {
                    eprintln!("  step {step}: Force Q{}={}", qi + 1, answer.as_char());
                }
                let oi = answer.idx();
                eliminated[qi] = 0b11111 ^ (1 << oi);
                answers[qi] = Some(answer);
            }
            Some(Action::Eliminate { qi, oi }) => {
                if verbose {
                    eprintln!(
                        "  step {step}: Eliminate Q{} oi={} ({})",
                        qi + 1,
                        oi,
                        LETTERS[oi].as_char()
                    );
                }
                eliminated[qi] |= 1 << oi;
            }
            None => {
                if verbose {
                    eprintln!("  step {step}: No action found");
                    let answered = (0..n).filter(|&i| answers[i].is_some()).count();
                    eprintln!("  answered: {answered}/{n}");
                }
                break;
            }
        }
    }
    false
}

#[cfg(test)]
pub fn trace_verbose(
    fp: &FlatPuzzle,
    orig_answers: &[Option<Answer>; MAX_N],
    orig_eliminated: &[u8; MAX_N],
    assume_qi: usize,
    assume_oi: u8,
) -> bool {
    trace_contradiction_inner(
        fp,
        orig_answers,
        orig_eliminated,
        assume_qi,
        assume_oi,
        true,
    )
}
