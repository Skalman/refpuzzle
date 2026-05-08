use arrayvec::ArrayVec;

use crate::check_validity::check_question_against_solution;
use crate::deduce::{DeduceAction, deduce};
use crate::evaluate::evaluate_claim;
use crate::lookahead::lookahead;
use crate::rng::Rng;
use crate::solver::solve;
use crate::types::*;

#[derive(Default)]
pub struct Stats {
    pub attempts: u32,
    pub fail_unique: u32,
    pub fail_solve: u32,
    pub fail_solve_zero_progress: u32,
    pub repair_attempts: u32,
    pub repair_ok: u32,
    pub repair_fail_no_candidates: u32,
    pub repair_fail_no_change: u32,
    pub repair_fail_changed: u32,
    pub solve_us: u64,
    pub hint_us: u64,
    pub deduce_calls: u32,
    pub deduce_results: u32,
    pub lookahead_calls: u32,
    pub lookahead_us: u64,
}

impl Stats {
    pub fn merge(&mut self, other: &Stats) {
        self.attempts += other.attempts;
        self.fail_unique += other.fail_unique;
        self.fail_solve += other.fail_solve;
        self.fail_solve_zero_progress += other.fail_solve_zero_progress;
        self.repair_attempts += other.repair_attempts;
        self.repair_ok += other.repair_ok;
        self.repair_fail_no_candidates += other.repair_fail_no_candidates;
        self.repair_fail_no_change += other.repair_fail_no_change;
        self.repair_fail_changed += other.repair_fail_changed;
        self.solve_us += other.solve_us;
        self.hint_us += other.hint_us;
        self.deduce_calls += other.deduce_calls;
        self.deduce_results += other.deduce_results;
        self.lookahead_calls += other.lookahead_calls;
        self.lookahead_us += other.lookahead_us;
    }

    pub fn print(&self) {
        let ok = self.attempts - self.fail_unique - self.fail_solve;
        eprintln!(
            "  attempts={} ok={} unique_fail={} solve_fail={} (zero_progress={}) | repair: {}/{}\n  solve={}ms hint={}ms | deduce: {} calls, {} results | lookahead: {} calls, {}ms\n  repair_fail: no_candidates={} no_change={} changed_but_stuck={}",
            self.attempts,
            ok,
            self.fail_unique,
            self.fail_solve,
            self.fail_solve_zero_progress,
            self.repair_ok,
            self.repair_attempts,
            self.solve_us / 1000,
            self.hint_us / 1000,
            self.deduce_calls,
            self.deduce_results,
            self.lookahead_calls,
            self.lookahead_us / 1000,
            self.repair_fail_no_candidates,
            self.repair_fail_no_change,
            self.repair_fail_changed,
        );
    }
}

fn us(t: std::time::Instant) -> u64 {
    t.elapsed().as_micros() as u64
}

