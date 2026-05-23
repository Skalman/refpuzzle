use arrayvec::ArrayVec;

use crate::types::*;

macro_rules! deduce_rules {
    ($($variant:ident),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[allow(dead_code)]
        pub enum DeduceRule {
            All,
            $($variant),+
        }

        #[allow(dead_code)]
        pub const ALL_DEDUCE_RULES: &[DeduceRule] = &[
            $(DeduceRule::$variant),+
        ];

        #[allow(dead_code)]
        impl DeduceRule {
            pub fn from_str(s: &str) -> Option<DeduceRule> {
                match s {
                    "All" => Some(DeduceRule::All),
                    $(stringify!($variant) => Some(DeduceRule::$variant),)+
                    _ => None,
                }
            }

            pub fn to_str(self) -> &'static str {
                match self {
                    DeduceRule::All => "All",
                    $(DeduceRule::$variant => stringify!($variant)),+
                }
            }
        }
    }
}

deduce_rules! {
    CountSaturated,
    CountMustMatchForce,
    CountMustMatchElim,
    OnlyOptionLeft,
    AnswerOfForward,
    AnswerOfReverse,
    SameAsReverse,
    PrevNextOnlySameReverse,
    LetterDistForward,
    LetterDistReverseForce,
    LetterDistReverseElim,
    CountAllAnswered,
    MostCommonCountElim,
    PositionalRangeAnswered,
    PositionalRangeUnanswered,
    VowelCrossElim,
    ConsonantCrossElim,
    CountExceeded,
    CountImpossible,
    AnswerOfTargetRuledOut,
    LetterDistImpossible,
    LetterDistWrong,
    LetterDistNoMatch,
    FirstClosestAfterOutOfRange,
    FirstClosestAfterWrongAnswer,
    FirstClosestAfterRuledOut,
    FirstClosestAfterEarlierMatch,
    FirstClosestAfterSelfRef,
    FirstClosestAfterNoneMatch,
    LastClosestBeforeOutOfRange,
    LastClosestBeforeWrongAnswer,
    LastClosestBeforeRuledOut,
    LastClosestBeforeLaterMatch,
    LastClosestBeforeSelfRef,
    LastClosestBeforeNoneMatch,
    OnlyOddEvenWrongParity,
    OnlyOddEvenWrongAnswer,
    OnlyOddEvenRuledOut,
    OnlyOddEvenNoneMatch,
    ConsecIdentOutOfRange,
    ConsecIdentSelfRef,
    ConsecIdentNoCommon,
    ConsecIdentNonePair,
    EqualCountSelfRef,
    PrevSameNotBefore,
    PrevSameRuledOut,
    PrevSameCloser,
    NextSameNotAfter,
    NextSameRuledOut,
    NextSameCloser,
    OnlySameSelfRef,
    OnlySameRuledOut,
    UniqueAlreadyUsed,
    LeastCommonElim,
    LeastCommonForce,
    TrueStatementForward,
    OnlyOddEvenRangeElim,
    MostCommonElim,
    MostCommonForce,
    ConsecIdentReverse,
    TrueStatementSelfRef,
    TrueStatementClaimInvalid,
    TrueStatementClaimValid,
    ConsecIdentForwardForce,
    ConsecIdentForwardElim,
    ConsecIdentForwardBothForce,
    EqualCountRangeElim,
    OnlySameOtherMatch,
    PrevSameNoneMatch,
    NextSameNoneMatch,
    OnlySameNoneMatch,
    OnlySameNoneForward,
    SameAsNegative,
    SameAsWhichForward,
    SameAsWhichReverse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeduceAction {
    Force { qi: usize, answer: Answer },
    Eliminate { qi: usize, oi: usize },
    EliminateMulti { question_mask: u16, option_mask: u8 },
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
    guaranteed: i16,
    possible: i16,
}

impl CountResult {
    fn min(&self) -> i16 {
        self.count + self.guaranteed
    }
    fn max(&self) -> i16 {
        self.count + self.guaranteed + self.possible
    }
}

#[derive(Clone, Copy)]
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
    let mut guaranteed: i16 = 0;
    let mut possible: i16 = 0;
    for i in from..to {
        match answers[i] {
            Some(a) if pred.matches(a) => count += 1,
            None => {
                let remaining_bits = !eliminated[i] & 0b11111u8;
                if remaining_bits == 0 {
                    continue;
                }
                let matching = remaining_bits & mask;
                let non_matching = remaining_bits & (!mask & 0b11111u8);
                if matching != 0 && non_matching == 0 {
                    guaranteed += 1;
                } else if matching != 0 {
                    possible += 1;
                }
            }
            _ => {}
        }
    }
    CountResult {
        count,
        guaranteed,
        possible,
    }
}

fn count_pred(t: &QuestionType) -> Option<CountPred> {
    match *t {
        QuestionType::CountAnswer { answer }
        | QuestionType::CountAnswerBefore { answer, .. }
        | QuestionType::CountAnswerAfter { answer, .. } => Some(CountPred::IsAnswer(answer)),
        QuestionType::CountVowel => Some(CountPred::IsVowel),
        QuestionType::CountConsonant => Some(CountPred::IsConsonant),
        _ => None,
    }
}

fn count_range(t: &QuestionType, n: usize) -> (usize, usize) {
    match *t {
        QuestionType::CountAnswerBefore { before_index, .. } => (0, before_index as usize),
        QuestionType::CountAnswerAfter { after_index, .. } => (after_index as usize + 1, n),
        _ => (0, n),
    }
}

fn can_still_match(pred: CountPred, eliminated: u8) -> bool {
    eliminated & pred.mask() != pred.mask()
}

pub type DeduceResults = ArrayVec<DeduceResult, 80>;

// ── Main functions ──

pub fn deduce(fp: &FlatPuzzle, state: &State) -> DeduceResults {
    deduce_impl(fp, state, None, None, false)
}

pub fn deduce_fast(fp: &FlatPuzzle, state: &State) -> DeduceResults {
    deduce_impl(fp, state, None, None, true)
}

#[cfg(test)]
pub fn deduce_with_rule(fp: &FlatPuzzle, state: &State, rule: DeduceRule) -> DeduceResults {
    deduce_impl(fp, state, Some(rule), None, false)
}

#[cfg(test)]
pub fn deduce_with_rule_exclude(
    fp: &FlatPuzzle,
    state: &State,
    rule: DeduceRule,
    exclude: Option<DeduceRule>,
) -> DeduceResults {
    let rule_filter = if rule == DeduceRule::All {
        None
    } else {
        Some(rule)
    };
    deduce_impl(fp, state, rule_filter, exclude, false)
}

