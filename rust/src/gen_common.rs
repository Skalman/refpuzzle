use crate::evaluate::{evaluate, evaluate_claim};
use crate::hints::{apply_action, find_action_fast, find_lookahead_action};
use crate::rng::Rng;
use crate::solver::solve;
use crate::types::*;

pub struct GenerateResult {
    pub rules: [Rule; MAX_N],
    pub solution: [Answer; MAX_N],
    pub fp: FlatPuzzle,
    pub n: usize,
}

pub fn to_optional(sol: &[Answer; MAX_N], n: usize) -> [Option<Answer>; MAX_N] {
    let mut arr = [None; MAX_N];
    for i in 0..n {
        arr[i] = Some(sol[i]);
    }
    arr
}

pub fn check_structural(rule: &Rule, qi: usize, sol: &[Answer; MAX_N], n: usize) -> bool {
    match *rule {
        Rule::OnlySame => {
            let mut matches = 0;
            for i in 0..n {
                if i != qi && sol[i] == sol[qi] {
                    matches += 1;
                }
            }
            matches == 1
        }
        Rule::ConsecIdent => {
            let mut pairs = 0;
            for i in 0..n.saturating_sub(1) {
                if sol[i] == sol[i + 1] {
                    pairs += 1;
                }
            }
            pairs == 1
        }
        Rule::OnlyOdd { answer } => {
            let mut matches = 0;
            for i in 0..n {
                if (i + 1) % 2 == 1 && sol[i] == answer {
                    matches += 1;
                }
            }
            matches == 1
        }
        Rule::Unique => count_letter(sol, sol[qi], n) == 1,
        Rule::EqualCount { answer } => {
            let ref_count = count_letter(sol, answer, n);
            LETTERS
                .iter()
                .any(|&l| l != answer && count_letter(sol, l, n) == ref_count)
        }
        _ => true,
    }
}

pub fn check_solvable(fp: &FlatPuzzle) -> bool {
    let n = fp.n;
    let mut answers = [None; MAX_N];
    let mut eliminated = [0u8; MAX_N];

    for _ in 0..n * 15 {
        if (0..n).all(|i| answers[i].is_some()) {
            return true;
        }

        if let Some(action) = find_action_fast(fp, &answers, &eliminated) {
            apply_action(&action, &mut answers, &mut eliminated);
            continue;
        }

        if let Some(action) = find_lookahead_action(fp, &answers, &eliminated) {
            apply_action(&action, &mut answers, &mut eliminated);
            continue;
        }

        return false;
    }
    false
}

pub fn validate_and_check(
    rules: &[Rule; MAX_N],
    solution: &[Answer; MAX_N],
    fp: &FlatPuzzle,
    n: usize,
) -> bool {
    // Cheap check first: no duplicate rules (same question text)
    for i in 0..n {
        for j in (i + 1)..n {
            if rules[i] == rules[j] {
                return false;
            }
        }
    }

    let opt_solution = to_optional(solution, n);
    for i in 0..n {
        if !evaluate(fp, i, solution[i], &opt_solution) {
            return false;
        }
    }

    let solutions = solve(fp, None, 2);
    if solutions.len() != 1 {
        return false;
    }

    check_solvable(fp)
}

// ── Build FlatPuzzle with options ──

