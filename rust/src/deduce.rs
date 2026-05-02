use crate::types::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeduceRule {
    All,
    CountSaturation,
    ForcedValues,
    VowelConsonantCross,
    Eliminations,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeduceAction {
    Force { qi: usize, answer: Answer },
    Eliminate { qi: usize, oi: usize },
}

#[derive(Clone, Copy, Debug)]
pub struct DeduceResult {
    pub action: DeduceAction,
    #[allow(dead_code)] // used by tests
    pub rule: DeduceRule,
}

// ── Helpers ──

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
            CountPred::IsAnswer(t) => a == t,
            CountPred::IsVowel => a.is_vowel(),
            CountPred::IsConsonant => !a.is_vowel(),
        }
    }
    fn mask(self) -> u8 {
        match self {
            CountPred::IsAnswer(t) => 1u8 << t.idx(),
            CountPred::IsVowel => 0b10001,
            CountPred::IsConsonant => 0b01110,
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
    let mask = pred.mask();
    let mut count: i16 = 0;
    let mut remaining: i16 = 0;
    for i in from..to {
        match answers[i] {
            Some(a) if pred.matches(a) => count += 1,
            None if eliminated[i] & mask != mask => remaining += 1,
            _ => {}
        }
    }
    CountResult { count, remaining }
}

fn count_pred(r: &QuestionType) -> Option<CountPred> {
    match *r {
        QuestionType::CountAnswer { answer }
        | QuestionType::CountAnswerBefore { answer, .. }
        | QuestionType::CountAnswerAfter { answer, .. } => Some(CountPred::IsAnswer(answer)),
        QuestionType::CountVowel => Some(CountPred::IsVowel),
        QuestionType::CountConsonant => Some(CountPred::IsConsonant),
        _ => None,
    }
}

fn count_range(r: &QuestionType, n: usize) -> (usize, usize) {
    match *r {
        QuestionType::CountAnswerBefore { before_index, .. } => (0, before_index as usize),
        QuestionType::CountAnswerAfter { after_index, .. } => (after_index as usize + 1, n),
        _ => (0, n),
    }
}

fn can_still_match(pred: CountPred, eliminated: u8) -> bool {
    eliminated & pred.mask() != pred.mask()
}

fn result(action: DeduceAction, rule: DeduceRule) -> Option<DeduceResult> {
    Some(DeduceResult { action, rule })
}

// ── Main functions ──

pub fn deduce(
    fp: &FlatPuzzle,
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
) -> Option<DeduceResult> {
    deduce_with_rule(fp, answers, eliminated, DeduceRule::All)
}

#[inline(always)]
pub fn deduce_with_rule(
    fp: &FlatPuzzle,
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    rule: DeduceRule,
) -> Option<DeduceResult> {
    let n = fp.n;

    // ── Count saturation ──
    if rule == DeduceRule::All || rule == DeduceRule::CountSaturation {
        for qi in 0..n {
            let Some(a) = answers[qi] else { continue };
            let r = &fp.question_types[qi];
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
                                return result(
                                    DeduceAction::Eliminate { qi: j, oi },
                                    DeduceRule::CountSaturation,
                                );
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
                                return result(
                                    DeduceAction::Eliminate { qi: j, oi },
                                    DeduceRule::CountSaturation,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // ── Forced values ──
    if rule == DeduceRule::All || rule == DeduceRule::ForcedValues {
        for qi in 0..n {
            if answers[qi].is_some() {
                continue;
            }
            let r = &fp.question_types[qi];

            if remaining_count(eliminated[qi]) == 1 {
                let oi = (!eliminated[qi] & 0b11111).trailing_zeros();
                return result(
                    DeduceAction::Force {
                        qi,
                        answer: LETTERS[oi as usize],
                    },
                    DeduceRule::ForcedValues,
                );
            }

            if let QuestionType::AnswerOf { question_index } = *r
                && let Some(target) = answers[question_index as usize]
            {
                for oi in 0..5usize {
                    if fp.option_answers[qi][oi] == target as u8 {
                        return result(
                            DeduceAction::Force {
                                qi,
                                answer: LETTERS[oi],
                            },
                            DeduceRule::ForcedValues,
                        );
                    }
                }
            }

            for other in 0..n {
                let Some(other_ans) = answers[other] else {
                    continue;
                };
                if let QuestionType::AnswerOf { question_index } = fp.question_types[other]
                    && question_index as usize == qi
                {
                    let implied = fp.option_answers[other][other_ans.idx()];
                    if implied <= 4 {
                        return result(
                            DeduceAction::Force {
                                qi,
                                answer: LETTERS[implied as usize],
                            },
                            DeduceRule::ForcedValues,
                        );
                    }
                }
                if let QuestionType::SameAs = fp.question_types[other] {
                    let target_q = fp.option_nums[other][other_ans.idx()];
                    if target_q >= 0 && target_q as usize == qi {
                        return result(
                            DeduceAction::Force {
                                qi,
                                answer: other_ans,
                            },
                            DeduceRule::ForcedValues,
                        );
                    }
                }
            }

            if let QuestionType::LetterDist { question_index } = *r
                && let Some(other_ans) = answers[question_index as usize]
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
                    return result(
                        DeduceAction::Force {
                            qi,
                            answer: valid_letter,
                        },
                        DeduceRule::ForcedValues,
                    );
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
                            return result(
                                DeduceAction::Force {
                                    qi,
                                    answer: LETTERS[oi],
                                },
                                DeduceRule::ForcedValues,
                            );
                        }
                    }
                }
            }
        }
    }

    // ── Vowel/consonant cross-elimination ──
    if rule == DeduceRule::All || rule == DeduceRule::VowelConsonantCross {
        let mut vowel_qi = None;
        let mut consonant_qi = None;
        for i in 0..n {
            if answers[i].is_some() {
                continue;
            }
            match fp.question_types[i] {
                QuestionType::CountVowel => vowel_qi = Some(i),
                QuestionType::CountConsonant => consonant_qi = Some(i),
                _ => {}
            }
        }
        if let (Some(vq), Some(cq)) = (vowel_qi, consonant_qi) {
            let nn = n as i16;
            for oi in 0..5usize {
                if is_elim(eliminated, vq, oi) {
                    continue;
                }
                let v = fp.option_nums[vq][oi];
                if v == NAN_VAL {
                    continue;
                }
                let need = nn - v;
                let has = (0..5)
                    .any(|coi| !is_elim(eliminated, cq, coi) && fp.option_nums[cq][coi] == need);
                if !has {
                    return result(
                        DeduceAction::Eliminate { qi: vq, oi },
                        DeduceRule::VowelConsonantCross,
                    );
                }
            }
            for oi in 0..5usize {
                if is_elim(eliminated, cq, oi) {
                    continue;
                }
                let v = fp.option_nums[cq][oi];
                if v == NAN_VAL {
                    continue;
                }
                let need = nn - v;
                let has = (0..5)
                    .any(|voi| !is_elim(eliminated, vq, voi) && fp.option_nums[vq][voi] == need);
                if !has {
                    return result(
                        DeduceAction::Eliminate { qi: cq, oi },
                        DeduceRule::VowelConsonantCross,
                    );
                }
            }
        }
    }

    // ── Eliminations ──
    if rule == DeduceRule::All || rule == DeduceRule::Eliminations {
        for qi in 0..n {
            if answers[qi].is_some() {
                continue;
            }
            let r = &fp.question_types[qi];

            for oi in 0..5usize {
                if is_elim(eliminated, qi, oi) {
                    continue;
                }
                let on = fp.option_nums[qi][oi];

                let elim = match *r {
                    QuestionType::CountAnswer { answer }
                    | QuestionType::CountAnswerBefore { answer, .. }
                    | QuestionType::CountAnswerAfter { answer, .. }
                        if on != NAN_VAL =>
                    {
                        let (from, to) = count_range(r, n);
                        let cr = count_matching(
                            answers,
                            eliminated,
                            CountPred::IsAnswer(answer),
                            from,
                            to,
                        );
                        cr.count > on || cr.count + cr.remaining < on
                    }
                    QuestionType::CountVowel | QuestionType::CountConsonant if on != NAN_VAL => {
                        let pred = if matches!(*r, QuestionType::CountVowel) {
                            CountPred::IsVowel
                        } else {
                            CountPred::IsConsonant
                        };
                        let cr = count_matching(answers, eliminated, pred, 0, n);
                        cr.count > on || cr.count + cr.remaining < on
                    }
                    QuestionType::AnswerOf { question_index } => {
                        let ov = fp.option_answers[qi][oi];
                        if ov <= 4 {
                            if let Some(target) = answers[question_index as usize] {
                                target as u8 != ov
                            } else {
                                is_elim(eliminated, question_index as usize, ov as usize)
                            }
                        } else {
                            false
                        }
                    }
                    QuestionType::LetterDist { question_index } => {
                        if let Some(other) = answers[question_index as usize] {
                            let dist = (oi as i16 - other.idx() as i16).abs();
                            dist != on
                        } else if on != NAN_VAL {
                            !(0..5usize).any(|ti| {
                                !is_elim(eliminated, question_index as usize, ti)
                                    && (oi as i16 - ti as i16).abs() == on
                            })
                        } else {
                            false
                        }
                    }
                    QuestionType::ClosestAfter { answer, .. }
                    | QuestionType::FirstWith { answer } => {
                        let scan_start: usize = match *r {
                            QuestionType::ClosestAfter { after_index, .. } => {
                                after_index as usize + 1
                            }
                            _ => 0,
                        };
                        elim_first_in_range(answers, eliminated, answer, scan_start, n, on, qi, oi)
                    }
                    QuestionType::ClosestBefore { answer, .. }
                    | QuestionType::LastWith { answer } => {
                        let before_idx: usize = match *r {
                            QuestionType::ClosestBefore { before_index, .. } => {
                                before_index as usize
                            }
                            _ => n,
                        };
                        elim_last_in_range(answers, eliminated, answer, before_idx, on, qi, oi)
                    }
                    QuestionType::OnlyOdd { answer } | QuestionType::OnlyEven { answer } => {
                        let parity = match r {
                            QuestionType::OnlyOdd { .. } => 1,
                            _ => 0,
                        };
                        if on != NONE_VAL {
                            let pos = on as usize;
                            if (pos + 1) % 2 != parity {
                                true
                            } else if pos < n {
                                if let Some(pa) = answers[pos] {
                                    pa != answer
                                } else {
                                    is_elim(eliminated, pos, answer.idx())
                                }
                            } else {
                                false
                            }
                        } else {
                            (0..n).any(|i| (i + 1) % 2 == parity && answers[i] == Some(answer))
                        }
                    }
                    QuestionType::ConsecIdent => {
                        if on != NONE_VAL {
                            let pos = on as usize;
                            if pos + 1 >= n {
                                true
                            } else {
                                let possible_a = !eliminated[pos] & 0b11111u8;
                                let possible_b = !eliminated[pos + 1] & 0b11111u8;
                                possible_a & possible_b == 0
                            }
                        } else {
                            (0..n.saturating_sub(1)).any(|i| {
                                matches!(
                                    (answers[i], answers[i + 1]),
                                    (Some(a), Some(b)) if a == b
                                )
                            })
                        }
                    }
                    QuestionType::EqualCount { answer } if on != NONE_VAL => {
                        LETTERS[on as usize] == answer
                    }
                    QuestionType::PrevSame if on != NONE_VAL => {
                        let pos = on as usize;
                        if pos >= qi {
                            true
                        } else if is_elim(eliminated, pos, oi) {
                            true
                        } else {
                            ((pos + 1)..qi)
                                .rev()
                                .any(|j| answers[j] == Some(LETTERS[oi]))
                        }
                    }
                    QuestionType::NextSame if on != NONE_VAL => {
                        let pos = on as usize;
                        if pos <= qi || pos >= n {
                            true
                        } else if is_elim(eliminated, pos, oi) {
                            true
                        } else {
                            ((qi + 1)..pos).any(|j| answers[j] == Some(LETTERS[oi]))
                        }
                    }
                    QuestionType::OnlySame | QuestionType::SameAs if on != NONE_VAL => {
                        let pos = on as usize;
                        pos == qi || (pos < n && is_elim(eliminated, pos, oi))
                    }
                    _ => false,
                };

                if elim {
                    return result(DeduceAction::Eliminate { qi, oi }, DeduceRule::Eliminations);
                }
            }
        }
    }

    None
}

