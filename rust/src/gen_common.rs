use crate::evaluate::{evaluate, evaluate_claim};
use crate::hints::{apply_action, find_action_fast, find_lookahead_action};
use crate::rng::Rng;
use crate::solver::solve;
use crate::types::*;

use std::sync::atomic::{AtomicU64, Ordering};

pub static STATS: [AtomicU64; 9] = {
    #[allow(clippy::declare_interior_mutable_const)]
    const Z: AtomicU64 = AtomicU64::new(0);
    [Z; 9]
};
// 0=attempts, 1=fail_eval, 2=fail_unique, 3=fail_solve, 4=repair_attempts,
// 5=repair_ok, 6=solve_us, 7=hint_us, 8=solve_fail_zero_progress

pub fn print_stats() {
    let s = |i: usize| STATS[i].load(Ordering::Relaxed);
    let ok = s(0) - s(1) - s(2) - s(3);
    eprintln!(
        "  attempts={} ok={} eval_fail={} unique_fail={} solve_fail={} (zero_progress={}) | repair: {}/{} | solve={}ms hint={}ms",
        s(0),
        ok,
        s(1),
        s(2),
        s(3),
        s(8),
        s(5),
        s(4),
        s(6) / 1000,
        s(7) / 1000
    );
}

fn us(t: std::time::Instant) -> u64 {
    t.elapsed().as_micros() as u64
}