pub fn build_flat_puzzle(
    rules: &[Rule; MAX_N],
    solution: &[Answer; MAX_N],
    n: usize,
    rng: &mut Rng,
) -> Option<FlatPuzzle> {
    let mut option_nums = [[NAN_VAL; 5]; MAX_N];
    let mut option_answers = [[0xFFu8; 5]; MAX_N];
    let mut option_claims = [[Claim::None; 5]; MAX_N];

    for qi in 0..n {
        let rule = &rules[qi];
        let correct_oi = solution[qi].idx();

        if rule.is_constrained() {
            for oi in 0..5 {
                option_answers[qi][oi] = oi as u8;
            }
            continue;
        }

        if matches!(rule, Rule::TrueStmt) {
            build_claims(
                qi,
                solution,
                n,
                rng,
                &mut option_claims[qi],
                &mut option_nums[qi],
            );
            continue;
        }

        let correct_val = compute_value(rule, qi, solution, n);

        match *rule {
            Rule::AnswerOf { question_index } => {
                option_answers[qi][correct_oi] = solution[question_index as usize] as u8;
                let correct_answer = solution[question_index as usize];
                let mut pool = [Answer::A; 4];
                let mut plen = 0;
                for &l in &LETTERS {
                    if l != correct_answer {
                        pool[plen] = l;
                        plen += 1;
                    }
                }
                rng.shuffle(&mut pool[..plen]);
                let mut di = 0;
                for oi in 0..5 {
                    if oi != correct_oi {
                        option_answers[qi][oi] = pool[di] as u8;
                        di += 1;
                    }
                }
            }
            Rule::ConsecIdent => {
                option_nums[qi][correct_oi] = correct_val;
                let distractors = pair_distractors(correct_val, n, rng);
                place_distractors(&distractors, &mut option_nums[qi], correct_oi);
            }
            Rule::LetterDist { .. } => {
                option_nums[qi][correct_oi] = correct_val;
                let mut pool = [0i16; 4];
                let mut plen = 0;
                for v in 0..5i16 {
                    if v != correct_val {
                        pool[plen] = v;
                        plen += 1;
                    }
                }
                rng.shuffle(&mut pool[..plen]);
                place_distractors(&pool, &mut option_nums[qi], correct_oi);
            }
            _ if is_counting_type(rule) => {
                option_nums[qi][correct_oi] = correct_val;
                let distractors =
                    count_distractors(correct_val as i32, count_max(rule, n) as i32, rng);
                place_distractors(&distractors, &mut option_nums[qi], correct_oi);
            }
            _ => {
                option_nums[qi][correct_oi] = correct_val;
                let distractors = positional_distractors(correct_val, n, rule, rng);
                place_distractors(&distractors, &mut option_nums[qi], correct_oi);
            }
        }
    }

    let (affected_by, global_indices) = FlatPuzzle::build_deps(rules, n);

    Some(FlatPuzzle {
        rules: *rules,
        option_nums,
        option_answers,
        option_claims,
        affected_by,
        global_indices,
        n,
    })
}

fn place_distractors(distractors: &[i16; 4], nums: &mut [i16; 5], correct_oi: usize) {
    let mut di = 0;
    for oi in 0..5 {
        if oi != correct_oi {
            nums[oi] = distractors[di];
            di += 1;
        }
    }
}

pub fn compute_value(rule: &Rule, qi: usize, sol: &[Answer; MAX_N], n: usize) -> i16 {
    match *rule {
        Rule::AnswerOf { question_index } => sol[question_index as usize] as i16,
        Rule::CountAnswer { answer } => count_letter(sol, answer, n) as i16,
        Rule::CountAnswerBefore {
            answer,
            before_index,
        } => (0..before_index as usize)
            .filter(|&i| sol[i] == answer)
            .count() as i16,
        Rule::CountAnswerAfter {
            answer,
            after_index,
        } => ((after_index as usize + 1)..n)
            .filter(|&i| sol[i] == answer)
            .count() as i16,
        Rule::CountVowel => (0..n).filter(|&i| sol[i].is_vowel()).count() as i16,
        Rule::CountConsonant => (0..n).filter(|&i| !sol[i].is_vowel()).count() as i16,
        Rule::MostCommonCount => {
            let c = letter_counts(sol, n);
            *c.iter().max().unwrap() as i16
        }
        Rule::ClosestAfter {
            after_index,
            answer,
        } => {
            for i in (after_index as usize + 1)..n {
                if sol[i] == answer {
                    return (i as i16) + 1;
                }
            }
            NONE_VAL
        }
        Rule::ClosestBefore {
            before_index,
            answer,
        } => {
            for i in (0..before_index as usize).rev() {
                if sol[i] == answer {
                    return (i as i16) + 1;
                }
            }
            NONE_VAL
        }
        Rule::FirstWith { answer } => {
            for i in 0..n {
                if sol[i] == answer {
                    return (i as i16) + 1;
                }
            }
            NONE_VAL
        }
        Rule::LastWith { answer } => {
            for i in (0..n).rev() {
                if sol[i] == answer {
                    return (i as i16) + 1;
                }
            }
            NONE_VAL
        }
        Rule::PrevSame => {
            for i in (0..qi).rev() {
                if sol[i] == sol[qi] {
                    return (i as i16) + 1;
                }
            }
            NONE_VAL
        }
        Rule::NextSame => {
            for i in (qi + 1)..n {
                if sol[i] == sol[qi] {
                    return (i as i16) + 1;
                }
            }
            NONE_VAL
        }
        Rule::OnlySame | Rule::SameAs => {
            for i in 0..n {
                if i != qi && sol[i] == sol[qi] {
                    return (i as i16) + 1;
                }
            }
            NONE_VAL
        }
        Rule::OnlyOdd { answer } => {
            for i in 0..n {
                if (i + 1) % 2 == 1 && sol[i] == answer {
                    return (i as i16) + 1;
                }
            }
            NONE_VAL
        }
        Rule::ConsecIdent => {
            for i in 0..n.saturating_sub(1) {
                if sol[i] == sol[i + 1] {
                    return i as i16;
                }
            }
            NONE_VAL
        }
        Rule::LetterDist {
            other_question_index,
        } => (sol[qi].idx() as i16 - sol[other_question_index as usize].idx() as i16).abs(),
        _ => NAN_VAL,
    }
}