#[inline(always)]
fn deduce_impl(
    fp: &FlatPuzzle,
    state: &State,
    rule: Option<DeduceRule>,
    exclude: Option<DeduceRule>,
    fast: bool,
) -> DeduceResults {
    let n = fp.n;
    let answers = &state.answers;
    let eliminated = &state.eliminated;
    let run = |r: DeduceRule| (rule.is_none() || rule == Some(r)) && exclude != Some(r);
    let mut results = DeduceResults::new();
    let mut push = |action: DeduceAction, rule: DeduceRule| {
        results.push(DeduceResult { action, rule });
    };

    // ── Count saturation ──
    for qi in 0..n {
        let Some(a) = answers[qi] else { continue };
        let t = &fp.question_types[qi];
        let Some(pred) = count_pred(t) else { continue };
        let on = fp.option_nums[qi][a.idx()];
        if on == NAN_VAL || on < 0 {
            continue;
        }
        let (from, to) = count_range(t, n);
        let cr = count_matching(answers, eliminated, pred, from, to);

        if run(DeduceRule::CountSaturated) && cr.min() == on && cr.possible > 0 {
            let mask = pred.mask();
            for j in from..to {
                if answers[j].is_none() {
                    let remaining_bits = !eliminated[j] & 0b11111u8;
                    if remaining_bits & (!mask & 0b11111u8) == 0 {
                        continue;
                    }
                    for oi in 0..5usize {
                        if !is_elim(eliminated, j, oi) && pred.matches(LETTERS[oi]) {
                            push(
                                DeduceAction::Eliminate { qi: j, oi },
                                DeduceRule::CountSaturated,
                            );
                        }
                    }
                }
            }
        }

        if cr.max() == on && cr.possible > 0 {
            if run(DeduceRule::CountMustMatchForce) && cr.possible == 1 {
                for j in from..to {
                    if answers[j].is_none() && can_still_match(pred, eliminated[j]) {
                        let mut match_count = 0;
                        let mut match_oi = 0;
                        for oi in 0..5usize {
                            if !is_elim(eliminated, j, oi) && pred.matches(LETTERS[oi]) {
                                match_count += 1;
                                match_oi = oi;
                            }
                        }
                        if match_count == 1 {
                            push(
                                DeduceAction::Force {
                                    qi: j,
                                    answer: LETTERS[match_oi],
                                },
                                DeduceRule::CountMustMatchForce,
                            );
                        }
                    }
                }
            }

            if run(DeduceRule::CountMustMatchElim) {
                for j in from..to {
                    if answers[j].is_none() && can_still_match(pred, eliminated[j]) {
                        for oi in 0..5usize {
                            if !is_elim(eliminated, j, oi) && !pred.matches(LETTERS[oi]) {
                                push(
                                    DeduceAction::Eliminate { qi: j, oi },
                                    DeduceRule::CountMustMatchElim,
                                );
                            }
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
        let t = &fp.question_types[qi];

        if run(DeduceRule::OnlyOptionLeft) && remaining_count(eliminated[qi]) == 1 {
            let oi = (!eliminated[qi] & 0b11111).trailing_zeros();
            push(
                DeduceAction::Force {
                    qi,
                    answer: LETTERS[oi as usize],
                },
                DeduceRule::OnlyOptionLeft,
            );
        }

        if run(DeduceRule::AnswerOfForward)
            && let QuestionType::AnswerOf { question_index } = *t
            && let Some(target) = answers[question_index as usize]
        {
            let mut best: Option<usize> = None;
            for oi in 0..5usize {
                if fp.option_answers[qi][oi] == target as u8 {
                    if !is_elim(eliminated, qi, oi) {
                        best = Some(oi);
                        break;
                    }
                    if best.is_none() {
                        best = Some(oi);
                    }
                }
            }
            if let Some(oi) = best {
                push(
                    DeduceAction::Force {
                        qi,
                        answer: LETTERS[oi],
                    },
                    DeduceRule::AnswerOfForward,
                );
            }
        }

        for other in 0..n {
            let Some(other_ans) = answers[other] else {
                continue;
            };

            if run(DeduceRule::AnswerOfReverse)
                && let QuestionType::AnswerOf { question_index } = fp.question_types[other]
                && question_index as usize == qi
            {
                let implied = fp.option_answers[other][other_ans.idx()];
                if implied <= 4 {
                    push(
                        DeduceAction::Force {
                            qi,
                            answer: LETTERS[implied as usize],
                        },
                        DeduceRule::AnswerOfReverse,
                    );
                }
            }

            if run(DeduceRule::SameAsReverse)
                && let QuestionType::SameAs = fp.question_types[other]
            {
                let target_q = fp.option_nums[other][other_ans.idx()];
                if target_q >= 0 && target_q as usize == qi {
                    push(
                        DeduceAction::Force {
                            qi,
                            answer: other_ans,
                        },
                        DeduceRule::SameAsReverse,
                    );
                }
            }

            if run(DeduceRule::PrevNextOnlySameReverse)
                && matches!(
                    fp.question_types[other],
                    QuestionType::PrevSame | QuestionType::NextSame | QuestionType::OnlySame
                )
            {
                let target_q = fp.option_nums[other][other_ans.idx()];
                if target_q >= 0 && target_q as usize == qi {
                    push(
                        DeduceAction::Force {
                            qi,
                            answer: other_ans,
                        },
                        DeduceRule::PrevNextOnlySameReverse,
                    );
                }
            }
        }

        if run(DeduceRule::LetterDistForward)
            && let QuestionType::LetterDist { question_index } = *t
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
                push(
                    DeduceAction::Force {
                        qi,
                        answer: valid_letter,
                    },
                    DeduceRule::LetterDistForward,
                );
            }
        }

        // Reverse LetterDist: other questions' LetterDist rules constrain qi
        for src in 0..n {
            if src == qi {
                continue;
            }
            if let QuestionType::LetterDist { question_index } = fp.question_types[src] {
                if question_index as usize != qi {
                    continue;
                }
                let mut elim_mask = 0u8;
                if let Some(src_ans) = answers[src] {
                    let dist = fp.option_nums[src][src_ans.idx()];
                    if dist == NAN_VAL {
                        continue;
                    }
                    let mut valid_count = 0u8;
                    let mut valid_oi = 0usize;
                    for oi in 0..5usize {
                        if is_elim(eliminated, qi, oi) {
                            continue;
                        }
                        if (oi as i16 - src_ans.idx() as i16).abs() == dist {
                            valid_count += 1;
                            valid_oi = oi;
                        } else {
                            elim_mask |= 1 << oi;
                        }
                    }
                    if run(DeduceRule::LetterDistReverseForce) && valid_count == 1 && elim_mask != 0
                    {
                        push(
                            DeduceAction::Force {
                                qi,
                                answer: LETTERS[valid_oi],
                            },
                            DeduceRule::LetterDistReverseForce,
                        );
                    }
                    if run(DeduceRule::LetterDistReverseElim) && elim_mask != 0 && valid_count != 1
                    {
                        push(
                            DeduceAction::EliminateMulti {
                                question_mask: 1 << qi,
                                option_mask: elim_mask,
                            },
                            DeduceRule::LetterDistReverseElim,
                        );
                    }
                } else {
                    for oi in 0..5usize {
                        if is_elim(eliminated, qi, oi) {
                            continue;
                        }
                        let compatible = (0..5usize).any(|si| {
                            !is_elim(eliminated, src, si)
                                && fp.option_nums[src][si] != NAN_VAL
                                && (oi as i16 - si as i16).abs() == fp.option_nums[src][si]
                        });
                        if !compatible {
                            elim_mask |= 1 << oi;
                        }
                    }
                    if run(DeduceRule::LetterDistReverseElim) && elim_mask != 0 {
                        push(
                            DeduceAction::EliminateMulti {
                                question_mask: 1 << qi,
                                option_mask: elim_mask,
                            },
                            DeduceRule::LetterDistReverseElim,
                        );
                    }
                }
            }
        }

        if !fast
            && run(DeduceRule::CountAllAnswered)
            && let Some(pred) = count_pred(t)
        {
            let (from, to) = count_range(t, n);
            let cr = count_matching(answers, eliminated, pred, from, to);
            if cr.possible == 0 {
                let target = cr.min();
                let mut match_oi = None;
                let mut match_count = 0;
                for oi in 0..fp.option_count {
                    if is_elim(eliminated, qi, oi) {
                        continue;
                    }
                    if fp.option_nums[qi][oi] == target {
                        match_oi = Some(oi);
                        match_count += 1;
                    }
                }
                if match_count == 1
                    && let Some(oi) = match_oi
                {
                    push(
                        DeduceAction::Force {
                            qi,
                            answer: LETTERS[oi],
                        },
                        DeduceRule::CountAllAnswered,
                    );
                }
            }
        }
    }

    // ── Positional range elimination ──
    if run(DeduceRule::PositionalRangeAnswered) {
        for src in 0..n {
            let Some(src_ans) = answers[src] else {
                continue;
            };
            let t = &fp.question_types[src];
            let v = fp.option_nums[src][src_ans.idx()];
            if v < 0 || v == NAN_VAL {
                continue;
            }
            let v = v as usize;

            let (letter, range_start, range_end) = match *t {
                QuestionType::FirstWith { answer } => (answer, 0usize, v),
                QuestionType::ClosestAfter {
                    answer,
                    after_index,
                } => (answer, after_index as usize + 1, v),
                QuestionType::LastWith { answer } => (answer, v + 1, n),
                QuestionType::ClosestBefore {
                    answer,
                    before_index,
                } => (answer, v + 1, before_index as usize),
                QuestionType::NextSame => (src_ans, src + 1, v),
                QuestionType::PrevSame => (src_ans, v + 1, src),
                _ => continue,
            };

            let letter_oi = letter.idx();
            let mut q_mask = 0u16;
            for j in range_start..range_end {
                if answers[j].is_some() {
                    continue;
                }
                if !is_elim(eliminated, j, letter_oi) {
                    q_mask |= 1 << j;
                }
            }
            if q_mask != 0 {
                push(
                    DeduceAction::EliminateMulti {
                        question_mask: q_mask,
                        option_mask: 1 << letter_oi,
                    },
                    DeduceRule::PositionalRangeAnswered,
                );
            }
        }
    }

    if run(DeduceRule::PositionalRangeUnanswered) {
        // Unanswered positional rules: min/max of remaining options defines exclusion range
        for src in 0..n {
            if answers[src].is_some() {
                continue;
            }
            let t = &fp.question_types[src];
            match *t {
                QuestionType::FirstWith { answer } | QuestionType::ClosestAfter { answer, .. } => {
                    let scan_start = match *t {
                        QuestionType::ClosestAfter { after_index, .. } => after_index as usize + 1,
                        _ => 0,
                    };
                    let mut min_pos = n;
                    for oi in 0..5usize {
                        if is_elim(eliminated, src, oi) {
                            continue;
                        }
                        let v = fp.option_nums[src][oi];
                        if v >= 0 && (v as usize) < min_pos {
                            min_pos = v as usize;
                        }
                    }
                    let letter_oi = answer.idx();
                    let mut q_mask = 0u16;
                    for j in scan_start..min_pos {
                        if answers[j].is_some() {
                            continue;
                        }
                        if !is_elim(eliminated, j, letter_oi) {
                            q_mask |= 1 << j;
                        }
                    }
                    if q_mask != 0 {
                        push(
                            DeduceAction::EliminateMulti {
                                question_mask: q_mask,
                                option_mask: 1 << letter_oi,
                            },
                            DeduceRule::PositionalRangeUnanswered,
                        );
                    }
                }
                QuestionType::LastWith { answer } | QuestionType::ClosestBefore { answer, .. } => {
                    let scan_end = match *t {
                        QuestionType::ClosestBefore { before_index, .. } => before_index as usize,
                        _ => n,
                    };
                    let mut max_pos: i16 = -1;
                    for oi in 0..5usize {
                        if is_elim(eliminated, src, oi) {
                            continue;
                        }
                        let v = fp.option_nums[src][oi];
                        if v > max_pos {
                            max_pos = v;
                        }
                    }
                    let letter_oi = answer.idx();
                    let mut q_mask = 0u16;
                    let scan_start = if max_pos >= 0 {
                        max_pos as usize + 1
                    } else {
                        0
                    };
                    for j in scan_start..scan_end {
                        if answers[j].is_some() {
                            continue;
                        }
                        if !is_elim(eliminated, j, letter_oi) {
                            q_mask |= 1 << j;
                        }
                    }
                    if q_mask != 0 {
                        push(
                            DeduceAction::EliminateMulti {
                                question_mask: q_mask,
                                option_mask: 1 << letter_oi,
                            },
                            DeduceRule::PositionalRangeUnanswered,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    // ── OnlyOdd/OnlyEven range elimination ──
    if !fast && run(DeduceRule::OnlyOddEvenRangeElim) {
        for src in 0..n {
            if answers[src].is_some() {
                continue;
            }
            let (answer, parity) = match fp.question_types[src] {
                QuestionType::OnlyOdd { answer } => (answer, 1usize),
                QuestionType::OnlyEven { answer } => (answer, 0usize),
                _ => continue,
            };
            let answer_oi = answer.idx();

            let mut claimed = 0u16;
            for oi in 0..5usize {
                if is_elim(eliminated, src, oi) {
                    continue;
                }
                let v = fp.option_nums[src][oi];
                if v >= 0 && (v as usize) < n {
                    claimed |= 1 << v;
                }
            }

            let mut q_mask = 0u16;
            for j in 0..n {
                if j == src {
                    continue;
                }
                if (j + 1) % 2 != parity {
                    continue;
                }
                if answers[j].is_some() {
                    continue;
                }
                if (claimed >> j) & 1 == 1 {
                    continue;
                }
                if !is_elim(eliminated, j, answer_oi) {
                    q_mask |= 1 << j;
                }
            }
            if q_mask != 0 {
                push(
                    DeduceAction::EliminateMulti {
                        question_mask: q_mask,
                        option_mask: 1 << answer_oi,
                    },
                    DeduceRule::OnlyOddEvenRangeElim,
                );
            }
        }
    }

    // ── Vowel/consonant cross-elimination ──
    if !fast {
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
            if run(DeduceRule::VowelCrossElim) {
                for oi in 0..5usize {
                    if is_elim(eliminated, vq, oi) {
                        continue;
                    }
                    let v = fp.option_nums[vq][oi];
                    if v == NAN_VAL {
                        continue;
                    }
                    let need = nn - v;
                    let has = (0..5).any(|coi| {
                        !is_elim(eliminated, cq, coi) && fp.option_nums[cq][coi] == need
                    });
                    if !has {
                        push(
                            DeduceAction::Eliminate { qi: vq, oi },
                            DeduceRule::VowelCrossElim,
                        );
                    }
                }
            }
            if run(DeduceRule::ConsonantCrossElim) {
                for oi in 0..5usize {
                    if is_elim(eliminated, cq, oi) {
                        continue;
                    }
                    let v = fp.option_nums[cq][oi];
                    if v == NAN_VAL {
                        continue;
                    }
                    let need = nn - v;
                    let has = (0..5).any(|voi| {
                        !is_elim(eliminated, vq, voi) && fp.option_nums[vq][voi] == need
                    });
                    if !has {
                        push(
                            DeduceAction::Eliminate { qi: cq, oi },
                            DeduceRule::ConsonantCrossElim,
                        );
                    }
                }
            }
        }
    }

    // ── Eliminations ──
    for qi in 0..n {
        if answers[qi].is_some() {
            continue;
        }
        let t = &fp.question_types[qi];

        for oi in 0..5usize {
            if is_elim(eliminated, qi, oi) {
                continue;
            }
            let on = fp.option_nums[qi][oi];

            match *t {
                QuestionType::CountAnswer { answer }
                | QuestionType::CountAnswerBefore { answer, .. }
                | QuestionType::CountAnswerAfter { answer, .. }
                    if on != NAN_VAL =>
                {
                    let (from, to) = count_range(t, n);
                    let cr =
                        count_matching(answers, eliminated, CountPred::IsAnswer(answer), from, to);
                    if run(DeduceRule::CountExceeded) && cr.min() > on {
                        push(
                            DeduceAction::Eliminate { qi, oi },
                            DeduceRule::CountExceeded,
                        );
                    }
                    if run(DeduceRule::CountImpossible) && cr.max() < on {
                        push(
                            DeduceAction::Eliminate { qi, oi },
                            DeduceRule::CountImpossible,
                        );
                    }
                }
                QuestionType::CountVowel | QuestionType::CountConsonant if on != NAN_VAL => {
                    let pred = if matches!(*t, QuestionType::CountVowel) {
                        CountPred::IsVowel
                    } else {
                        CountPred::IsConsonant
                    };
                    let cr = count_matching(answers, eliminated, pred, 0, n);
                    if run(DeduceRule::CountExceeded) && cr.min() > on {
                        push(
                            DeduceAction::Eliminate { qi, oi },
                            DeduceRule::CountExceeded,
                        );
                    }
                    if run(DeduceRule::CountImpossible) && cr.max() < on {
                        push(
                            DeduceAction::Eliminate { qi, oi },
                            DeduceRule::CountImpossible,
                        );
                    }
                }
                QuestionType::MostCommonCount
                    if on != NAN_VAL && run(DeduceRule::MostCommonCountElim) =>
                {
                    let mut max_known: i16 = 0;
                    let mut max_possible: i16 = 0;
                    for &letter in &LETTERS[..fp.option_count] {
                        let cr =
                            count_matching(answers, eliminated, CountPred::IsAnswer(letter), 0, n);
                        if cr.min() > max_known {
                            max_known = cr.min();
                        }
                        if cr.max() > max_possible {
                            max_possible = cr.max();
                        }
                    }
                    if on < max_known || on > max_possible {
                        push(
                            DeduceAction::Eliminate { qi, oi },
                            DeduceRule::MostCommonCountElim,
                        );
                    }
                }
                QuestionType::AnswerOf { question_index } => {
                    let ov = fp.option_answers[qi][oi];
                    if ov <= 4 && run(DeduceRule::AnswerOfTargetRuledOut) {
                        if let Some(target) = answers[question_index as usize] {
                            if target as u8 != ov {
                                push(
                                    DeduceAction::Eliminate { qi, oi },
                                    DeduceRule::AnswerOfTargetRuledOut,
                                );
                            }
                        } else if is_elim(eliminated, question_index as usize, ov as usize) {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::AnswerOfTargetRuledOut,
                            );
                        }
                    }
                }
                QuestionType::LetterDist { question_index } => {
                    let max_dist = oi.max(4 - oi) as i16;
                    if run(DeduceRule::LetterDistImpossible) && on != NAN_VAL && on > max_dist {
                        push(
                            DeduceAction::Eliminate { qi, oi },
                            DeduceRule::LetterDistImpossible,
                        );
                    }
                    if run(DeduceRule::LetterDistWrong)
                        && let Some(other) = answers[question_index as usize]
                    {
                        let dist = (oi as i16 - other.idx() as i16).abs();
                        if dist != on {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::LetterDistWrong,
                            );
                        }
                    }
                    if run(DeduceRule::LetterDistNoMatch)
                        && on != NAN_VAL
                        && answers[question_index as usize].is_none()
                    {
                        let max_dist = oi.max(4 - oi) as i16;
                        if on <= max_dist {
                            let no_match = !(0..5usize).any(|ti| {
                                !is_elim(eliminated, question_index as usize, ti)
                                    && (oi as i16 - ti as i16).abs() == on
                            });
                            if no_match {
                                push(
                                    DeduceAction::Eliminate { qi, oi },
                                    DeduceRule::LetterDistNoMatch,
                                );
                            }
                        }
                    }
                }
                QuestionType::ClosestAfter {
                    answer,
                    after_index,
                } => {
                    let scan_start = after_index as usize + 1;
                    if run(DeduceRule::FirstClosestAfterOutOfRange)
                        && on != NONE_VAL
                        && ((on as usize) < scan_start || (on as usize) >= n)
                    {
                        push(
                            DeduceAction::Eliminate { qi, oi },
                            DeduceRule::FirstClosestAfterOutOfRange,
                        );
                    }
                    if on != NONE_VAL && (on as usize) >= scan_start && (on as usize) < n {
                        let pos = on as usize;
                        if run(DeduceRule::FirstClosestAfterWrongAnswer)
                            && let Some(pa) = answers[pos]
                            && pa != answer
                        {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::FirstClosestAfterWrongAnswer,
                            );
                        }
                        if run(DeduceRule::FirstClosestAfterRuledOut)
                            && answers[pos].is_none()
                            && is_elim(eliminated, pos, answer.idx())
                        {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::FirstClosestAfterRuledOut,
                            );
                        }
                        if run(DeduceRule::FirstClosestAfterEarlierMatch) {
                            for j in scan_start..pos {
                                if answers[j] == Some(answer) {
                                    push(
                                        DeduceAction::Eliminate { qi, oi },
                                        DeduceRule::FirstClosestAfterEarlierMatch,
                                    );
                                }
                            }
                        }
                        if run(DeduceRule::FirstClosestAfterSelfRef)
                            && LETTERS[oi] == answer
                            && qi >= scan_start
                            && qi < pos
                        {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::FirstClosestAfterSelfRef,
                            );
                        }
                    }
                    if run(DeduceRule::FirstClosestAfterNoneMatch)
                        && on == NONE_VAL
                        && (scan_start..n).any(|j| answers[j] == Some(answer))
                    {
                        push(
                            DeduceAction::Eliminate { qi, oi },
                            DeduceRule::FirstClosestAfterNoneMatch,
                        );
                    }
                }
                QuestionType::FirstWith { answer } => {
                    let scan_start = 0usize;
                    if run(DeduceRule::FirstClosestAfterOutOfRange)
                        && on != NONE_VAL
                        && ((on as usize) < scan_start || (on as usize) >= n)
                    {
                        push(
                            DeduceAction::Eliminate { qi, oi },
                            DeduceRule::FirstClosestAfterOutOfRange,
                        );
                    }
                    if on != NONE_VAL && (on as usize) < n {
                        let pos = on as usize;
                        if run(DeduceRule::FirstClosestAfterWrongAnswer)
                            && let Some(pa) = answers[pos]
                            && pa != answer
                        {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::FirstClosestAfterWrongAnswer,
                            );
                        }
                        if run(DeduceRule::FirstClosestAfterRuledOut)
                            && answers[pos].is_none()
                            && is_elim(eliminated, pos, answer.idx())
                        {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::FirstClosestAfterRuledOut,
                            );
                        }
                        if run(DeduceRule::FirstClosestAfterEarlierMatch) {
                            for j in scan_start..pos {
                                if answers[j] == Some(answer) {
                                    push(
                                        DeduceAction::Eliminate { qi, oi },
                                        DeduceRule::FirstClosestAfterEarlierMatch,
                                    );
                                }
                            }
                        }
                        if run(DeduceRule::FirstClosestAfterSelfRef)
                            && LETTERS[oi] == answer
                            && qi >= scan_start
                            && qi < pos
                        {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::FirstClosestAfterSelfRef,
                            );
                        }
                    }
                    if run(DeduceRule::FirstClosestAfterNoneMatch)
                        && on == NONE_VAL
                        && (scan_start..n).any(|j| answers[j] == Some(answer))
                    {
                        push(
                            DeduceAction::Eliminate { qi, oi },
                            DeduceRule::FirstClosestAfterNoneMatch,
                        );
                    }
                }
                QuestionType::ClosestBefore {
                    answer,
                    before_index,
                } => {
                    let before_idx = before_index as usize;
                    if run(DeduceRule::LastClosestBeforeOutOfRange)
                        && on != NONE_VAL
                        && (on < 0 || (on as usize) >= before_idx)
                    {
                        push(
                            DeduceAction::Eliminate { qi, oi },
                            DeduceRule::LastClosestBeforeOutOfRange,
                        );
                    }
                    if on != NONE_VAL && on >= 0 && (on as usize) < before_idx {
                        let pos = on as usize;
                        if run(DeduceRule::LastClosestBeforeWrongAnswer)
                            && let Some(pa) = answers[pos]
                            && pa != answer
                        {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::LastClosestBeforeWrongAnswer,
                            );
                        }
                        if run(DeduceRule::LastClosestBeforeRuledOut)
                            && answers[pos].is_none()
                            && is_elim(eliminated, pos, answer.idx())
                        {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::LastClosestBeforeRuledOut,
                            );
                        }
                        if run(DeduceRule::LastClosestBeforeLaterMatch) {
                            for j in ((pos + 1)..before_idx).rev() {
                                if answers[j] == Some(answer) {
                                    push(
                                        DeduceAction::Eliminate { qi, oi },
                                        DeduceRule::LastClosestBeforeLaterMatch,
                                    );
                                }
                            }
                        }
                        if run(DeduceRule::LastClosestBeforeSelfRef)
                            && LETTERS[oi] == answer
                            && qi > pos
                            && qi < before_idx
                        {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::LastClosestBeforeSelfRef,
                            );
                        }
                    }
                    if run(DeduceRule::LastClosestBeforeNoneMatch)
                        && on == NONE_VAL
                        && (0..before_idx).any(|j| answers[j] == Some(answer))
                    {
                        push(
                            DeduceAction::Eliminate { qi, oi },
                            DeduceRule::LastClosestBeforeNoneMatch,
                        );
                    }
                }
                QuestionType::LastWith { answer } => {
                    let before_idx = n;
                    if run(DeduceRule::LastClosestBeforeOutOfRange)
                        && on != NONE_VAL
                        && (on < 0 || (on as usize) >= before_idx)
                    {
                        push(
                            DeduceAction::Eliminate { qi, oi },
                            DeduceRule::LastClosestBeforeOutOfRange,
                        );
                    }
                    if on != NONE_VAL && on >= 0 && (on as usize) < before_idx {
                        let pos = on as usize;
                        if run(DeduceRule::LastClosestBeforeWrongAnswer)
                            && let Some(pa) = answers[pos]
                            && pa != answer
                        {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::LastClosestBeforeWrongAnswer,
                            );
                        }
                        if run(DeduceRule::LastClosestBeforeRuledOut)
                            && answers[pos].is_none()
                            && is_elim(eliminated, pos, answer.idx())
                        {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::LastClosestBeforeRuledOut,
                            );
                        }
                        if run(DeduceRule::LastClosestBeforeLaterMatch) {
                            for j in ((pos + 1)..before_idx).rev() {
                                if answers[j] == Some(answer) {
                                    push(
                                        DeduceAction::Eliminate { qi, oi },
                                        DeduceRule::LastClosestBeforeLaterMatch,
                                    );
                                }
                            }
                        }
                        if run(DeduceRule::LastClosestBeforeSelfRef)
                            && LETTERS[oi] == answer
                            && qi > pos
                            && qi < before_idx
                        {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::LastClosestBeforeSelfRef,
                            );
                        }
                    }
                    if run(DeduceRule::LastClosestBeforeNoneMatch)
                        && on == NONE_VAL
                        && (0..before_idx).any(|j| answers[j] == Some(answer))
                    {
                        push(
                            DeduceAction::Eliminate { qi, oi },
                            DeduceRule::LastClosestBeforeNoneMatch,
                        );
                    }
                }
                QuestionType::OnlyOdd { answer } | QuestionType::OnlyEven { answer } => {
                    let parity = match t {
                        QuestionType::OnlyOdd { .. } => 1,
                        _ => 0,
                    };
                    if on != NONE_VAL {
                        let pos = on as usize;
                        if run(DeduceRule::OnlyOddEvenWrongParity) && (pos + 1) % 2 != parity {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::OnlyOddEvenWrongParity,
                            );
                        }
                        if (pos + 1) % 2 == parity && pos < n {
                            if run(DeduceRule::OnlyOddEvenWrongAnswer)
                                && let Some(pa) = answers[pos]
                                && pa != answer
                            {
                                push(
                                    DeduceAction::Eliminate { qi, oi },
                                    DeduceRule::OnlyOddEvenWrongAnswer,
                                );
                            }
                            if run(DeduceRule::OnlyOddEvenRuledOut)
                                && answers[pos].is_none()
                                && is_elim(eliminated, pos, answer.idx())
                            {
                                push(
                                    DeduceAction::Eliminate { qi, oi },
                                    DeduceRule::OnlyOddEvenRuledOut,
                                );
                            }
                        }
                    } else {
                        if run(DeduceRule::OnlyOddEvenNoneMatch)
                            && (0..n).any(|i| (i + 1) % 2 == parity && answers[i] == Some(answer))
                        {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::OnlyOddEvenNoneMatch,
                            );
                        }
                    }
                }
                QuestionType::ConsecIdent => {
                    if on != NONE_VAL {
                        let pos = on as usize;
                        if run(DeduceRule::ConsecIdentOutOfRange) && pos + 1 >= n {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::ConsecIdentOutOfRange,
                            );
                        }
                        if pos + 1 < n {
                            let possible_a = !eliminated[pos] & 0b11111u8;
                            let possible_b = !eliminated[pos + 1] & 0b11111u8;
                            if run(DeduceRule::ConsecIdentNoCommon) && possible_a & possible_b == 0
                            {
                                push(
                                    DeduceAction::Eliminate { qi, oi },
                                    DeduceRule::ConsecIdentNoCommon,
                                );
                            }
                            if possible_a & possible_b != 0
                                && run(DeduceRule::ConsecIdentSelfRef)
                                && (pos == qi || pos + 1 == qi)
                            {
                                let partner = if pos == qi { pos + 1 } else { pos };
                                if is_elim(eliminated, partner, oi) {
                                    push(
                                        DeduceAction::Eliminate { qi, oi },
                                        DeduceRule::ConsecIdentSelfRef,
                                    );
                                }
                            }
                        }
                    } else {
                        if run(DeduceRule::ConsecIdentNonePair)
                            && (0..n.saturating_sub(1)).any(|i| {
                                matches!(
                                    (answers[i], answers[i + 1]),
                                    (Some(a), Some(b)) if a == b
                                )
                            })
                        {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::ConsecIdentNonePair,
                            );
                        }
                    }
                }
                QuestionType::EqualCount { answer } if on != NONE_VAL => {
                    if run(DeduceRule::EqualCountSelfRef) && LETTERS[on as usize] == answer {
                        push(
                            DeduceAction::Eliminate { qi, oi },
                            DeduceRule::EqualCountSelfRef,
                        );
                    }
                    if run(DeduceRule::EqualCountRangeElim) {
                        let claimed = LETTERS[on as usize];
                        if claimed != answer {
                            let ref_mask = 1u8 << answer.idx();
                            let claimed_mask = 1u8 << claimed.idx();
                            let mut rc = 0i16;
                            let mut rr = 0i16;
                            let mut sc = 0i16;
                            let mut sr = 0i16;
                            for j in 0..n {
                                if let Some(a) = answers[j] {
                                    if a == answer {
                                        rc += 1;
                                    }
                                    if a == claimed {
                                        sc += 1;
                                    }
                                } else {
                                    if eliminated[j] & ref_mask == 0 {
                                        rr += 1;
                                    }
                                    if eliminated[j] & claimed_mask == 0 {
                                        sr += 1;
                                    }
                                }
                            }
                            if rc + rr < sc || sc + sr < rc {
                                push(
                                    DeduceAction::Eliminate { qi, oi },
                                    DeduceRule::EqualCountRangeElim,
                                );
                            }
                        }
                    }
                }
                QuestionType::PrevSame
                    if on == NONE_VAL
                        && run(DeduceRule::PrevSameNoneMatch)
                        && (0..qi).any(|j| answers[j] == Some(LETTERS[oi])) =>
                {
                    push(
                        DeduceAction::Eliminate { qi, oi },
                        DeduceRule::PrevSameNoneMatch,
                    );
                }
                QuestionType::PrevSame if on != NONE_VAL => {
                    let pos = on as usize;
                    if run(DeduceRule::PrevSameNotBefore) && pos >= qi {
                        push(
                            DeduceAction::Eliminate { qi, oi },
                            DeduceRule::PrevSameNotBefore,
                        );
                    }
                    if pos < qi {
                        if run(DeduceRule::PrevSameRuledOut) && is_elim(eliminated, pos, oi) {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::PrevSameRuledOut,
                            );
                        }
                        if run(DeduceRule::PrevSameCloser)
                            && ((pos + 1)..qi)
                                .rev()
                                .any(|j| answers[j] == Some(LETTERS[oi]))
                        {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::PrevSameCloser,
                            );
                        }
                    }
                }
                QuestionType::NextSame
                    if on == NONE_VAL
                        && run(DeduceRule::NextSameNoneMatch)
                        && ((qi + 1)..n).any(|j| answers[j] == Some(LETTERS[oi])) =>
                {
                    push(
                        DeduceAction::Eliminate { qi, oi },
                        DeduceRule::NextSameNoneMatch,
                    );
                }
                QuestionType::NextSame if on != NONE_VAL => {
                    let pos = on as usize;
                    if run(DeduceRule::NextSameNotAfter) && (pos <= qi || pos >= n) {
                        push(
                            DeduceAction::Eliminate { qi, oi },
                            DeduceRule::NextSameNotAfter,
                        );
                    }
                    if pos > qi && pos < n {
                        if run(DeduceRule::NextSameRuledOut) && is_elim(eliminated, pos, oi) {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::NextSameRuledOut,
                            );
                        }
                        if run(DeduceRule::NextSameCloser)
                            && ((qi + 1)..pos).any(|j| answers[j] == Some(LETTERS[oi]))
                        {
                            push(
                                DeduceAction::Eliminate { qi, oi },
                                DeduceRule::NextSameCloser,
                            );
                        }
                    }
                }
                QuestionType::OnlySame
                    if on == NONE_VAL
                        && run(DeduceRule::OnlySameNoneMatch)
                        && (0..n).any(|j| j != qi && answers[j] == Some(LETTERS[oi])) =>
                {
                    push(
                        DeduceAction::Eliminate { qi, oi },
                        DeduceRule::OnlySameNoneMatch,
                    );
                }
                QuestionType::SameAsWhich { question_index }
                    if run(DeduceRule::SameAsWhichForward)
                        && let Some(ref_ans) = answers[question_index as usize]
                        && on >= 0
                        && (on as usize) < n
                        && on as usize != qi
                        && on as usize != question_index as usize =>
                {
                    let j = on as usize;
                    let wrong = match answers[j] {
                        Some(ja) => ja != ref_ans,
                        None => is_elim(eliminated, j, ref_ans.idx()),
                    };
                    if wrong {
                        push(
                            DeduceAction::Eliminate { qi, oi },
                            DeduceRule::SameAsWhichForward,
                        );
                    }
                }
                QuestionType::OnlySame | QuestionType::SameAs if on != NONE_VAL => {
                    let pos = on as usize;
                    if run(DeduceRule::OnlySameSelfRef) && pos == qi {
                        push(
                            DeduceAction::Eliminate { qi, oi },
                            DeduceRule::OnlySameSelfRef,
                        );
                    }
                    if run(DeduceRule::OnlySameRuledOut) && pos < n && is_elim(eliminated, pos, oi)
                    {
                        push(
                            DeduceAction::Eliminate { qi, oi },
                            DeduceRule::OnlySameRuledOut,
                        );
                    }
                    if run(DeduceRule::OnlySameOtherMatch)
                        && matches!(*t, QuestionType::OnlySame)
                        && pos < n
                        && pos != qi
                    {
                        let letter = LETTERS[oi];
                        for j in 0..n {
                            if j != qi && j != pos && answers[j] == Some(letter) {
                                push(
                                    DeduceAction::Eliminate { qi, oi },
                                    DeduceRule::OnlySameOtherMatch,
                                );
                                break;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // ── LeastCommon ──
    if !fast {
        for qi in 0..n {
            if answers[qi].is_some() {
                continue;
            }
            if !matches!(fp.question_types[qi], QuestionType::LeastCommon) {
                continue;
            }

            let mut min_count = [0i16; 5];
            let mut max_count = [0i16; 5];
            for j in 0..n {
                if j == qi {
                    continue;
                }
                if let Some(a) = answers[j] {
                    min_count[a.idx()] += 1;
                    max_count[a.idx()] += 1;
                } else {
                    for li in 0..5usize {
                        if !is_elim(eliminated, j, li) {
                            max_count[li] += 1;
                        }
                    }
                }
            }

            let mut can_be_least_opt = [false; 5];
            let mut must_be_least_opt = [false; 5];

            for oi in 0..5usize {
                if is_elim(eliminated, qi, oi) {
                    continue;
                }
                let v = fp.option_answers[qi][oi];
                if v >= 5 {
                    continue;
                }
                let claimed = v as usize;
                let self_letter = oi;

                let mut adj_min = min_count;
                let mut adj_max = max_count;
                adj_min[self_letter] += 1;
                adj_max[self_letter] += 1;

                let can_be_least =
                    (0..5).all(|li| li == claimed || adj_max[li] >= adj_min[claimed]);
                let must_be_least =
                    (0..5).all(|li| li == claimed || adj_min[li] > adj_max[claimed]);

                can_be_least_opt[oi] = can_be_least;
                must_be_least_opt[oi] = must_be_least;

                if run(DeduceRule::LeastCommonElim) && !can_be_least {
                    push(
                        DeduceAction::Eliminate { qi, oi },
                        DeduceRule::LeastCommonElim,
                    );
                }
            }

            if run(DeduceRule::LeastCommonForce) {
                for oi in 0..5usize {
                    if !must_be_least_opt[oi] {
                        continue;
                    }
                    let only_viable = (0..5usize)
                        .all(|oj| oj == oi || is_elim(eliminated, qi, oj) || !can_be_least_opt[oj]);
                    if only_viable {
                        push(
                            DeduceAction::Force {
                                qi,
                                answer: LETTERS[oi],
                            },
                            DeduceRule::LeastCommonForce,
                        );
                    }
                }
            }
        }
    }

    // ── MostCommon ──
    if !fast {
        for qi in 0..n {
            if answers[qi].is_some() {
                continue;
            }
            if !matches!(fp.question_types[qi], QuestionType::MostCommon) {
                continue;
            }

            let mut min_count = [0i16; 5];
            let mut max_count = [0i16; 5];
            for j in 0..n {
                if j == qi {
                    continue;
                }
                if let Some(a) = answers[j] {
                    min_count[a.idx()] += 1;
                    max_count[a.idx()] += 1;
                } else {
                    for li in 0..5usize {
                        if !is_elim(eliminated, j, li) {
                            max_count[li] += 1;
                        }
                    }
                }
            }

            let mut can_be_most_opt = [false; 5];
            let mut must_be_most_opt = [false; 5];

            for oi in 0..5usize {
                if is_elim(eliminated, qi, oi) {
                    continue;
                }
                let v = fp.option_answers[qi][oi];
                if v >= 5 {
                    continue;
                }
                let claimed = v as usize;
                let self_letter = oi;

                let mut adj_min = min_count;
                let mut adj_max = max_count;
                adj_min[self_letter] += 1;
                adj_max[self_letter] += 1;

                let can_be_most = (0..5).all(|li| li == claimed || adj_min[li] <= adj_max[claimed]);
                let must_be_most = (0..5).all(|li| li == claimed || adj_max[li] < adj_min[claimed]);

                can_be_most_opt[oi] = can_be_most;
                must_be_most_opt[oi] = must_be_most;

                if run(DeduceRule::MostCommonElim) && !can_be_most {
                    push(
                        DeduceAction::Eliminate { qi, oi },
                        DeduceRule::MostCommonElim,
                    );
                }
            }

            if run(DeduceRule::MostCommonForce) {
                for oi in 0..5usize {
                    if !must_be_most_opt[oi] {
                        continue;
                    }
                    let only_viable = (0..5usize)
                        .all(|oj| oj == oi || is_elim(eliminated, qi, oj) || !can_be_most_opt[oj]);
                    if only_viable {
                        push(
                            DeduceAction::Force {
                                qi,
                                answer: LETTERS[oi],
                            },
                            DeduceRule::MostCommonForce,
                        );
                    }
                }
            }
        }
    }

    // ── TrueStatement forward ──
    if !fast && run(DeduceRule::TrueStatementForward) {
        for qi in 0..n {
            let Some(a) = answers[qi] else { continue };
            if !matches!(fp.question_types[qi], QuestionType::TrueStmt) {
                continue;
            }
            let Some(claim) = &fp.option_claims[qi][a.idx()] else {
                continue;
            };
            match claim.question_type {
                QuestionType::FirstWith { answer } | QuestionType::LastWith { answer } => {
                    let tqi = claim.value;
                    if tqi >= 0
                        && (tqi as usize) < n
                        && answers[tqi as usize].is_none()
                        && !is_elim(eliminated, tqi as usize, answer.idx())
                    {
                        push(
                            DeduceAction::Force {
                                qi: tqi as usize,
                                answer,
                            },
                            DeduceRule::TrueStatementForward,
                        );
                    }
                }
                QuestionType::AnswerOf { question_index } => {
                    let tqi = question_index as usize;
                    if claim.value >= 0 && claim.value <= 4 && tqi < n && answers[tqi].is_none() {
                        let letter = LETTERS[claim.value as usize];
                        if !is_elim(eliminated, tqi, letter.idx()) {
                            push(
                                DeduceAction::Force {
                                    qi: tqi,
                                    answer: letter,
                                },
                                DeduceRule::TrueStatementForward,
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // ── OnlySame None forward ──
    if !fast && run(DeduceRule::OnlySameNoneForward) {
        for qi in 0..n {
            if !matches!(fp.question_types[qi], QuestionType::OnlySame) {
                continue;
            }
            let Some(a) = answers[qi] else { continue };
            let v = fp.option_nums[qi][a.idx()];
            if v != NONE_VAL {
                continue;
            }
            for j in 0..n {
                if j == qi {
                    continue;
                }
                if answers[j].is_none() && !is_elim(eliminated, j, a.idx()) {
                    push(
                        DeduceAction::Eliminate { qi: j, oi: a.idx() },
                        DeduceRule::OnlySameNoneForward,
                    );
                }
            }
        }
    }

    // SameAs negative: non-selected option targets cannot share this question's answer
    if run(DeduceRule::SameAsNegative) {
        for qi in 0..n {
            if !matches!(fp.question_types[qi], QuestionType::SameAs) {
                continue;
            }
            let Some(ans) = answers[qi] else { continue };
            let ai = ans.idx();
            let selected = fp.option_nums[qi][ai];
            let mut q_mask = 0u16;
            for oi in 0..5usize {
                if oi == ai {
                    continue;
                }
                let target = fp.option_nums[qi][oi];
                if target < 0 || target as usize >= n || target as usize == qi {
                    continue;
                }
                if target != selected
                    && answers[target as usize].is_none()
                    && !is_elim(eliminated, target as usize, ai)
                {
                    q_mask |= 1 << target;
                }
            }
            if q_mask != 0 {
                push(
                    DeduceAction::EliminateMulti {
                        question_mask: q_mask,
                        option_mask: 1 << ai,
                    },
                    DeduceRule::SameAsNegative,
                );
            }
        }
    }

    // ── ConsecIdent forward ──
    if !fast {
        for qi in 0..n {
            if !matches!(fp.question_types[qi], QuestionType::ConsecIdent) {
                continue;
            }
            let Some(a) = answers[qi] else { continue };
            let v = fp.option_nums[qi][a.idx()];
            if v == NONE_VAL || v < 0 || (v as usize) + 1 >= n {
                continue;
            }
            let p = v as usize;
            let poss_a = !eliminated[p] & 0b11111u8;
            let poss_b = !eliminated[p + 1] & 0b11111u8;

            if run(DeduceRule::ConsecIdentForwardForce) {
                if answers[p].is_some() && answers[p + 1].is_none() {
                    let letter = answers[p].unwrap();
                    if !is_elim(eliminated, p + 1, letter.idx()) {
                        push(
                            DeduceAction::Force {
                                qi: p + 1,
                                answer: letter,
                            },
                            DeduceRule::ConsecIdentForwardForce,
                        );
                    }
                }
                if answers[p + 1].is_some() && answers[p].is_none() {
                    let letter = answers[p + 1].unwrap();
                    if !is_elim(eliminated, p, letter.idx()) {
                        push(
                            DeduceAction::Force {
                                qi: p,
                                answer: letter,
                            },
                            DeduceRule::ConsecIdentForwardForce,
                        );
                    }
                }
            }

            if run(DeduceRule::ConsecIdentForwardElim) {
                for oi in 0..5usize {
                    if answers[p].is_none()
                        && !is_elim(eliminated, p, oi)
                        && (poss_b & (1 << oi)) == 0
                    {
                        push(
                            DeduceAction::Eliminate { qi: p, oi },
                            DeduceRule::ConsecIdentForwardElim,
                        );
                    }
                    if answers[p + 1].is_none()
                        && !is_elim(eliminated, p + 1, oi)
                        && (poss_a & (1 << oi)) == 0
                    {
                        push(
                            DeduceAction::Eliminate { qi: p + 1, oi },
                            DeduceRule::ConsecIdentForwardElim,
                        );
                    }
                }
            }

            if run(DeduceRule::ConsecIdentForwardBothForce)
                && answers[p].is_none()
                && answers[p + 1].is_none()
            {
                let common = poss_a & poss_b;
                if common.count_ones() == 1 {
                    let oi = common.trailing_zeros() as usize;
                    push(
                        DeduceAction::Force {
                            qi: p,
                            answer: LETTERS[oi],
                        },
                        DeduceRule::ConsecIdentForwardBothForce,
                    );
                    push(
                        DeduceAction::Force {
                            qi: p + 1,
                            answer: LETTERS[oi],
                        },
                        DeduceRule::ConsecIdentForwardBothForce,
                    );
                }
            }
        }
    }

    // ── SameAsWhich reverse ──
    if !fast && run(DeduceRule::SameAsWhichReverse) {
        for src in 0..n {
            let Some(src_ans) = answers[src] else {
                continue;
            };
            let QuestionType::SameAsWhich { question_index } = fp.question_types[src] else {
                continue;
            };
            let on = fp.option_nums[src][src_ans.idx()];
            if on < 0 || (on as usize) >= n {
                continue;
            }
            let j = on as usize;
            let qi_ref = question_index as usize;

            if let Some(ref_ans) = answers[qi_ref]
                && answers[j].is_none()
                && !is_elim(eliminated, j, ref_ans.idx())
            {
                push(
                    DeduceAction::Force {
                        qi: j,
                        answer: ref_ans,
                    },
                    DeduceRule::SameAsWhichReverse,
                );
            }
            if let Some(j_ans) = answers[j]
                && answers[qi_ref].is_none()
                && !is_elim(eliminated, qi_ref, j_ans.idx())
            {
                push(
                    DeduceAction::Force {
                        qi: qi_ref,
                        answer: j_ans,
                    },
                    DeduceRule::SameAsWhichReverse,
                );
            }
        }
    }

    // ── ConsecIdent reverse: eliminate matching neighbors for impossible pairs ──
    if run(DeduceRule::ConsecIdentReverse) {
        for qi in 0..n {
            if !matches!(fp.question_types[qi], QuestionType::ConsecIdent) {
                continue;
            }
            let mut possible_pairs = 0u16;
            for oi in 0..5usize {
                if is_elim(eliminated, qi, oi) {
                    continue;
                }
                let v = fp.option_nums[qi][oi];
                if v == NONE_VAL {
                    continue;
                }
                let pos = v as usize;
                if pos + 1 < n {
                    possible_pairs |= 1 << pos;
                }
            }
            for j in 0..n.saturating_sub(1) {
                if possible_pairs & (1 << j) != 0 {
                    continue;
                }
                if let Some(a) = answers[j]
                    && answers[j + 1].is_none()
                    && !is_elim(eliminated, j + 1, a.idx())
                {
                    push(
                        DeduceAction::Eliminate {
                            qi: j + 1,
                            oi: a.idx(),
                        },
                        DeduceRule::ConsecIdentReverse,
                    );
                }
                if let Some(b) = answers[j + 1]
                    && answers[j].is_none()
                    && !is_elim(eliminated, j, b.idx())
                {
                    push(
                        DeduceAction::Eliminate { qi: j, oi: b.idx() },
                        DeduceRule::ConsecIdentReverse,
                    );
                }
            }
        }
    }

    // ── TrueStatement self-reference ──
    if !fast && run(DeduceRule::TrueStatementSelfRef) {
        for qi in 0..n {
            if !matches!(fp.question_types[qi], QuestionType::TrueStmt) {
                continue;
            }
            for oi in 0..5usize {
                if is_elim(eliminated, qi, oi) {
                    continue;
                }
                let Some(claim) = &fp.option_claims[qi][oi] else {
                    continue;
                };
                let contradicts = match claim.question_type {
                    QuestionType::FirstWith { answer } | QuestionType::LastWith { answer } => {
                        claim.value as usize == qi && answer != LETTERS[oi]
                    }
                    QuestionType::AnswerOf { question_index } => {
                        question_index as usize == qi
                            && claim.value >= 0
                            && claim.value <= 4
                            && LETTERS[claim.value as usize] != LETTERS[oi]
                    }
                    _ => false,
                };
                if contradicts {
                    push(
                        DeduceAction::Eliminate { qi, oi },
                        DeduceRule::TrueStatementSelfRef,
                    );
                }
            }
        }
    }

    // ── TrueStatement claim invalid ──
    if !fast && run(DeduceRule::TrueStatementClaimInvalid) {
        for qi in 0..n {
            if !matches!(fp.question_types[qi], QuestionType::TrueStmt) {
                continue;
            }
            for oi in 0..5usize {
                if is_elim(eliminated, qi, oi) {
                    continue;
                }
                let Some(claim) = &fp.option_claims[qi][oi] else {
                    continue;
                };
                let v = crate::check_answer::check_claim(fp, *state, OptionPos { qi, oi }, *claim);
                if v == crate::check_answer::Validity::Invalid {
                    push(
                        DeduceAction::Eliminate { qi, oi },
                        DeduceRule::TrueStatementClaimInvalid,
                    );
                }
            }
        }
    }

    // TrueStatement claim valid: if exactly one non-eliminated claim is not provably false, force it
    if !fast && run(DeduceRule::TrueStatementClaimValid) {
        for qi in 0..n {
            if !matches!(fp.question_types[qi], QuestionType::TrueStmt) {
                continue;
            }
            if answers[qi].is_some() {
                continue;
            }
            let mut surviving_oi: Option<usize> = None;
            let mut surviving_count = 0;
            for oi in 0..5usize {
                if is_elim(eliminated, qi, oi) {
                    continue;
                }
                let Some(claim) = &fp.option_claims[qi][oi] else {
                    continue;
                };
                let mut hyp = *state;
                hyp.answers[qi] = Some(LETTERS[oi]);
                hyp.eliminated[qi] = 0b11111 ^ (1 << oi);
                let v = crate::check_answer::check_claim(fp, hyp, OptionPos { qi, oi }, *claim);
                if v != crate::check_answer::Validity::Invalid {
                    surviving_count += 1;
                    surviving_oi = Some(oi);
                }
            }
            if surviving_count == 1
                && let Some(oi) = surviving_oi
            {
                push(
                    DeduceAction::Force {
                        qi,
                        answer: LETTERS[oi],
                    },
                    DeduceRule::TrueStatementClaimValid,
                );
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn slow_test_duration() -> Option<std::time::Duration> {
        if std::env::var("REFPUZZLE_FAST_TESTS").is_ok() {
            return Some(std::time::Duration::from_millis(200));
        }
        if cfg!(debug_assertions) {
            panic!("slow test — run with --release or set REFPUZZLE_FAST_TESTS=1");
        }
        Some(std::time::Duration::from_secs(5))
    }

    // Mirrors src/lib/playground.ts encoding for cross-runner-compatible links.
    fn playground_link(puzzle: &Value, states: &[Value], n: usize) -> String {
        use base64::Engine;
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use flate2::Compression;
        use flate2::write::DeflateEncoder;
        use std::io::Write;

        let puzzle_json = serde_json::to_string(puzzle).unwrap();
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(puzzle_json.as_bytes()).unwrap();
        let p = URL_SAFE_NO_PAD.encode(encoder.finish().unwrap());

        // History: one action per non-unmarked option, iterated in (qi, oi) order
        // to match savedStateFromMarks in playground.ts.
        let mut actions: Vec<String> = Vec::new();
        for qi in 0..n {
            let s = states.get(qi).and_then(|v| v.as_str()).unwrap_or("");
            let mut correct: Option<usize> = None;
            let mut incorrect = [false; 5];
            for ch in s.chars() {
                if ch.is_ascii_uppercase() {
                    correct = Some((ch as u8 - b'A') as usize);
                } else if ch.is_ascii_lowercase() {
                    incorrect[(ch as u8 - b'a') as usize] = true;
                }
            }
            for oi in 0..5 {
                let letter = (b'A' + oi as u8) as char;
                if Some(oi) == correct {
                    actions.push(format!("{}{}", qi + 1, letter));
                } else if incorrect[oi] {
                    actions.push(format!("{}{}", qi + 1, letter.to_ascii_lowercase()));
                }
            }
        }
        let h = actions.join(".");

        let base = std::env::var("PLAYGROUND_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:5173".to_string());
        if h.is_empty() {
            format!("{base}/playground#p={p}")
        } else {
            format!("{base}/playground#p={p}&h={h}")
        }
    }

    #[test]
    fn test_shared_deduce() {
        let json_str =
            std::fs::read_to_string("../tests/deduce.json").expect("can't read tests/deduce.json");
        let suite: Value = serde_json::from_str(&json_str).unwrap();
        let tests = suite["tests"].as_array().unwrap();

        let mut passed = 0;
        let mut failed = 0;
        let mut covered_rules = std::collections::HashSet::new();
        let mut dry_failed = 0;

        for test in tests {
            if test.get("section").is_some() {
                continue;
            }
            let name = test["name"].as_str().unwrap();
            let states = test["state"].as_array().unwrap();
            let expect = test["expect"].as_str();
            let rule_filter = test.get("rule").and_then(|v| v.as_str());

            let fp = crate::serialize::parse_puzzle(&test["puzzle"]);
            let fp = match fp {
                Some(fp) => fp,
                None => {
                    failed += 1;
                    eprintln!("FAIL: {name}: parse failed");
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

            let parsed_rule = rule_filter.and_then(DeduceRule::from_str);
            if let Some(r) = parsed_rule {
                covered_rules.insert(r.to_str());
            }

            let state = State {
                answers,
                eliminated,
            };
            let drs = match parsed_rule {
                Some(r) => deduce_with_rule(&fp, &state, r),
                None => deduce(&fp, &state),
            };
            fn format_result(dr: Option<&DeduceResult>) -> String {
                match dr {
                    Some(DeduceResult {
                        action: DeduceAction::Force { qi, answer },
                        ..
                    }) => format!("{}{}", qi + 1, answer.as_char()),
                    Some(DeduceResult {
                        action: DeduceAction::Eliminate { qi, oi },
                        ..
                    }) => format!("{}{}", qi + 1, (b'a' + *oi as u8) as char),
                    Some(DeduceResult {
                        action:
                            DeduceAction::EliminateMulti {
                                question_mask,
                                option_mask,
                            },
                        ..
                    }) => format!("qm{:b}o{:05b}", question_mask, option_mask),
                    None => "null".to_string(),
                }
            }

            let got = format_result(drs.first());
            let expected = expect.unwrap_or("null");

            if got == expected {
                passed += 1;
            } else {
                failed += 1;
                eprintln!("FAIL: {name}");
                eprintln!("  expected: {expected}");
                eprintln!("  got:      {got}");
                eprintln!(
                    "  link:     {}",
                    playground_link(&test["puzzle"], states, n)
                );
            }

            // DRY check
            if let Some(r) = parsed_rule {
                if !drs.is_empty() && got == expected {
                    let without = deduce_with_rule_exclude(&fp, &state, DeduceRule::All, Some(r));
                    let without_got = format_result(without.first());
                    if without_got == got {
                        dry_failed += 1;
                        eprintln!("DRY: {name}");
                        eprintln!("  excluding {} still produces: {got}", r.to_str());
                    }
                }
            }
        }

        // Coverage check
        let uncovered: Vec<_> = ALL_DEDUCE_RULES
            .iter()
            .filter(|r| !covered_rules.contains(r.to_str()))
            .collect();
        for r in &uncovered {
            eprintln!("MISSING TEST COVERAGE: {}", r.to_str());
        }

        eprintln!("{passed}/{} passed", passed + failed);
        if dry_failed > 0 {
            eprintln!("{dry_failed} DRY violation(s)");
        }
        assert_eq!(failed, 0, "{failed} test(s) failed");
        assert_eq!(
            uncovered.len(),
            0,
            "{} rule(s) without tests",
            uncovered.len()
        );
        assert_eq!(dry_failed, 0, "{dry_failed} DRY violation(s)");
    }

    #[test]
    fn test_deduce_soundness_fuzz() {
        use crate::build::fill_options;
        use crate::rng::Rng;
        use crate::solve_brute::solve;

        fn random_question_type(rng: &mut Rng, qi: usize, n: usize) -> QuestionType {
            match rng.int(0, 24) {
                0 => QuestionType::CountAnswer {
                    answer: rng.pick(&LETTERS),
                },
                1 => QuestionType::CountAnswerBefore {
                    answer: rng.pick(&LETTERS),
                    before_index: rng.int(2, n as i32 - 1) as u8,
                },
                2 => QuestionType::CountAnswerAfter {
                    answer: rng.pick(&LETTERS),
                    after_index: rng.int(0, n as i32 - 3) as u8,
                },
                3 => QuestionType::CountVowel,
                4 => QuestionType::CountConsonant,
                5 => QuestionType::MostCommonCount,
                6 => QuestionType::ClosestAfter {
                    after_index: rng.int(0, n as i32 - 3) as u8,
                    answer: rng.pick(&LETTERS),
                },
                7 => QuestionType::ClosestBefore {
                    before_index: rng.int(2, n as i32 - 1) as u8,
                    answer: rng.pick(&LETTERS),
                },
                8 => QuestionType::FirstWith {
                    answer: rng.pick(&LETTERS),
                },
                9 => QuestionType::LastWith {
                    answer: rng.pick(&LETTERS),
                },
                10 if qi >= 2 => QuestionType::PrevSame,
                11 if qi + 2 < n => QuestionType::NextSame,
                12 => QuestionType::OnlySame,
                13 => QuestionType::SameAs,
                14 => QuestionType::OnlyOdd {
                    answer: rng.pick(&LETTERS),
                },
                15 => QuestionType::OnlyEven {
                    answer: rng.pick(&LETTERS),
                },
                16 => QuestionType::ConsecIdent,
                17 => {
                    let q = rng.int(0, n as i32 - 1) as u8;
                    if q as usize == qi {
                        QuestionType::AnswerIsSelf
                    } else {
                        QuestionType::AnswerOf { question_index: q }
                    }
                }
                18 => QuestionType::LeastCommon,
                19 => QuestionType::MostCommon,
                20 => QuestionType::NoOtherHasAnswer,
                21 => QuestionType::EqualCount {
                    answer: rng.pick(&LETTERS),
                },
                22 => QuestionType::AnswerIsSelf,
                23 => {
                    let q = rng.int(0, n as i32 - 1) as u8;
                    if q as usize == qi {
                        QuestionType::AnswerIsSelf
                    } else {
                        QuestionType::LetterDist { question_index: q }
                    }
                }
                24 => {
                    let q = rng.int(0, n as i32 - 1) as u8;
                    if q as usize == qi {
                        QuestionType::AnswerIsSelf
                    } else {
                        QuestionType::SameAsWhich { question_index: q }
                    }
                }
                _ => QuestionType::AnswerIsSelf,
            }
        }

        let Some(duration) = slow_test_duration() else { return; };
        let mut failures = 0;
        let mut puzzles_tested = 0;
        let deadline = std::time::Instant::now() + duration;

        for seed in 0u32.. {
            if seed % 100 == 0 && std::time::Instant::now() > deadline {
                break;
            }
            let mut rng = Rng::new(seed.wrapping_mul(7919).wrapping_add(42));
            let n = rng.int(4, 8) as usize;

            let solution: [Answer; MAX_N] =
                std::array::from_fn(|i| if i < n { rng.pick(&LETTERS) } else { Answer::A });

            let mut question_types = [QuestionType::AnswerIsSelf; MAX_N];
            for qi in 0..n {
                question_types[qi] = random_question_type(&mut rng, qi, n);
            }

            let fp = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                fill_options(&question_types, &solution, n, 5, &mut Rng::new(seed))
            }));
            let Ok(Some(fp)) = fp else { continue };

            let solutions = solve(&fp, None, 2);
            if solutions.len() != 1 {
                continue;
            }
            if (0..n).any(|i| solutions[0][i] != solution[i]) {
                eprintln!("CONSTRUCTION FAIL seed={seed}:");
                eprintln!(
                    "  construction: {:?}",
                    &solution[..n]
                        .iter()
                        .map(|a| a.as_char())
                        .collect::<Vec<_>>()
                );
                eprintln!(
                    "  brute:        {:?}",
                    &solutions[0][..n]
                        .iter()
                        .map(|a| a.as_char())
                        .collect::<Vec<_>>()
                );
                for qi in 0..n {
                    eprintln!(
                        "  Q{}: {:?} opts={:?} ans={:?}",
                        qi + 1,
                        fp.question_types[qi],
                        &fp.option_nums[qi],
                        &fp.option_answers[qi]
                    );
                }
                panic!("fill_options bug: brute solution != construction solution (seed={seed})");
            }

            puzzles_tested += 1;

            for state_seed in 0..20u32 {
                let mut rng = Rng::new(seed.wrapping_mul(1000).wrapping_add(state_seed));
                let mut answers: [Option<Answer>; MAX_N] = [None; MAX_N];
                let mut eliminated = [0u8; MAX_N];

                for qi in 0..n {
                    let r = rng.int(0, 4);
                    if r == 0 {
                        answers[qi] = Some(solution[qi]);
                        eliminated[qi] = 0b11111 ^ (1 << solution[qi].idx());
                    } else if r <= 2 {
                        let count = rng.int(1, 3) as usize;
                        for _ in 0..count {
                            let oi = rng.int(0, 4) as usize;
                            if LETTERS[oi] != solution[qi] {
                                eliminated[qi] |= 1 << oi;
                            }
                        }
                    }
                }

                let drs = deduce(
                    &fp,
                    &State {
                        answers,
                        eliminated,
                    },
                );
                for dr in &drs {
                    let bad = match dr.action {
                        DeduceAction::Force { qi, answer } => answer != solution[qi],
                        DeduceAction::Eliminate { qi, oi } => LETTERS[oi] == solution[qi],
                        DeduceAction::EliminateMulti {
                            question_mask,
                            option_mask,
                        } => (0..n).any(|qi| {
                            (question_mask >> qi) & 1 == 1
                                && (option_mask >> solution[qi].idx()) & 1 == 1
                        }),
                    };
                    if bad {
                        failures += 1;
                        if failures <= 3 {
                            eprintln!(
                                "SOUNDNESS FAIL seed={} state_seed={} rule={}: {:?}",
                                seed,
                                state_seed,
                                dr.rule.to_str(),
                                dr.action
                            );
                            eprintln!(
                                "  solution: {:?}",
                                &solution[..n]
                                    .iter()
                                    .map(|a| a.as_char())
                                    .collect::<Vec<_>>()
                            );
                            eprintln!("  answers:  {:?}", &answers[..n]);
                            eprintln!(
                                "  elim:     {:?}",
                                &eliminated[..n]
                                    .iter()
                                    .map(|e| format!("{:05b}", e))
                                    .collect::<Vec<_>>()
                            );
                            for qi in 0..n {
                                eprintln!(
                                    "  Q{}: {:?} opts={:?} ans={:?}",
                                    qi + 1,
                                    fp.question_types[qi],
                                    &fp.option_nums[qi],
                                    &fp.option_answers[qi]
                                );
                            }
                        }
                    }
                }
            }
        }

        eprintln!("Fuzz: {puzzles_tested} puzzles tested, {failures} soundness failures");
        assert_eq!(failures, 0, "{failures} soundness failure(s)");
    }
}
