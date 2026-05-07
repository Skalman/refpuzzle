use crate::evaluate::evaluate_claim;
use crate::types::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Validity {
    Valid,
    Invalid,
    Pending,
}

// ── Helpers ──

struct CountResult {
    count: i16,
    remaining: i16,
}

#[derive(Clone, Copy)]
#[allow(clippy::enum_variant_names)]
enum Pred {
    IsAnswer(Answer),
    IsVowel,
    IsConsonant,
}

impl Pred {
    fn matches(self, a: Answer) -> bool {
        match self {
            Pred::IsAnswer(t) => a == t,
            Pred::IsVowel => a.is_vowel(),
            Pred::IsConsonant => !a.is_vowel(),
        }
    }
    fn mask(self) -> u8 {
        match self {
            Pred::IsAnswer(t) => 1u8 << t.idx(),
            Pred::IsVowel => 0b10001,
            Pred::IsConsonant => 0b01110,
        }
    }
}

fn count_matching(
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    pred: Pred,
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

fn count_validity(cr: CountResult, value: i16) -> Validity {
    if cr.count > value || cr.count + cr.remaining < value {
        Validity::Invalid
    } else if cr.count == value && cr.remaining == 0 {
        Validity::Valid
    } else {
        Validity::Pending
    }
}

fn count_range(t: &QuestionType, n: usize) -> (usize, usize) {
    match *t {
        QuestionType::CountAnswerBefore { before_index, .. } => (0, before_index as usize),
        QuestionType::CountAnswerAfter { after_index, .. } => (after_index as usize + 1, n),
        _ => (0, n),
    }
}

fn first_in_range(
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    answer: Answer,
    start: usize,
    end: usize,
    pos: i16,
) -> Validity {
    let amask = 1u8 << answer.idx();
    if pos != NONE_VAL {
        let p = pos as usize;
        if p < start || p >= end {
            return Validity::Invalid;
        }
        if let Some(pa) = answers[p] {
            if pa != answer {
                return Validity::Invalid;
            }
        } else if eliminated[p] & amask != 0 {
            return Validity::Invalid;
        }
        let mut all_certain = true;
        for j in start..p {
            if answers[j] == Some(answer) {
                return Validity::Invalid;
            }
            if answers[j].is_none() && eliminated[j] & amask == 0 {
                all_certain = false;
            }
        }
        if answers[p] == Some(answer) && all_certain {
            Validity::Valid
        } else {
            Validity::Pending
        }
    } else {
        let mut could_exist = false;
        for j in start..end {
            if answers[j] == Some(answer) {
                return Validity::Invalid;
            }
            if answers[j].is_none() && eliminated[j] & amask == 0 {
                could_exist = true;
            }
        }
        if could_exist {
            Validity::Pending
        } else {
            Validity::Valid
        }
    }
}

fn last_in_range(
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    answer: Answer,
    start: usize,
    end: usize,
    pos: i16,
) -> Validity {
    let amask = 1u8 << answer.idx();
    if pos != NONE_VAL {
        let p = pos as usize;
        if p < start || p >= end {
            return Validity::Invalid;
        }
        if let Some(pa) = answers[p] {
            if pa != answer {
                return Validity::Invalid;
            }
        } else if eliminated[p] & amask != 0 {
            return Validity::Invalid;
        }
        let mut all_certain = true;
        for j in (p + 1)..end {
            if answers[j] == Some(answer) {
                return Validity::Invalid;
            }
            if answers[j].is_none() && eliminated[j] & amask == 0 {
                all_certain = false;
            }
        }
        if answers[p] == Some(answer) && all_certain {
            Validity::Valid
        } else {
            Validity::Pending
        }
    } else {
        let mut could_exist = false;
        for j in start..end {
            if answers[j] == Some(answer) {
                return Validity::Invalid;
            }
            if answers[j].is_none() && eliminated[j] & amask == 0 {
                could_exist = true;
            }
        }
        if could_exist {
            Validity::Pending
        } else {
            Validity::Valid
        }
    }
}

fn all_answered(answers: &[Option<Answer>; MAX_N], n: usize) -> bool {
    (0..n).all(|i| answers[i].is_some())
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

fn count_answer_with_remaining(
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    target: Answer,
    from: usize,
    to: usize,
) -> (i16, i16) {
    let mask = 1u8 << target.idx();
    let mut count: i16 = 0;
    let mut remaining: i16 = 0;
    for i in from..to {
        if answers[i] == Some(target) {
            count += 1;
        } else if answers[i].is_none() && eliminated[i] & mask == 0 {
            remaining += 1;
        }
    }
    (count, remaining)
}

fn fill_counts(answers: &[Option<Answer>; MAX_N], n: usize) -> [i16; 5] {
    let mut counts = [0i16; 5];
    for i in 0..n {
        if let Some(a) = answers[i] {
            counts[a.idx()] += 1;
        }
    }
    counts
}

// ── Main function ──

pub fn check_answer_validity(
    fp: &FlatPuzzle,
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    qi: usize,
) -> Validity {
    let a = match answers[qi] {
        Some(a) => a,
        None => return Validity::Pending,
    };
    let ai = a.idx();
    let t = &fp.question_types[qi];
    let on = fp.option_nums[qi][ai];
    let n = fp.n;

    match *t {
        // ── Counting ──
        QuestionType::CountAnswer { answer }
        | QuestionType::CountAnswerBefore { answer, .. }
        | QuestionType::CountAnswerAfter { answer, .. } => {
            if on == NAN_VAL {
                return Validity::Pending;
            }
            let (from, to) = count_range(t, n);
            let cr = count_matching(answers, eliminated, Pred::IsAnswer(answer), from, to);
            count_validity(cr, on)
        }

        QuestionType::CountVowel => {
            if on == NAN_VAL {
                return Validity::Pending;
            }
            let cr = count_matching(answers, eliminated, Pred::IsVowel, 0, n);
            count_validity(cr, on)
        }

        QuestionType::CountConsonant => {
            if on == NAN_VAL {
                return Validity::Pending;
            }
            let cr = count_matching(answers, eliminated, Pred::IsConsonant, 0, n);
            count_validity(cr, on)
        }

        QuestionType::MostCommonCount => {
            if on == NAN_VAL {
                return Validity::Pending;
            }
            let c = fill_counts(answers, n);
            for i in 0..5 {
                if c[i] > on {
                    return Validity::Invalid;
                }
            }
            if !all_answered(answers, n) {
                return Validity::Pending;
            }
            let max = c.iter().copied().max().unwrap_or(0);
            if max == on {
                Validity::Valid
            } else {
                Validity::Invalid
            }
        }

        // ── Positional: first/closest-after ──
        QuestionType::FirstWith { answer } => first_in_range(answers, eliminated, answer, 0, n, on),

        QuestionType::ClosestAfter {
            after_index,
            answer,
        } => first_in_range(answers, eliminated, answer, after_index as usize + 1, n, on),

        // ── Positional: last/closest-before ──
        QuestionType::LastWith { answer } => last_in_range(answers, eliminated, answer, 0, n, on),

        QuestionType::ClosestBefore {
            before_index,
            answer,
        } => last_in_range(answers, eliminated, answer, 0, before_index as usize, on),

        // ── Reference ──
        QuestionType::AnswerOf { question_index } => {
            let claimed = fp.option_answers[qi][ai];
            match answers[question_index as usize] {
                Some(target) => {
                    if target as u8 == claimed {
                        Validity::Valid
                    } else {
                        Validity::Invalid
                    }
                }
                None => Validity::Pending,
            }
        }

        QuestionType::LetterDist { question_index } => match answers[question_index as usize] {
            Some(other) => {
                let dist = (ai as i16 - other.idx() as i16).abs();
                if dist == on {
                    Validity::Valid
                } else {
                    Validity::Invalid
                }
            }
            None => Validity::Pending,
        },

        QuestionType::SameAs => {
            if on < 0 || on as usize >= n || on as usize == qi {
                return Validity::Invalid;
            }
            match answers[on as usize] {
                Some(ta) => {
                    if ta == a {
                        Validity::Valid
                    } else {
                        Validity::Invalid
                    }
                }
                None => Validity::Pending,
            }
        }

        QuestionType::SameAsWhich { question_index } => {
            if on < 0
                || on as usize >= n
                || on as usize == qi
                || on as usize == question_index as usize
            {
                return Validity::Invalid;
            }
            let ref_ans = match answers[question_index as usize] {
                Some(a) => a,
                None => return Validity::Pending,
            };
            match answers[on as usize] {
                Some(ta) => {
                    if ta == ref_ans {
                        Validity::Valid
                    } else {
                        Validity::Invalid
                    }
                }
                None => Validity::Pending,
            }
        }

        // ── Unique ──
        QuestionType::Unique => {
            let amask = 1u8 << ai;
            let mut others: i16 = 0;
            let mut could_match: i16 = 0;
            for j in 0..n {
                if j == qi {
                    continue;
                }
                match answers[j] {
                    Some(x) if x == a => others += 1,
                    None if eliminated[j] & amask == 0 => could_match += 1,
                    _ => {}
                }
            }
            if others > 0 {
                Validity::Invalid
            } else if could_match == 0 {
                Validity::Valid
            } else {
                Validity::Pending
            }
        }

        // ── Previous/Next same ──
        QuestionType::PrevSame => {
            if on != NONE_VAL && (on < 0 || on as usize >= qi) {
                return Validity::Invalid;
            }
            last_in_range(answers, eliminated, a, 0, qi, on)
        }

        QuestionType::NextSame => {
            if on != NONE_VAL && (on as usize <= qi || on as usize >= n) {
                return Validity::Invalid;
            }
            first_in_range(answers, eliminated, a, qi + 1, n, on)
        }

        // ── Only same ──
        QuestionType::OnlySame => {
            let amask = 1u8 << ai;

            if on == NONE_VAL {
                let mut matches: i16 = 0;
                let mut could_match: i16 = 0;
                for j in 0..n {
                    if j == qi {
                        continue;
                    }
                    match answers[j] {
                        Some(x) if x == a => matches += 1,
                        None if eliminated[j] & amask == 0 => could_match += 1,
                        _ => {}
                    }
                }
                if matches > 0 {
                    Validity::Invalid
                } else if could_match == 0 {
                    Validity::Valid
                } else {
                    Validity::Pending
                }
            } else if on < 0 || on as usize >= n {
                Validity::Invalid
            } else {
                let target = on as usize;
                if target == qi {
                    return Validity::Invalid;
                }

                if let Some(ta) = answers[target]
                    && ta != a
                {
                    return Validity::Invalid;
                }

                let mut other_matches: i16 = 0;
                let mut other_remaining: i16 = 0;
                for j in 0..n {
                    if j == qi || j == target {
                        continue;
                    }
                    match answers[j] {
                        Some(x) if x == a => other_matches += 1,
                        None if eliminated[j] & amask == 0 => other_remaining += 1,
                        _ => {}
                    }
                }

                if other_matches > 0 {
                    return Validity::Invalid;
                }
                if answers[target] == Some(a) && other_remaining == 0 {
                    Validity::Valid
                } else {
                    Validity::Pending
                }
            }
        }

        // ── Consecutive identical ──
        QuestionType::ConsecIdent => {
            if on != NONE_VAL {
                let p = on as usize;
                if p + 1 >= n {
                    return Validity::Invalid;
                }

                if let (Some(pa), Some(pb)) = (answers[p], answers[p + 1])
                    && pa != pb
                {
                    return Validity::Invalid;
                }

                let poss_a = !eliminated[p] & 0b11111u8;
                let poss_b = !eliminated[p + 1] & 0b11111u8;
                if poss_a & poss_b == 0 {
                    return Validity::Invalid;
                }
                if let Some(pa) = answers[p]
                    && eliminated[p + 1] & (1 << pa.idx()) != 0
                {
                    return Validity::Invalid;
                }
                if let Some(pb) = answers[p + 1]
                    && eliminated[p] & (1 << pb.idx()) != 0
                {
                    return Validity::Invalid;
                }

                let mut other_confirmed_pairs = 0i16;
                let mut uncertain_pairs = 0i16;
                for j in 0..n.saturating_sub(1) {
                    if j == p {
                        continue;
                    }
                    match (answers[j], answers[j + 1]) {
                        (Some(x), Some(y)) if x == y => other_confirmed_pairs += 1,
                        (Some(_), Some(_)) => {}
                        _ => uncertain_pairs += 1,
                    }
                }

                if other_confirmed_pairs > 0 {
                    return Validity::Invalid;
                }

                if let (Some(pa), Some(pb)) = (answers[p], answers[p + 1])
                    && pa == pb
                    && uncertain_pairs == 0
                {
                    return Validity::Valid;
                }

                Validity::Pending
            } else {
                let mut any_confirmed_pair = false;
                let mut any_uncertain = false;
                for j in 0..n.saturating_sub(1) {
                    match (answers[j], answers[j + 1]) {
                        (Some(x), Some(y)) if x == y => any_confirmed_pair = true,
                        (Some(_), Some(_)) => {}
                        _ => any_uncertain = true,
                    }
                }
                if any_confirmed_pair {
                    Validity::Invalid
                } else if any_uncertain {
                    Validity::Pending
                } else {
                    Validity::Valid
                }
            }
        }

        // ── Only odd / only even ──
        QuestionType::OnlyOdd { answer } | QuestionType::OnlyEven { answer } => {
            let parity = match *t {
                QuestionType::OnlyOdd { .. } => 1,
                _ => 0,
            };
            let amask = 1u8 << answer.idx();

            if on != NONE_VAL {
                let p = on as usize;
                if (p + 1) % 2 != parity {
                    return Validity::Invalid;
                }

                if let Some(pa) = answers[p]
                    && pa != answer
                {
                    return Validity::Invalid;
                }

                let mut other_matches: i16 = 0;
                let mut other_remaining: i16 = 0;
                for j in 0..n {
                    if j == p || (j + 1) % 2 != parity {
                        continue;
                    }
                    match answers[j] {
                        Some(x) if x == answer => other_matches += 1,
                        None if eliminated[j] & amask == 0 => other_remaining += 1,
                        _ => {}
                    }
                }

                if other_matches > 0 {
                    return Validity::Invalid;
                }
                if answers[p] == Some(answer) && other_remaining == 0 {
                    Validity::Valid
                } else {
                    Validity::Pending
                }
            } else {
                let mut any_match = false;
                let mut any_could = false;
                for j in 0..n {
                    if (j + 1) % 2 != parity {
                        continue;
                    }
                    if answers[j] == Some(answer) {
                        any_match = true;
                    }
                    if answers[j].is_none() && eliminated[j] & amask == 0 {
                        any_could = true;
                    }
                }
                if any_match {
                    Validity::Invalid
                } else if any_could {
                    Validity::Pending
                } else {
                    Validity::Valid
                }
            }
        }

        // ── True statement ──
        QuestionType::TrueStmt => {
            if !all_answered(answers, n) {
                return Validity::Pending;
            }
            let claims = &fp.option_claims[qi];
            let mut true_count = 0u8;
            let mut selected_true = false;
            for i in 0..5 {
                if claims[i] != Claim::None && evaluate_claim(&claims[i], answers, n) {
                    true_count += 1;
                    if i == ai {
                        selected_true = true;
                    }
                }
            }
            if selected_true && true_count == 1 {
                Validity::Valid
            } else {
                Validity::Invalid
            }
        }

        // ── Always valid ──
        QuestionType::AnswerIsSelf => Validity::Valid,

        // ── Equal count ──
        QuestionType::EqualCount { answer } => {
            if on != NONE_VAL {
                let claimed = LETTERS[on as usize];
                if claimed == answer {
                    return Validity::Invalid;
                }
                let (rc, rr) = count_answer_with_remaining(answers, eliminated, answer, 0, n);
                let (sc, sr) = count_answer_with_remaining(answers, eliminated, claimed, 0, n);
                if rc + rr < sc || sc + sr < rc {
                    return Validity::Invalid;
                }
                if rr == 0 && sr == 0 {
                    return if rc == sc {
                        Validity::Valid
                    } else {
                        Validity::Invalid
                    };
                }
                Validity::Pending
            } else {
                if !all_answered(answers, n) {
                    return Validity::Pending;
                }
                let ref_count = count_answer_simple(answers, answer, 0, n);
                let any_match = LETTERS
                    .iter()
                    .any(|&l| l != answer && count_answer_simple(answers, l, 0, n) == ref_count);
                if any_match {
                    Validity::Invalid
                } else {
                    Validity::Valid
                }
            }
        }

        // ── Global: need all answers ──
        QuestionType::LeastCommon | QuestionType::MostCommon => {
            if !all_answered(answers, n) {
                return Validity::Pending;
            }
            let claimed = fp.option_answers[qi][ai] as usize;
            if claimed >= 5 {
                return Validity::Invalid;
            }
            let c = fill_counts(answers, n);
            match *t {
                QuestionType::LeastCommon => {
                    let min = c.iter().copied().min().unwrap_or(0);
                    if c[claimed] == min && c.iter().filter(|&&x| x == min).count() == 1 {
                        Validity::Valid
                    } else {
                        Validity::Invalid
                    }
                }
                QuestionType::MostCommon => {
                    let max = c.iter().copied().max().unwrap_or(0);
                    if c[claimed] == max && c.iter().filter(|&&x| x == max).count() == 1 {
                        Validity::Valid
                    } else {
                        Validity::Invalid
                    }
                }
                _ => unreachable!(),
            }
        }
    }
}

pub fn check_question_against_solution(
    fp: &FlatPuzzle,
    qi: usize,
    _selected: Answer,
    answers: &[Option<Answer>; MAX_N],
) -> bool {
    let empty = [0u8; MAX_N];
    check_answer_validity(fp, answers, &empty, qi) == Validity::Valid
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn test_shared_check_validity() {
        let json_str = std::fs::read_to_string("../tests/check-validity.json")
            .expect("can't read tests/check-validity.json");
        let suite: Value = serde_json::from_str(&json_str).unwrap();
        let tests = suite["tests"].as_array().unwrap();

        let mut passed = 0;
        let mut failed = 0;

        for test in tests {
            if test.get("section").is_some() {
                continue;
            }
            let name = test["name"].as_str().unwrap();
            let qi = test["qi"].as_u64().unwrap() as usize;
            let states = test["state"].as_array().unwrap();
            let expect = test["expect"].as_str().unwrap();

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

            let got = check_answer_validity(&fp, &answers, &eliminated, qi);
            let got_str = match got {
                Validity::Valid => "valid",
                Validity::Invalid => "invalid",
                Validity::Pending => "pending",
            };

            if got_str == expect {
                passed += 1;
            } else {
                failed += 1;
                eprintln!("FAIL: {name}");
                eprintln!("  expected: {expect}");
                eprintln!("  got:      {got_str}");
            }
        }

        eprintln!("{passed}/{} passed", passed + failed);
        assert_eq!(failed, 0, "{failed} test(s) failed");
    }

    #[test]
    fn test_shared_evaluators() {
        let json_str = std::fs::read_to_string("../tests/evaluators.json")
            .expect("can't read tests/evaluators.json");
        let suite: Value = serde_json::from_str(&json_str).unwrap();
        let tests = suite["tests"].as_array().unwrap();

        let mut passed = 0;
        let mut failed = 0;

        for test in tests {
            if test.get("section").is_some() {
                continue;
            }
            let name = test["name"].as_str().unwrap();
            let qi = test["qi"].as_u64().unwrap() as usize;
            let expect = test["expect"].as_bool().unwrap();

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
            let answer_arr = test["answers"].as_array().unwrap();
            for i in 0..n {
                if let Some(s) = answer_arr[i].as_str() {
                    answers[i] = Some(LETTERS[(s.as_bytes()[0] - b'A') as usize]);
                }
            }

            let got = check_question_against_solution(&fp, qi, answers[qi].unwrap(), &answers);

            if got == expect {
                passed += 1;
            } else {
                failed += 1;
                eprintln!("FAIL: {name}");
                eprintln!("  expected: {expect}");
                eprintln!("  got:      {got}");
            }
        }

        eprintln!("{passed}/{} passed", passed + failed);
        assert_eq!(failed, 0, "{failed} test(s) failed");
    }
}