fn is_counting_type(rule: &Rule) -> bool {
    matches!(
        rule,
        Rule::CountAnswer { .. }
            | Rule::CountAnswerBefore { .. }
            | Rule::CountAnswerAfter { .. }
            | Rule::CountVowel
            | Rule::CountConsonant
            | Rule::MostCommonCount
    )
}

fn count_max(rule: &Rule, n: usize) -> usize {
    match *rule {
        Rule::CountAnswerBefore { before_index, .. } => before_index as usize,
        Rule::CountAnswerAfter { after_index, .. } => n - after_index as usize - 1,
        _ => n,
    }
}

fn count_distractors(correct: i32, max: i32, rng: &mut Rng) -> [i16; 4] {
    let upper = max.max(4);
    let mut pool = [0i16; 32];
    let mut plen = 0;
    for i in 0..=upper {
        if i != correct {
            pool[plen] = i as i16;
            plen += 1;
        }
    }
    rng.shuffle(&mut pool[..plen]);
    let mut result = [0i16; 4];
    for i in 0..4.min(plen) {
        result[i] = pool[i];
    }
    result
}

fn positional_distractors(correct: i16, n: usize, rule: &Rule, rng: &mut Rng) -> [i16; 4] {
    let mut min_pos: i16 = 1;
    let mut max_pos = n as i16;

    match *rule {
        Rule::ClosestAfter { after_index, .. } | Rule::CountAnswerAfter { after_index, .. } => {
            min_pos = after_index as i16 + 2;
        }
        Rule::ClosestBefore { before_index, .. } | Rule::CountAnswerBefore { before_index, .. } => {
            max_pos = before_index as i16;
        }
        _ => {}
    }

    let mut pool = [0i16; 20];
    let mut plen = 0;
    for i in min_pos..=max_pos {
        if i != correct {
            pool[plen] = i;
            plen += 1;
        }
    }
    if correct != NONE_VAL {
        pool[plen] = NONE_VAL;
        plen += 1;
    }
    rng.shuffle(&mut pool[..plen]);
    let mut result = [0i16; 4];
    for i in 0..4.min(plen) {
        result[i] = pool[i];
    }
    result
}

fn pair_distractors(correct: i16, n: usize, rng: &mut Rng) -> [i16; 4] {
    let mut pool = [0i16; 16];
    let mut plen = 0;
    for i in 0..n.saturating_sub(1) {
        let v = i as i16;
        if v != correct {
            pool[plen] = v;
            plen += 1;
        }
    }
    if correct != NONE_VAL {
        pool[plen] = NONE_VAL;
        plen += 1;
    }
    rng.shuffle(&mut pool[..plen]);
    let mut result = [0i16; 4];
    for i in 0..4.min(plen) {
        result[i] = pool[i];
    }
    result
}

pub fn letter_counts(sol: &[Answer; MAX_N], n: usize) -> [i32; 5] {
    let mut counts = [0i32; 5];
    for i in 0..n {
        counts[sol[i].idx()] += 1;
    }
    counts
}

pub fn count_letter(sol: &[Answer; MAX_N], letter: Answer, n: usize) -> i32 {
    let mut c = 0i32;
    for i in 0..n {
        if sol[i] == letter {
            c += 1;
        }
    }
    c
}