pub struct GenerateResult {
    pub rules: [Rule; MAX_N],
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

fn run_hint_engine(fp: &FlatPuzzle) -> (bool, [Option<Answer>; MAX_N], [u8; MAX_N]) {
    run_hint_engine_from(fp, [None; MAX_N], [0u8; MAX_N])
}

fn run_hint_engine_from(
    fp: &FlatPuzzle,
    mut answers: [Option<Answer>; MAX_N],
    mut eliminated: [u8; MAX_N],
) -> (bool, [Option<Answer>; MAX_N], [u8; MAX_N]) {
    let n = fp.n;

    for _ in 0..n * 30 {
        if (0..n).all(|i| answers[i].is_some()) {
            return (true, answers, eliminated);
        }

        if let Some(action) = find_action_fast(fp, &answers, &eliminated) {
            if let crate::hints::Action::Contradiction { .. } = action {
                return (false, answers, eliminated);
            }
            apply_action(&action, &mut answers, &mut eliminated);
            continue;
        }

        if let Some(action) = find_lookahead_action(fp, &answers, &eliminated) {
            apply_action(&action, &mut answers, &mut eliminated);
            continue;
        }

        return (false, answers, eliminated);
    }
    (false, answers, eliminated)
}

pub fn validate_and_check(
    rules: &[Rule; MAX_N],
    solution: &[Answer; MAX_N],
    fp: &mut FlatPuzzle,
    n: usize,
    rng: &mut Rng,
) -> bool {
    STATS[0].fetch_add(1, Ordering::Relaxed);

    for i in 0..n {
        for j in (i + 1)..n {
            if rules[i] == rules[j] {
                STATS[1].fetch_add(1, Ordering::Relaxed);
                return false;
            }
        }
    }

    let opt_solution = to_optional(solution, n);
    for i in 0..n {
        if !evaluate(fp, i, solution[i], &opt_solution) {
            STATS[1].fetch_add(1, Ordering::Relaxed);
            return false;
        }
    }

    let t0 = std::time::Instant::now();
    let solutions = solve(fp, None, 2);
    STATS[6].fetch_add(us(t0), Ordering::Relaxed);
    if solutions.len() != 1 {
        STATS[2].fetch_add(1, Ordering::Relaxed);
        return false;
    }

    let t0 = std::time::Instant::now();
    let (ok, stuck_answers, stuck_elim) = run_hint_engine(fp);
    STATS[7].fetch_add(us(t0), Ordering::Relaxed);
    if ok {
        return true;
    }
    let solved_before = (0..n).filter(|&i| stuck_answers[i].is_some()).count();
    STATS[3].fetch_add(1, Ordering::Relaxed);
    if solved_before == 0 {
        STATS[8].fetch_add(1, Ordering::Relaxed);
    }

    let candidates = rank_repair_candidates(fp, &stuck_answers);
    let mut maybe_solved = false;

    for &qi in &candidates {
        STATS[4].fetch_add(1, Ordering::Relaxed);
        repair_one_question(fp, qi, solution, &stuck_elim, rng);

        let t0 = std::time::Instant::now();
        let (ok, _, _) = if solved_before == 0 {
            run_hint_engine(fp)
        } else {
            run_hint_engine_from(fp, stuck_answers, stuck_elim)
        };
        STATS[7].fetch_add(us(t0), Ordering::Relaxed);

        if ok {
            maybe_solved = true;
            break;
        }
    }

    if !maybe_solved {
        let t0 = std::time::Instant::now();
        let (ok, _, _) = run_hint_engine(fp);
        STATS[7].fetch_add(us(t0), Ordering::Relaxed);
        maybe_solved = ok;
    }

    if maybe_solved {
        for i in 0..n {
            if !evaluate(fp, i, solution[i], &opt_solution) {
                return false;
            }
        }
        let t0 = std::time::Instant::now();
        let (ok, _, _) = run_hint_engine(fp);
        STATS[7].fetch_add(us(t0), Ordering::Relaxed);
        if !ok {
            return false;
        }
        let t0 = std::time::Instant::now();
        let solutions = solve(fp, None, 2);
        STATS[6].fetch_add(us(t0), Ordering::Relaxed);
        if solutions.len() == 1 {
            STATS[5].fetch_add(1, Ordering::Relaxed);
            return true;
        }
    }

    false
}

// ── Distractor repair ──
// When solvability fails, repair one question at a time with extreme-but-valid
// distractor values (0, max, edge positions). Prioritize counting rules since
// extreme counts are easy for the hint engine to disprove.

fn rank_repair_candidates(fp: &FlatPuzzle, stuck_answers: &[Option<Answer>; MAX_N]) -> Vec<usize> {
    let n = fp.n;
    let mut scored: Vec<(usize, u8)> = Vec::new();
    for qi in 0..n {
        if stuck_answers[qi].is_some() {
            continue;
        }
        let rule = fp.rules[qi];
        if rule.is_constrained() || matches!(rule, Rule::TrueStmt) {
            continue;
        }
        let score = match rule {
            _ if is_counting_type(&rule) => 3,
            Rule::AnswerOf { question_index } => {
                if stuck_answers[question_index as usize].is_some() {
                    2
                } else {
                    0
                }
            }
            Rule::LetterDist { question_index } => {
                if stuck_answers[question_index as usize].is_some() {
                    2
                } else {
                    0
                }
            }
            _ => 1, // positional, pairs, etc.
        };
        if score > 0 {
            scored.push((qi, score));
        }
    }
    scored.sort_by_key(|&(_, s)| std::cmp::Reverse(s));
    scored.into_iter().map(|(qi, _)| qi).collect()
}

pub fn repair_one_question(
    fp: &mut FlatPuzzle,
    qi: usize,
    solution: &[Answer; MAX_N],
    stuck_elim: &[u8; MAX_N],
    rng: &mut Rng,
) {
    let n = fp.n;
    let correct_oi = solution[qi].idx();
    let elim = stuck_elim[qi];
    let rule = fp.rules[qi];

    // Only repair non-eliminated wrong options — leave eliminated ones untouched
    // to preserve puzzle quality.

    match rule {
        Rule::AnswerOf { question_index } => {
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
                if oi != correct_oi && (elim >> oi) & 1 == 0 && di < plen {
                    fp.option_answers[qi][oi] = pool[di] as u8;
                    di += 1;
                }
            }
        }
        Rule::LetterDist { .. } => {
            let correct_val = fp.option_nums[qi][correct_oi];
            // Find closest non-eliminated wrong option, replace with furthest value
            let mut best_oi = None;
            let mut best_dist = u16::MAX;
            for oi in 0..5 {
                if oi != correct_oi && (elim >> oi) & 1 == 0 {
                    let dist = (fp.option_nums[qi][oi] - correct_val).unsigned_abs();
                    if dist < best_dist {
                        best_dist = dist;
                        best_oi = Some(oi);
                    }
                }
            }
            if let Some(oi) = best_oi {
                let old_val = fp.option_nums[qi][oi];
                let mut best_new = old_val;
                let mut best_new_dist = 0u16;
                for v in 0..5i16 {
                    if v != correct_val && v != old_val {
                        let mut in_use = false;
                        for k in 0..5 {
                            if k != oi && fp.option_nums[qi][k] == v {
                                in_use = true;
                            }
                        }
                        if !in_use {
                            let d = (v - correct_val).unsigned_abs();
                            if d > best_new_dist {
                                best_new_dist = d;
                                best_new = v;
                            }
                        }
                    }
                }
                fp.option_nums[qi][oi] = best_new;
            }
        }
        _ if is_counting_type(&rule) => {
            let correct_val = fp.option_nums[qi][correct_oi];
            let max = count_max(&rule, n) as i16;
            // Find the non-eliminated wrong option closest to correct — that's
            // the one the hint engine can't distinguish. Replace just that one.
            let mut best_oi = None;
            let mut best_dist = u16::MAX;
            for oi in 0..5 {
                if oi != correct_oi && (elim >> oi) & 1 == 0 {
                    let dist = (fp.option_nums[qi][oi] - correct_val).unsigned_abs();
                    if dist < best_dist {
                        best_dist = dist;
                        best_oi = Some(oi);
                    }
                }
            }
            if let Some(oi) = best_oi {
                // Replace with the furthest available value from correct
                let old_val = fp.option_nums[qi][oi];
                let mut best_new = old_val;
                let mut best_new_dist = 0u16;
                for v in 0..=max {
                    if v != correct_val && v != old_val {
                        let mut in_use = false;
                        for k in 0..5 {
                            if k != oi && fp.option_nums[qi][k] == v {
                                in_use = true;
                            }
                        }
                        if !in_use {
                            let d = (v - correct_val).unsigned_abs();
                            if d > best_new_dist {
                                best_new_dist = d;
                                best_new = v;
                            }
                        }
                    }
                }
                fp.option_nums[qi][oi] = best_new;
            }
        }
        _ => {
            // Positional, ConsecIdent, etc.: same strategy — find closest-to-correct
            // non-eliminated wrong option, replace with furthest available value.
            let correct_val = fp.option_nums[qi][correct_oi];
            let mut best_oi = None;
            let mut best_dist = u16::MAX;
            for oi in 0..5 {
                if oi != correct_oi && (elim >> oi) & 1 == 0 {
                    let v = fp.option_nums[qi][oi];
                    let dist = if v == NONE_VAL || correct_val == NONE_VAL {
                        1 // treat None as close (hard to distinguish)
                    } else {
                        (v - correct_val).unsigned_abs()
                    };
                    if dist < best_dist {
                        best_dist = dist;
                        best_oi = Some(oi);
                    }
                }
            }
            if let Some(oi) = best_oi {
                let (min_val, max_val) = match rule {
                    Rule::ConsecIdent => (0i16, (n as i16 - 2).max(0)),
                    _ => {
                        let min_p = match rule {
                            Rule::ClosestAfter { after_index, .. } => after_index as i16 + 1,
                            _ => 0,
                        };
                        let max_p = match rule {
                            Rule::ClosestBefore { before_index, .. } => before_index as i16 - 1,
                            _ => n as i16 - 1,
                        };
                        (min_p, max_p)
                    }
                };
                let old_val = fp.option_nums[qi][oi];
                let mut best_new = old_val;
                let mut best_new_dist = 0u16;
                // Try all values in range + NONE_VAL
                let candidates_iter = (min_val..=max_val).chain(std::iter::once(NONE_VAL));
                for v in candidates_iter {
                    if v == correct_val || v == old_val {
                        continue;
                    }
                    let mut in_use = false;
                    for k in 0..5 {
                        if k != oi && fp.option_nums[qi][k] == v {
                            in_use = true;
                        }
                    }
                    if in_use {
                        continue;
                    }
                    let d = if v == NONE_VAL || correct_val == NONE_VAL {
                        max_val.unsigned_abs() + 1 // treat None as far
                    } else {
                        (v - correct_val).unsigned_abs()
                    };
                    if d > best_new_dist {
                        best_new_dist = d;
                        best_new = v;
                    }
                }
                fp.option_nums[qi][oi] = best_new;
            }
        }
    }
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
            Rule::LeastCommon | Rule::MostCommon => {
                // Find the correct letter (least/most common) and build shuffled options
                let counts = letter_counts(solution, n);
                let target_count = if matches!(*rule, Rule::LeastCommon) {
                    *counts.iter().min().unwrap()
                } else {
                    *counts.iter().max().unwrap()
                };
                let correct_letter = LETTERS
                    .iter()
                    .find(|&&l| counts[l.idx()] == target_count)
                    .unwrap();
                option_answers[qi][correct_oi] = *correct_letter as u8;
                let mut pool = [Answer::A; 4];
                let mut plen = 0;
                for &l in &LETTERS {
                    if l != *correct_letter {
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
            Rule::OnlyOdd { .. } => {
                option_nums[qi][correct_oi] = correct_val;
                let distractors = odd_position_distractors(correct_val, n, rng);
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
                    return i as i16;
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
                    return i as i16;
                }
            }
            NONE_VAL
        }
        Rule::FirstWith { answer } => {
            for i in 0..n {
                if sol[i] == answer {
                    return i as i16;
                }
            }
            NONE_VAL
        }
        Rule::LastWith { answer } => {
            for i in (0..n).rev() {
                if sol[i] == answer {
                    return i as i16;
                }
            }
            NONE_VAL
        }
        Rule::PrevSame => {
            for i in (0..qi).rev() {
                if sol[i] == sol[qi] {
                    return i as i16;
                }
            }
            NONE_VAL
        }
        Rule::NextSame => {
            for i in (qi + 1)..n {
                if sol[i] == sol[qi] {
                    return i as i16;
                }
            }
            NONE_VAL
        }
        Rule::OnlySame | Rule::SameAs => {
            for i in 0..n {
                if i != qi && sol[i] == sol[qi] {
                    return i as i16;
                }
            }
            NONE_VAL
        }
        Rule::OnlyOdd { answer } => {
            for i in 0..n {
                if (i + 1) % 2 == 1 && sol[i] == answer {
                    return i as i16;
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
        Rule::LetterDist { question_index } => {
            (sol[qi].idx() as i16 - sol[question_index as usize].idx() as i16).abs()
        }
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
    result[..4.min(plen)].copy_from_slice(&pool[..4.min(plen)]);
    result
}

fn positional_distractors(correct: i16, n: usize, rule: &Rule, rng: &mut Rng) -> [i16; 4] {
    let mut min_pos: i16 = 0;
    let mut max_pos = n as i16 - 1;

    match *rule {
        Rule::ClosestAfter { after_index, .. } | Rule::CountAnswerAfter { after_index, .. } => {
            min_pos = after_index as i16 + 1;
        }
        Rule::ClosestBefore { before_index, .. } | Rule::CountAnswerBefore { before_index, .. } => {
            max_pos = before_index as i16 - 1;
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
    result[..4.min(plen)].copy_from_slice(&pool[..4.min(plen)]);
    result
}

fn odd_position_distractors(correct: i16, n: usize, rng: &mut Rng) -> [i16; 4] {
    let mut pool = [0i16; 16];
    let mut plen = 0;
    for i in (0..n as i16).step_by(2) {
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
    result[..4.min(plen)].copy_from_slice(&pool[..4.min(plen)]);
    result
}

fn pair_distractors(correct: i16, n: usize, rng: &mut Rng) -> [i16; 4] {
    let mut pool = [0i16; 16];
    let mut plen = 0;
    for i in 0..n.saturating_sub(1) as i16 {
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
    result[..4.min(plen)].copy_from_slice(&pool[..4.min(plen)]);
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