pub struct GenerateResult {
    pub question_types: [QuestionType; MAX_N],
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

pub fn solution_satisfies_type(
    qt: &QuestionType,
    qi: usize,
    sol: &[Answer; MAX_N],
    n: usize,
) -> bool {
    match *qt {
        QuestionType::OnlySame => {
            let mut matches = 0;
            for i in 0..n {
                if i != qi && sol[i] == sol[qi] {
                    matches += 1;
                }
            }
            matches == 1
        }
        QuestionType::ConsecIdent => {
            let mut pairs = 0;
            for i in 0..n.saturating_sub(1) {
                if sol[i] == sol[i + 1] {
                    pairs += 1;
                }
            }
            pairs == 1
        }
        QuestionType::OnlyOdd { answer } | QuestionType::OnlyEven { answer } => {
            let parity = match qt {
                QuestionType::OnlyOdd { .. } => 1,
                _ => 0,
            };
            let mut matches = 0;
            for i in 0..n {
                if (i + 1) % 2 == parity && sol[i] == answer {
                    matches += 1;
                }
            }
            matches == 1
        }
        QuestionType::Unique => count_letter(sol, sol[qi], n) == 1,
        QuestionType::EqualCount { .. } => true,
        QuestionType::SameAsWhich { question_index } => {
            let ref_ans = sol[question_index as usize];
            let has_match =
                (0..n).any(|j| j != qi && j != question_index as usize && sol[j] == ref_ans);
            let distractor_count = (0..n).filter(|&j| j != qi && sol[j] != ref_ans).count();
            has_match && distractor_count >= 4
        }
        _ => true,
    }
}

fn try_solve(fp: &FlatPuzzle, stats: &mut Stats) -> (bool, [Option<Answer>; MAX_N], [u8; MAX_N]) {
    try_solve_from(fp, [None; MAX_N], [0u8; MAX_N], stats)
}

fn try_solve_from(
    fp: &FlatPuzzle,
    mut answers: [Option<Answer>; MAX_N],
    mut eliminated: [u8; MAX_N],
    stats: &mut Stats,
) -> (bool, [Option<Answer>; MAX_N], [u8; MAX_N]) {
    let n = fp.n;

    for _ in 0..n * 30 {
        if (0..n).all(|i| answers[i].is_some()) {
            let valid = (0..n).all(|i| {
                crate::check_validity::check_question_against_solution(
                    fp,
                    i,
                    answers[i].unwrap(),
                    &answers,
                )
            });
            return (valid, answers, eliminated);
        }

        stats.deduce_calls += 1;
        let drs = deduce(fp, &answers, &eliminated);
        stats.deduce_results += drs.len() as u32;
        if !drs.is_empty() {
            for dr in &drs {
                match dr.action {
                    DeduceAction::Force { qi, answer } => {
                        if let Some(existing) = answers[qi] {
                            assert_eq!(
                                existing,
                                answer,
                                "conflicting forces for Q{}: {} vs {}",
                                qi + 1,
                                existing.as_char(),
                                answer.as_char()
                            );
                        } else {
                            eliminated[qi] = 0b11111 ^ (1 << answer.idx());
                            answers[qi] = Some(answer);
                        }
                    }
                    DeduceAction::Eliminate { qi, oi } => {
                        assert!(
                            answers[qi].is_none() || answers[qi] != Some(LETTERS[oi]),
                            "eliminating Q{} option {} but it's already forced to that answer",
                            qi + 1,
                            LETTERS[oi].as_char()
                        );
                        eliminated[qi] |= 1 << oi;
                    }
                    DeduceAction::EliminateMulti {
                        question_mask,
                        option_mask,
                    } => {
                        for i in 0..MAX_N {
                            if (question_mask >> i) & 1 == 1 {
                                eliminated[i] |= option_mask;
                            }
                        }
                    }
                }
            }
            continue;
        }

        stats.lookahead_calls += 1;
        let t_la = std::time::Instant::now();
        let lr = lookahead(fp, &answers, &eliminated, 6);
        stats.lookahead_us += us(t_la);
        if let Some(lr) = lr {
            eliminated[lr.eliminate_qi] |= 1 << lr.eliminate_oi;
            continue;
        }

        return (false, answers, eliminated);
    }
    (false, answers, eliminated)
}

pub fn validate_and_repair(
    question_types: &[QuestionType; MAX_N],
    solution: &[Answer; MAX_N],
    fp: &mut FlatPuzzle,
    n: usize,
    rng: &mut Rng,
    stats: &mut Stats,
) -> bool {
    stats.attempts += 1;

    // Assert construction correctness
    let opt_solution = to_optional(solution, n);
    for i in 0..n {
        if !check_question_against_solution(fp, i, solution[i], &opt_solution) {
            panic!(
                "BUG: check_question_against_solution failed for Q{} type={:?} answer={:?} solution={:?}",
                i + 1,
                question_types[i],
                solution[i],
                &solution[..n]
            );
        }
    }

    // Step 1: Can the engine solve it? (fast, rejects most bad puzzles)
    let t0 = std::time::Instant::now();
    let (ok, stuck_answers, stuck_elim) = try_solve(fp, stats);
    stats.hint_us += us(t0);
    if ok {
        // Step 2: Is the solution unique? (expensive, only for solvable puzzles)
        let t0 = std::time::Instant::now();
        let solutions = solve(fp, None, 2);
        stats.solve_us += us(t0);
        if solutions.len() == 1 {
            return true;
        }
        stats.fail_unique += 1;
        return false;
    }

    // Step 3: Repair — tweak distractors and retry
    let solved_before = (0..n).filter(|&i| stuck_answers[i].is_some()).count();
    stats.fail_solve += 1;
    if solved_before == 0 {
        stats.fail_solve_zero_progress += 1;
    }

    let candidates = rank_repair_candidates(fp, &stuck_answers);
    let mut repaired = false;
    let mut any_changed = false;

    for &qi in &candidates {
        stats.repair_attempts += 1;

        let before: [i16; 5] = fp.option_nums[qi];
        repair_one_question(fp, qi, solution, &stuck_elim, rng);
        if fp.option_nums[qi] != before {
            any_changed = true;
        }

        let t0 = std::time::Instant::now();
        let (ok, _, _) = if solved_before == 0 {
            try_solve(fp, stats)
        } else {
            try_solve_from(fp, stuck_answers, stuck_elim, stats)
        };
        stats.hint_us += us(t0);

        if ok {
            repaired = true;
            break;
        }
    }

    if !repaired {
        if candidates.is_empty() {
            stats.repair_fail_no_candidates += 1;
        } else if !any_changed {
            stats.repair_fail_no_change += 1;
        } else {
            stats.repair_fail_changed += 1;
        }
    }

    if !repaired {
        let t0 = std::time::Instant::now();
        let (ok, _, _) = try_solve(fp, stats);
        stats.hint_us += us(t0);
        repaired = ok;
    }

    if !repaired {
        return false;
    }

    // Step 4: After repair, verify uniqueness
    let t0 = std::time::Instant::now();
    let solutions = solve(fp, None, 2);
    stats.solve_us += us(t0);
    if solutions.len() == 1 {
        stats.repair_ok += 1;
        return true;
    }

    false
}

// ── Distractor repair ──
// When solvability fails, repair one question at a time with extreme-but-valid
// distractor values (0, max, edge positions). Prioritize counting types since
// extreme counts are easy for the hint engine to disprove.

fn rank_repair_candidates(
    fp: &FlatPuzzle,
    stuck_answers: &[Option<Answer>; MAX_N],
) -> ArrayVec<usize, MAX_N> {
    let n = fp.n;
    let mut scored: ArrayVec<(usize, u8), MAX_N> = ArrayVec::new();
    for qi in 0..n {
        if stuck_answers[qi].is_some() {
            continue;
        }
        let qt = fp.question_types[qi];
        if qt.is_constrained() || matches!(qt, QuestionType::TrueStmt) {
            continue;
        }
        let score = match qt {
            _ if is_counting_type(&qt) => 3,
            QuestionType::AnswerOf { question_index } => {
                if stuck_answers[question_index as usize].is_some() {
                    2
                } else {
                    0
                }
            }
            QuestionType::LetterDist { question_index } => {
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
    let qt = fp.question_types[qi];

    // Only repair non-eliminated wrong options — leave eliminated ones untouched
    // to preserve puzzle quality.

    match qt {
        QuestionType::AnswerOf { question_index } => {
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
        QuestionType::LetterDist { .. } => {
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
        _ if is_counting_type(&qt) => {
            let correct_val = fp.option_nums[qi][correct_oi];
            let max = count_max(&qt, n) as i16;
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
        QuestionType::SameAsWhich { question_index } => {
            let ref_ans = solution[question_index as usize];
            let correct_val = fp.option_nums[qi][correct_oi];
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
                for j in 0..n as i16 {
                    let ju = j as usize;
                    if ju == qi || ju == question_index as usize || solution[ju] == ref_ans {
                        continue;
                    }
                    if j == correct_val || j == old_val {
                        continue;
                    }
                    let mut in_use = false;
                    for k in 0..5 {
                        if k != oi && fp.option_nums[qi][k] == j {
                            in_use = true;
                        }
                    }
                    if in_use {
                        continue;
                    }
                    let d = (j - correct_val).unsigned_abs();
                    if d > best_new_dist {
                        best_new_dist = d;
                        best_new = j;
                    }
                }
                fp.option_nums[qi][oi] = best_new;
            }
        }
        _ => {
            // Positional, ConsecIdent, OnlyOdd, etc.: same strategy — find closest-to-correct
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
                let (min_val, max_val, step) = match qt {
                    QuestionType::ConsecIdent => (0i16, (n as i16 - 2).max(0), 1),
                    QuestionType::PrevSame => (0, qi as i16 - 1, 1),
                    QuestionType::NextSame => (qi as i16 + 1, n as i16 - 1, 1),
                    QuestionType::OnlyOdd { .. } => (0, n as i16 - 1, 2),
                    QuestionType::OnlyEven { .. } => (1, n as i16 - 1, 2),
                    QuestionType::EqualCount { .. } => (0, 4, 1),
                    _ => {
                        let min_p = match qt {
                            QuestionType::ClosestAfter { after_index, .. } => {
                                after_index as i16 + 1
                            }
                            _ => 0,
                        };
                        let max_p = match qt {
                            QuestionType::ClosestBefore { before_index, .. } => {
                                before_index as i16 - 1
                            }
                            _ => n as i16 - 1,
                        };
                        (min_p, max_p, 1)
                    }
                };
                let exclude_self = matches!(qt, QuestionType::OnlySame | QuestionType::SameAs);
                let exclude_ref = match qt {
                    QuestionType::EqualCount { answer } => answer.idx() as i16,
                    _ => -2,
                };
                let old_val = fp.option_nums[qi][oi];
                let mut best_new = old_val;
                let mut best_new_dist = 0u16;
                // Try all values in range + NONE_VAL
                let candidates_iter = (min_val..=max_val)
                    .step_by(step as usize)
                    .chain(std::iter::once(NONE_VAL));
                for v in candidates_iter {
                    if v == correct_val
                        || v == old_val
                        || (exclude_self && v == qi as i16)
                        || v == exclude_ref
                    {
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
    question_types: &[QuestionType; MAX_N],
    solution: &[Answer; MAX_N],
    n: usize,
    rng: &mut Rng,
) -> Option<FlatPuzzle> {
    let mut option_nums = [[NAN_VAL; 5]; MAX_N];
    let mut option_answers = [[0xFFu8; 5]; MAX_N];
    let mut option_claims: [[Option<Claim>; 5]; MAX_N] = [[None; 5]; MAX_N];

    for qi in 0..n {
        let qt = &question_types[qi];
        let correct_oi = solution[qi].idx();

        if qt.is_constrained() {
            for oi in 0..5 {
                option_answers[qi][oi] = oi as u8;
            }
            continue;
        }

        if matches!(qt, QuestionType::TrueStmt) {
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

        let correct_val = correct_option_value(qt, qi, solution, n);

        match *qt {
            QuestionType::AnswerOf { question_index } => {
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
            QuestionType::LeastCommon | QuestionType::MostCommon => {
                // Find the correct letter (least/most common) and build shuffled options
                let counts = letter_counts(solution, n);
                let target_count = if matches!(*qt, QuestionType::LeastCommon) {
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
            QuestionType::EqualCount { answer } => {
                option_nums[qi][correct_oi] = correct_val;
                let mut pool = [0i16; 4];
                let mut plen = 0;
                for &l in &LETTERS {
                    if l != answer && l.idx() as i16 != correct_val {
                        pool[plen] = l.idx() as i16;
                        plen += 1;
                    }
                }
                if correct_val != NONE_VAL {
                    pool[plen] = NONE_VAL;
                    plen += 1;
                }
                rng.shuffle(&mut pool[..plen]);
                place_distractors(&pool, &mut option_nums[qi], correct_oi);
            }
            QuestionType::ConsecIdent => {
                option_nums[qi][correct_oi] = correct_val;
                let distractors = pair_distractors(correct_val, n, rng);
                place_distractors(&distractors, &mut option_nums[qi], correct_oi);
            }
            QuestionType::LetterDist { .. } => {
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
            _ if is_counting_type(qt) => {
                option_nums[qi][correct_oi] = correct_val;
                let distractors =
                    count_distractors(correct_val as i32, count_max(qt, n) as i32, rng);
                place_distractors(&distractors, &mut option_nums[qi], correct_oi);
            }
            QuestionType::OnlyOdd { .. } | QuestionType::OnlyEven { .. } => {
                let start = if matches!(qt, QuestionType::OnlyOdd { .. }) {
                    0
                } else {
                    1
                };
                option_nums[qi][correct_oi] = correct_val;
                let distractors = parity_position_distractors(correct_val, n, start, rng);
                place_distractors(&distractors, &mut option_nums[qi], correct_oi);
            }
            QuestionType::SameAsWhich { question_index } => {
                let ref_ans = solution[question_index as usize];
                option_nums[qi][correct_oi] = correct_val;
                let mut pool = [0i16; MAX_N];
                let mut plen = 0;
                for j in 0..n {
                    if j != qi && j != question_index as usize && solution[j] != ref_ans {
                        pool[plen] = j as i16;
                        plen += 1;
                    }
                }
                rng.shuffle(&mut pool[..plen]);
                let mut distractors = [0i16; 4];
                distractors[..4.min(plen)].copy_from_slice(&pool[..4.min(plen)]);
                place_distractors(&distractors, &mut option_nums[qi], correct_oi);
            }
            _ => {
                option_nums[qi][correct_oi] = correct_val;
                let distractors = positional_distractors(correct_val, qi, n, qt, rng);
                place_distractors(&distractors, &mut option_nums[qi], correct_oi);
            }
        }
    }

    let (affected_by, global_indices) = FlatPuzzle::build_deps(question_types, n);

    Some(FlatPuzzle {
        question_types: *question_types,
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

pub fn correct_option_value(qt: &QuestionType, qi: usize, sol: &[Answer; MAX_N], n: usize) -> i16 {
    match *qt {
        QuestionType::AnswerOf { question_index } => sol[question_index as usize] as i16,
        QuestionType::CountAnswer { answer } => count_letter(sol, answer, n) as i16,
        QuestionType::CountAnswerBefore {
            answer,
            before_index,
        } => (0..before_index as usize)
            .filter(|&i| sol[i] == answer)
            .count() as i16,
        QuestionType::CountAnswerAfter {
            answer,
            after_index,
        } => ((after_index as usize + 1)..n)
            .filter(|&i| sol[i] == answer)
            .count() as i16,
        QuestionType::CountVowel => (0..n).filter(|&i| sol[i].is_vowel()).count() as i16,
        QuestionType::CountConsonant => (0..n).filter(|&i| !sol[i].is_vowel()).count() as i16,
        QuestionType::MostCommonCount => {
            let c = letter_counts(sol, n);
            *c.iter().max().unwrap() as i16
        }
        QuestionType::ClosestAfter {
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
        QuestionType::ClosestBefore {
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
        QuestionType::FirstWith { answer } => {
            for i in 0..n {
                if sol[i] == answer {
                    return i as i16;
                }
            }
            NONE_VAL
        }
        QuestionType::LastWith { answer } => {
            for i in (0..n).rev() {
                if sol[i] == answer {
                    return i as i16;
                }
            }
            NONE_VAL
        }
        QuestionType::PrevSame => {
            for i in (0..qi).rev() {
                if sol[i] == sol[qi] {
                    return i as i16;
                }
            }
            NONE_VAL
        }
        QuestionType::NextSame => {
            for i in (qi + 1)..n {
                if sol[i] == sol[qi] {
                    return i as i16;
                }
            }
            NONE_VAL
        }
        QuestionType::OnlySame | QuestionType::SameAs => {
            for i in 0..n {
                if i != qi && sol[i] == sol[qi] {
                    return i as i16;
                }
            }
            NONE_VAL
        }
        QuestionType::SameAsWhich { question_index } => {
            let ref_ans = sol[question_index as usize];
            for i in 0..n {
                if i != qi && i != question_index as usize && sol[i] == ref_ans {
                    return i as i16;
                }
            }
            NONE_VAL
        }
        QuestionType::OnlyOdd { answer } | QuestionType::OnlyEven { answer } => {
            let parity = match qt {
                QuestionType::OnlyOdd { .. } => 1,
                _ => 0,
            };
            for i in 0..n {
                if (i + 1) % 2 == parity && sol[i] == answer {
                    return i as i16;
                }
            }
            NONE_VAL
        }
        QuestionType::ConsecIdent => {
            for i in 0..n.saturating_sub(1) {
                if sol[i] == sol[i + 1] {
                    return i as i16;
                }
            }
            NONE_VAL
        }
        QuestionType::EqualCount { answer } => {
            let ref_count = count_letter(sol, answer, n);
            for &l in &LETTERS {
                if l != answer && count_letter(sol, l, n) == ref_count {
                    return l.idx() as i16;
                }
            }
            NONE_VAL
        }
        QuestionType::LetterDist { question_index } => {
            (sol[qi].idx() as i16 - sol[question_index as usize].idx() as i16).abs()
        }
        _ => NAN_VAL,
    }
}

fn is_counting_type(qt: &QuestionType) -> bool {
    matches!(
        qt,
        QuestionType::CountAnswer { .. }
            | QuestionType::CountAnswerBefore { .. }
            | QuestionType::CountAnswerAfter { .. }
            | QuestionType::CountVowel
            | QuestionType::CountConsonant
            | QuestionType::MostCommonCount
    )
}

fn count_max(qt: &QuestionType, n: usize) -> usize {
    match *qt {
        QuestionType::CountAnswerBefore { before_index, .. } => before_index as usize,
        QuestionType::CountAnswerAfter { after_index, .. } => n - after_index as usize - 1,
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

fn positional_distractors(
    correct: i16,
    qi: usize,
    n: usize,
    qt: &QuestionType,
    rng: &mut Rng,
) -> [i16; 4] {
    let mut min_pos: i16 = 0;
    let mut max_pos = n as i16 - 1;

    match *qt {
        QuestionType::ClosestAfter { after_index, .. }
        | QuestionType::CountAnswerAfter { after_index, .. } => {
            min_pos = after_index as i16 + 1;
        }
        QuestionType::ClosestBefore { before_index, .. }
        | QuestionType::CountAnswerBefore { before_index, .. } => {
            max_pos = before_index as i16 - 1;
        }
        QuestionType::PrevSame => {
            max_pos = qi as i16 - 1;
        }
        QuestionType::NextSame => {
            min_pos = qi as i16 + 1;
        }
        QuestionType::ConsecIdent => {
            max_pos = n as i16 - 2;
        }
        _ => {}
    }

    let mut pool = [0i16; 20];
    let mut plen = 0;
    for i in min_pos..=max_pos {
        if i != correct
            && !matches!(*qt, QuestionType::OnlySame | QuestionType::SameAs if i as usize == qi)
        {
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

fn parity_position_distractors(correct: i16, n: usize, start: usize, rng: &mut Rng) -> [i16; 4] {
    let mut pool = [0i16; 16];
    let mut plen = 0;
    for i in (start..n).step_by(2) {
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

fn claim_category(claim: &Claim) -> u16 {
    match claim.question_type {
        QuestionType::CountAnswer { answer } => 100 + answer.idx() as u16,
        QuestionType::CountConsonant => 200,
        QuestionType::CountVowel => 201,
        QuestionType::CountAnswerAfter { answer, .. } => 300 + answer.idx() as u16,
        QuestionType::CountAnswerBefore { answer, .. } => 400 + answer.idx() as u16,
        QuestionType::AnswerOf { question_index } => 500 + question_index as u16,
        QuestionType::FirstWith { answer } => 600 + answer.idx() as u16,
        QuestionType::LastWith { answer } => 700 + answer.idx() as u16,
        QuestionType::MostCommon => 800,
        QuestionType::ClosestAfter {
            answer,
            after_index,
        } => 900 + answer.idx() as u16 * 20 + after_index as u16,
        QuestionType::ClosestBefore {
            answer,
            before_index,
        } => 1000 + answer.idx() as u16 * 20 + before_index as u16,
        QuestionType::MostCommonCount => 1100,
        QuestionType::LeastCommon => 1200,
        QuestionType::Unique => 1300,
        QuestionType::EqualCount { answer } => 1400 + answer.idx() as u16,
        QuestionType::ConsecIdent => 1500,
        QuestionType::OnlyOdd { answer } => 1600 + answer.idx() as u16,
        QuestionType::OnlyEven { answer } => 1700 + answer.idx() as u16,
        QuestionType::SameAsWhich { question_index } => 1800 + question_index as u16,
        _ => 9999,
    }
}

fn build_claims(
    qi: usize,
    solution: &[Answer; MAX_N],
    n: usize,
    rng: &mut Rng,
    claims: &mut [Option<Claim>; 5],
    nums: &mut [i16; 5],
) {
    let target_oi = solution[qi].idx();
    let opt_sol = to_optional(solution, n);

    let true_claim = make_true_claim(solution, qi, n, rng);
    claims[target_oi] = Some(true_claim);
    nums[target_oi] = NAN_VAL;

    for oi in 0..5 {
        if oi == target_oi {
            continue;
        }
        let mut found = false;
        for _ in 0..30 {
            let fc = make_false_claim(solution, qi, n, rng, &opt_sol);
            let cat = claim_category(&fc);
            if cat != claim_category(claims[target_oi].as_ref().unwrap())
                && (0..oi).all(|j| {
                    j == target_oi || claims[j].as_ref().is_none_or(|c| claim_category(c) != cat)
                })
            {
                claims[oi] = Some(fc);
                found = true;
                break;
            }
        }
        if !found {
            claims[oi] = Some(make_false_claim(solution, qi, n, rng, &opt_sol));
        }
        nums[oi] = NAN_VAL;
    }
}

fn try_make_claim(sol: &[Answer; MAX_N], _qi: usize, n: usize, rng: &mut Rng) -> Option<Claim> {
    match rng.int(0, 18) {
        0 => {
            let a = rng.pick(&LETTERS);
            Some(Claim {
                question_type: QuestionType::CountAnswer { answer: a },
                value: count_letter(sol, a, n) as i16,
            })
        }
        1 => Some(Claim {
            question_type: QuestionType::CountConsonant,
            value: (0..n).filter(|&i| !sol[i].is_vowel()).count() as i16,
        }),
        2 => Some(Claim {
            question_type: QuestionType::CountVowel,
            value: (0..n).filter(|&i| sol[i].is_vowel()).count() as i16,
        }),
        3 => {
            let a = rng.pick(&LETTERS);
            let ai = rng.int(0, (n as i32 - 5).max(0)) as u8;
            Some(Claim {
                question_type: QuestionType::CountAnswerAfter {
                    answer: a,
                    after_index: ai,
                },
                value: ((ai as usize + 1)..n).filter(|&i| sol[i] == a).count() as i16,
            })
        }
        4 => {
            let a = rng.pick(&LETTERS);
            let bi = rng.int(4, n as i32 - 1) as u8;
            Some(Claim {
                question_type: QuestionType::CountAnswerBefore {
                    answer: a,
                    before_index: bi,
                },
                value: (0..bi as usize).filter(|&i| sol[i] == a).count() as i16,
            })
        }
        5 => {
            let target = rng.int(0, n as i32 - 1) as u8;
            Some(Claim {
                question_type: QuestionType::AnswerOf {
                    question_index: target,
                },
                value: sol[target as usize].idx() as i16,
            })
        }
        6 => {
            let a = rng.pick(&LETTERS);
            let first = (0..n).find(|&i| sol[i] == a)?;
            Some(Claim {
                question_type: QuestionType::FirstWith { answer: a },
                value: first as i16,
            })
        }
        7 => {
            let a = rng.pick(&LETTERS);
            let last = (0..n).rev().find(|&i| sol[i] == a)?;
            Some(Claim {
                question_type: QuestionType::LastWith { answer: a },
                value: last as i16,
            })
        }
        8 => {
            let counts = letter_counts(sol, n);
            let max = *counts.iter().max().unwrap_or(&0);
            let most: ArrayVec<Answer, 5> = LETTERS
                .iter()
                .filter(|&&a| counts[a.idx()] == max)
                .copied()
                .collect();
            if most.len() != 1 {
                return None;
            }
            Some(Claim {
                question_type: QuestionType::MostCommon,
                value: most[0].idx() as i16,
            })
        }
        9 => {
            let a = rng.pick(&LETTERS);
            let ai = rng.int(0, (n as i32 - 2).max(0)) as u8;
            let target = ((ai as usize + 1)..n).find(|&i| sol[i] == a)?;
            Some(Claim {
                question_type: QuestionType::ClosestAfter {
                    answer: a,
                    after_index: ai,
                },
                value: target as i16,
            })
        }
        10 => {
            let a = rng.pick(&LETTERS);
            let bi = rng.int(2, n as i32 - 1) as u8;
            let target = (0..bi as usize).rev().find(|&i| sol[i] == a)?;
            Some(Claim {
                question_type: QuestionType::ClosestBefore {
                    answer: a,
                    before_index: bi,
                },
                value: target as i16,
            })
        }
        11 => {
            let counts = letter_counts(sol, n);
            let max = *counts.iter().max().unwrap_or(&0);
            Some(Claim {
                question_type: QuestionType::MostCommonCount,
                value: max as i16,
            })
        }
        12 => {
            let counts = letter_counts(sol, n);
            let min = *counts.iter().min().unwrap_or(&0);
            if counts.iter().filter(|&&c| c == min).count() != 1 {
                return None;
            }
            let idx = counts.iter().position(|&c| c == min).unwrap();
            Some(Claim {
                question_type: QuestionType::LeastCommon,
                value: idx as i16,
            })
        }
        13 => {
            let counts = letter_counts(sol, n);
            if counts.iter().filter(|&&c| c == 1).count() != 1 {
                return None;
            }
            let idx = counts.iter().position(|&c| c == 1).unwrap();
            Some(Claim {
                question_type: QuestionType::Unique,
                value: idx as i16,
            })
        }
        14 => {
            let ref_ans = rng.pick(&LETTERS);
            let ref_count = count_letter(sol, ref_ans, n);
            let mut candidates = ArrayVec::<Answer, 5>::new();
            for &l in &LETTERS {
                if l != ref_ans && count_letter(sol, l, n) == ref_count {
                    candidates.push(l);
                }
            }
            if candidates.is_empty() {
                return None;
            }
            let target = rng.pick(&candidates);
            Some(Claim {
                question_type: QuestionType::EqualCount { answer: ref_ans },
                value: target.idx() as i16,
            })
        }
        15 => {
            let mut pair_idx: i16 = NONE_VAL;
            let mut pair_count = 0;
            for i in 0..n.saturating_sub(1) {
                if sol[i] == sol[i + 1] {
                    if pair_count == 0 {
                        pair_idx = i as i16;
                    }
                    pair_count += 1;
                }
            }
            if pair_count > 1 {
                return None;
            }
            Some(Claim {
                question_type: QuestionType::ConsecIdent,
                value: pair_idx,
            })
        }
        16 => {
            let a = rng.pick(&LETTERS);
            let mut found: i16 = NONE_VAL;
            let mut count = 0;
            for i in 0..n {
                if (i + 1) % 2 == 1 && sol[i] == a {
                    found = i as i16;
                    count += 1;
                }
            }
            if count != 1 {
                return None;
            }
            Some(Claim {
                question_type: QuestionType::OnlyOdd { answer: a },
                value: found,
            })
        }
        17 => {
            let a = rng.pick(&LETTERS);
            let mut found: i16 = NONE_VAL;
            let mut count = 0;
            for i in 0..n {
                if (i + 1) % 2 == 0 && sol[i] == a {
                    found = i as i16;
                    count += 1;
                }
            }
            if count != 1 {
                return None;
            }
            Some(Claim {
                question_type: QuestionType::OnlyEven { answer: a },
                value: found,
            })
        }
        _ => {
            let ref_qi = rng.int(0, n as i32 - 1) as u8;
            let ref_ans = sol[ref_qi as usize];
            let mut matches = ArrayVec::<usize, MAX_N>::new();
            for i in 0..n {
                if i != ref_qi as usize && i != _qi && sol[i] == ref_ans {
                    matches.push(i);
                }
            }
            if matches.is_empty() {
                return None;
            }
            let target = rng.pick(&matches);
            Some(Claim {
                question_type: QuestionType::SameAsWhich {
                    question_index: ref_qi,
                },
                value: target as i16,
            })
        }
    }
}

fn make_true_claim(sol: &[Answer; MAX_N], qi: usize, n: usize, rng: &mut Rng) -> Claim {
    for _ in 0..20 {
        if let Some(claim) = try_make_claim(sol, qi, n, rng) {
            debug_assert!(evaluate_claim(&claim, qi, &to_optional(sol, n), n));
            return claim;
        }
    }
    let a = rng.pick(&LETTERS);
    Claim {
        question_type: QuestionType::CountAnswer { answer: a },
        value: count_letter(sol, a, n) as i16,
    }
}

fn make_false_claim(
    sol: &[Answer; MAX_N],
    qi: usize,
    n: usize,
    rng: &mut Rng,
    opt_sol: &[Option<Answer>; MAX_N],
) -> Claim {
    for _ in 0..30 {
        let base = make_true_claim(sol, qi, n, rng);
        let fc = perturb_claim(base, n, rng);
        if let Some(fc) = fc
            && !evaluate_claim(&fc, qi, opt_sol, n)
        {
            return fc;
        }
    }
    Claim {
        question_type: QuestionType::CountAnswer { answer: Answer::A },
        value: n as i16 + 1,
    }
}

fn perturb_claim(claim: Claim, n: usize, rng: &mut Rng) -> Option<Claim> {
    match claim.question_type {
        QuestionType::CountAnswer { .. }
        | QuestionType::CountConsonant
        | QuestionType::CountVowel
        | QuestionType::CountAnswerAfter { .. }
        | QuestionType::CountAnswerBefore { .. }
        | QuestionType::MostCommonCount => {
            let offset = rng.pick(&[-2i8, -1, 1, 2]);
            let new_val = claim.value as i32 + offset as i32;
            if !(0..=n as i32).contains(&new_val) {
                return None;
            }
            Some(Claim {
                value: new_val as i16,
                ..claim
            })
        }
        QuestionType::AnswerOf { .. }
        | QuestionType::MostCommon
        | QuestionType::LeastCommon
        | QuestionType::Unique
        | QuestionType::EqualCount { .. } => {
            let wrong = rng.pick(&LETTERS);
            Some(Claim {
                value: wrong.idx() as i16,
                ..claim
            })
        }
        QuestionType::FirstWith { .. }
        | QuestionType::LastWith { .. }
        | QuestionType::ClosestAfter { .. }
        | QuestionType::ClosestBefore { .. }
        | QuestionType::ConsecIdent
        | QuestionType::OnlyOdd { .. }
        | QuestionType::OnlyEven { .. }
        | QuestionType::SameAsWhich { .. } => {
            let wrong = rng.int(0, n as i32 - 1) as i16;
            Some(Claim {
                value: wrong,
                ..claim
            })
        }
        _ => None,
    }
}