// ── Claims for only_true_statement ──

fn build_claims(
    qi: usize,
    solution: &[Answer; MAX_N],
    n: usize,
    rng: &mut Rng,
    claims: &mut [Claim; 5],
    nums: &mut [i16; 5],
) {
    let target_oi = solution[qi].idx();
    let opt_sol = to_optional(solution, n);

    let true_claim = make_true_claim(solution, n, rng);
    claims[target_oi] = true_claim;
    nums[target_oi] = NAN_VAL;

    for oi in 0..5 {
        if oi == target_oi {
            continue;
        }
        let mut found = false;
        for _ in 0..30 {
            let fc = make_false_claim(solution, n, rng, &opt_sol);
            if fc != claims[target_oi] && (0..oi).all(|j| j == target_oi || claims[j] != fc) {
                claims[oi] = fc;
                found = true;
                break;
            }
        }
        if !found {
            claims[oi] = make_false_claim(solution, n, rng, &opt_sol);
        }
        nums[oi] = NAN_VAL;
    }
}

fn make_true_claim(sol: &[Answer; MAX_N], n: usize, rng: &mut Rng) -> Claim {
    match rng.int(0, 4) {
        0 => {
            let a = rng.pick(&LETTERS);
            Claim::CountAnswerEquals {
                answer: a,
                value: count_letter(sol, a, n) as u8,
            }
        }
        1 => Claim::CountConsonantEquals {
            value: (0..n).filter(|&i| !sol[i].is_vowel()).count() as u8,
        },
        2 => Claim::CountVowelEquals {
            value: (0..n).filter(|&i| sol[i].is_vowel()).count() as u8,
        },
        3 => {
            let a = rng.pick(&LETTERS);
            let ai = rng.int(0, n as i32 - 2) as u8;
            Claim::CountAnswerAfterEquals {
                answer: a,
                after_index: ai,
                value: ((ai as usize + 1)..n).filter(|&i| sol[i] == a).count() as u8,
            }
        }
        _ => {
            let a = rng.pick(&LETTERS);
            let bi = rng.int(1, n as i32 - 1) as u8;
            Claim::CountAnswerBeforeEquals {
                answer: a,
                before_index: bi,
                value: (0..bi as usize).filter(|&i| sol[i] == a).count() as u8,
            }
        }
    }
}

fn make_false_claim(
    sol: &[Answer; MAX_N],
    n: usize,
    rng: &mut Rng,
    opt_sol: &[Option<Answer>; MAX_N],
) -> Claim {
    for _ in 0..30 {
        let base = make_true_claim(sol, n, rng);
        let offset = rng.pick(&[-2i8, -1, 1, 2]);
        let base_value = match base {
            Claim::CountAnswerEquals { value, .. }
            | Claim::CountConsonantEquals { value }
            | Claim::CountVowelEquals { value }
            | Claim::CountAnswerAfterEquals { value, .. }
            | Claim::CountAnswerBeforeEquals { value, .. } => value as i8,
            Claim::None => continue,
        };
        let new_val = base_value + offset;
        if new_val < 0 || new_val > n as i8 {
            continue;
        }
        let fc = set_claim_value(base, new_val as u8);
        if !evaluate_claim(&fc, opt_sol, n) {
            return fc;
        }
    }
    Claim::CountAnswerEquals {
        answer: Answer::A,
        value: n as u8 + 1,
    }
}

fn set_claim_value(claim: Claim, value: u8) -> Claim {
    match claim {
        Claim::CountAnswerEquals { answer, .. } => Claim::CountAnswerEquals { answer, value },
        Claim::CountConsonantEquals { .. } => Claim::CountConsonantEquals { value },
        Claim::CountVowelEquals { .. } => Claim::CountVowelEquals { value },
        Claim::CountAnswerAfterEquals {
            answer,
            after_index,
            ..
        } => Claim::CountAnswerAfterEquals {
            answer,
            after_index,
            value,
        },
        Claim::CountAnswerBeforeEquals {
            answer,
            before_index,
            ..
        } => Claim::CountAnswerBeforeEquals {
            answer,
            before_index,
            value,
        },
        Claim::None => Claim::None,
    }
}