fn elim_first_in_range(
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    answer: Answer,
    scan_start: usize,
    n: usize,
    on: i16,
    qi: usize,
    oi: usize,
) -> bool {
    if on != NONE_VAL {
        let pos = on as usize;
        if (on as usize) < scan_start || pos >= n {
            return true;
        }
        if let Some(pa) = answers[pos] {
            if pa != answer {
                return true;
            }
        } else if is_elim(eliminated, pos, answer.idx()) {
            return true;
        }
        for j in scan_start..pos {
            if answers[j] == Some(answer) {
                return true;
            }
        }
        if LETTERS[oi] == answer && qi >= scan_start && qi < pos {
            return true;
        }
        false
    } else {
        (scan_start..n).any(|j| answers[j] == Some(answer))
    }
}

fn elim_last_in_range(
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    answer: Answer,
    before_idx: usize,
    on: i16,
    qi: usize,
    oi: usize,
) -> bool {
    if on != NONE_VAL {
        let pos = on as usize;
        if on < 0 || pos >= before_idx {
            return true;
        }
        if let Some(pa) = answers[pos] {
            if pa != answer {
                return true;
            }
        } else if is_elim(eliminated, pos, answer.idx()) {
            return true;
        }
        for j in ((pos + 1)..before_idx).rev() {
            if answers[j] == Some(answer) {
                return true;
            }
        }
        if LETTERS[oi] == answer && qi > pos && qi < before_idx {
            return true;
        }
        false
    } else {
        (0..before_idx).any(|j| answers[j] == Some(answer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn test_shared_deduce() {
        let json_str =
            std::fs::read_to_string("../tests/deduce.json").expect("can't read tests/deduce.json");
        let suite: Value = serde_json::from_str(&json_str).unwrap();
        let tests = suite["tests"].as_array().unwrap();

        let mut passed = 0;
        let mut failed = 0;

        for test in tests {
            if test.get("section").is_some() {
                continue;
            }
            let name = test["name"].as_str().unwrap();
            let states = test["state"].as_array().unwrap();
            let expect = test["expect"].as_str();
            let rule_filter = test.get("rule").and_then(|v| v.as_str());

            let fp = crate::parse_puzzle(&test["puzzle"]);
            let fp = match fp {
                Some(fp) => fp,
                None => {
                    eprintln!("SKIP: {name}: parse failed");
                    continue;
                }
            };

            let n = fp.n;
            let mut answers: [Option<Answer>; MAX_N] = [None; MAX_N];
            let mut eliminated = [0u8; MAX_N];
            for i in 0..n {
                let s = states[i].as_str().unwrap_or("");
                for ch in s.chars() {
                    if ch.is_ascii_uppercase() {
                        let oi = (ch as u8 - b'A') as usize;
                        answers[i] = Some(LETTERS[oi]);
                        eliminated[i] = 0b11111 ^ (1 << oi);
                    } else if ch.is_ascii_lowercase() {
                        let oi = (ch as u8 - b'a') as usize;
                        eliminated[i] |= 1 << oi;
                    }
                }
            }

            let dr = match rule_filter {
                Some("count_saturation") => {
                    deduce_with_rule(&fp, &answers, &eliminated, DeduceRule::CountSaturation)
                }
                Some("forced_values") => {
                    deduce_with_rule(&fp, &answers, &eliminated, DeduceRule::ForcedValues)
                }
                Some("vowel_consonant_cross") => {
                    deduce_with_rule(&fp, &answers, &eliminated, DeduceRule::VowelConsonantCross)
                }
                Some("eliminations") => {
                    deduce_with_rule(&fp, &answers, &eliminated, DeduceRule::Eliminations)
                }
                _ => deduce(&fp, &answers, &eliminated),
            };

            let got = match dr {
                Some(DeduceResult {
                    action: DeduceAction::Force { qi, answer },
                    ..
                }) => format!("{}{}", qi + 1, answer.as_char()),
                Some(DeduceResult {
                    action: DeduceAction::Eliminate { qi, oi },
                    ..
                }) => format!("{}{}", qi + 1, (b'a' + oi as u8) as char),
                None => "null".to_string(),
            };
            let expected = expect.unwrap_or("null");

            if got == expected {
                passed += 1;
            } else {
                failed += 1;
                eprintln!("FAIL: {name}");
                eprintln!("  expected: {expected}");
                eprintln!("  got:      {got}");
            }
        }

        eprintln!("{passed}/{} passed", passed + failed);
        assert_eq!(failed, 0, "{failed} test(s) failed");
    }
}
