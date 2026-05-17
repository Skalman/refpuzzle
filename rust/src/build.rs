use arrayvec::ArrayVec;

use crate::check_validity::check_question_against_solution;
use crate::deduce::{DeduceAction, DeduceResult, deduce};
use crate::evaluate::evaluate_claim;
use crate::lookahead::lookahead;
use crate::rng::Rng;
use crate::solve_brute::solve;
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
    pub repair_us: u64,
    pub deduce_calls: u32,
    pub deduce_results: u32,
    pub lookahead_calls: u32,
    pub lookahead_hits: u32,
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
        self.repair_us += other.repair_us;
        self.deduce_calls += other.deduce_calls;
        self.deduce_results += other.deduce_results;
        self.lookahead_calls += other.lookahead_calls;
        self.lookahead_hits += other.lookahead_hits;
        self.lookahead_us += other.lookahead_us;
    }

    pub fn print(&self) {
        let ok = self.attempts - self.fail_unique - self.fail_solve;
        eprintln!(
            "  attempts={} ok={} unique_fail={} solve_fail={} (zero_progress={}) | repair: {}/{}\n  solve={}ms hint={}ms | deduce: {} calls, {} results | lookahead: {} calls, {} hits, {}ms\n  repair: {}ms | repair_fail: no_candidates={} no_change={} changed_but_stuck={}",
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
            self.lookahead_hits,
            self.lookahead_us / 1000,
            self.repair_us / 1000,
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
        QuestionType::NoOtherHasAnswer => count_letter(sol, sol[qi], n) == 1,
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

pub fn phantom_mask(option_count: usize) -> u8 {
    (0b11111u8) & !((1u8 << option_count) - 1)
}

fn try_solve(
    fp: &FlatPuzzle,
    stats: &mut Stats,
    trace: bool,
) -> (bool, [Option<Answer>; MAX_N], [u8; MAX_N]) {
    let pm = phantom_mask(fp.option_count);
    try_solve_from(fp, [None; MAX_N], [pm; MAX_N], stats, trace)
}

fn try_solve_from(
    fp: &FlatPuzzle,
    mut answers: [Option<Answer>; MAX_N],
    mut eliminated: [u8; MAX_N],
    stats: &mut Stats,
    trace: bool,
) -> (bool, [Option<Answer>; MAX_N], [u8; MAX_N]) {
    let n = fp.n;
    let mut batch = 0u32;

    for _ in 0..n * 15 {
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
            if trace {
                trace_deduce_batch(batch, &drs, n);
                batch += 1;
            }
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

        if fp.option_count < 5 {
            return (false, answers, eliminated);
        }

        stats.lookahead_calls += 1;
        let t_la = std::time::Instant::now();
        let lr = lookahead(fp, &answers, &eliminated, 6, true);
        stats.lookahead_us += us(t_la);
        if let Some(lr) = lr {
            stats.lookahead_hits += 1;
            if trace {
                trace_lookahead(&lr);
            }
            eliminated[lr.eliminate_qi] |= 1 << lr.eliminate_oi;
            continue;
        }

        return (false, answers, eliminated);
    }
    (false, answers, eliminated)
}

// ── Trace helpers ──

#[cold]
#[inline(never)]
fn trace_deduce_batch(batch: u32, drs: &[DeduceResult], n: usize) {
    eprintln!("--- deduce batch {} ({} results) ---", batch, drs.len());
    let letters_lower = ['a', 'b', 'c', 'd', 'e'];
    for dr in drs {
        match dr.action {
            DeduceAction::Force { qi, answer } => {
                eprintln!("  {}{} {}", qi + 1, answer.as_char(), dr.rule.to_str());
            }
            DeduceAction::Eliminate { qi, oi } => {
                eprintln!("  {}{} {}", qi + 1, letters_lower[oi], dr.rule.to_str());
            }
            DeduceAction::EliminateMulti {
                question_mask,
                option_mask,
            } => {
                for i in 0..n {
                    if (question_mask >> i) & 1 == 1 {
                        for oi in 0..5usize {
                            if (option_mask >> oi) & 1 == 1 {
                                eprintln!("  {}{} {}", i + 1, letters_lower[oi], dr.rule.to_str());
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cold]
#[inline(never)]
fn trace_lookahead(lr: &crate::lookahead::LookaheadResult) {
    let letters_lower = ['a', 'b', 'c', 'd', 'e'];
    eprintln!(
        "--- lookahead: assume Q{}={} -> contradiction Q{} -> elim {}{} (chain={}) ---",
        lr.assumption_qi + 1,
        lr.assumption_answer.as_char(),
        lr.contradiction_qi + 1,
        lr.eliminate_qi + 1,
        letters_lower[lr.eliminate_oi],
        lr.chain.len()
    );
    for dr in &lr.chain {
        match dr.action {
            DeduceAction::Force { qi, answer } => {
                eprintln!("    {}{} {}", qi + 1, answer.as_char(), dr.rule.to_str());
            }
            DeduceAction::Eliminate { qi, oi } => {
                eprintln!("    {}{} {}", qi + 1, letters_lower[oi], dr.rule.to_str());
            }
            _ => {}
        }
    }
}

#[cold]
#[inline(never)]
fn trace_repair(qi: usize, before: &[i16; 5], after: &[i16; 5], probe_len: usize) {
    let fmt = |v: &[i16; 5]| -> String {
        v.iter()
            .map(|&x| {
                if x == NONE_VAL {
                    "null".to_string()
                } else if x == NAN_VAL {
                    "nan".to_string()
                } else {
                    x.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    eprintln!(
        "--- repair Q{}: [{}] -> [{}] probe={} ---",
        qi + 1,
        fmt(before),
        fmt(after),
        probe_len
    );
}

fn abs_diff(a: i16, b: i16) -> u16 {
    (a as i32 - b as i32).unsigned_abs() as u16
}

#[cfg(debug_assertions)]
fn validate_option_values(fp: &FlatPuzzle) {
    let n = fp.n;
    let oc = fp.option_count;
    for qi in 0..n {
        let qt = &fp.question_types[qi];
        for oi in 0..oc {
            if let Some(ref claim) = fp.option_claims[qi][oi] {
                if claim.value != NONE_VAL {
                    let pool = valid_values(&claim.question_type, n);
                    assert!(
                        pool.contains(&claim.value),
                        "Q{} option {}: claim {:?} value {} not in {:?}",
                        qi + 1,
                        LETTERS[oi].as_char(),
                        claim.question_type,
                        claim.value,
                        &*pool
                    );
                }
                continue;
            }
            let v = fp.option_nums[qi][oi];
            if v == NAN_VAL || v == NONE_VAL {
                continue;
            }
            let pool = valid_values(qt, n);
            assert!(
                pool.contains(&v),
                "Q{} option {}: type {:?} value {} not in {:?}",
                qi + 1,
                LETTERS[oi].as_char(),
                qt,
                v,
                &*pool
            );
        }
    }
}

fn valid_values(qt: &QuestionType, n: usize) -> ArrayVec<i16, 20> {
    let mut out = ArrayVec::new();
    match *qt {
        QuestionType::CountAnswer { .. }
        | QuestionType::CountVowel
        | QuestionType::CountConsonant
        | QuestionType::MostCommonCount => {
            for v in 0..=n as i16 {
                out.push(v);
            }
        }
        QuestionType::CountAnswerBefore { before_index, .. } => {
            for v in 0..=before_index as i16 {
                out.push(v);
            }
        }
        QuestionType::CountAnswerAfter { after_index, .. } => {
            for v in 0..=(n as i16 - 1 - after_index as i16) {
                out.push(v);
            }
        }
        QuestionType::AnswerOf { .. }
        | QuestionType::LeastCommon
        | QuestionType::MostCommon
        | QuestionType::NoOtherHasAnswer
        | QuestionType::LetterDist { .. }
        | QuestionType::EqualCount { .. } => {
            for v in 0..5i16 {
                out.push(v);
            }
        }
        QuestionType::ClosestAfter { after_index, .. } => {
            for v in (after_index as i16 + 1)..n as i16 {
                out.push(v);
            }
        }
        QuestionType::ClosestBefore { before_index, .. } => {
            for v in 0..before_index as i16 {
                out.push(v);
            }
        }
        QuestionType::OnlyOdd { .. } => {
            for v in (0..n as i16).step_by(2) {
                out.push(v);
            }
        }
        QuestionType::OnlyEven { .. } => {
            for v in (1..n as i16).step_by(2) {
                out.push(v);
            }
        }
        QuestionType::ConsecIdent => {
            for v in 0..n as i16 - 1 {
                out.push(v);
            }
        }
        QuestionType::TrueStmt | QuestionType::AnswerIsSelf => {}
        _ => {
            for v in 0..n as i16 {
                out.push(v);
            }
        }
    }
    out
}

pub fn validate_and_repair(
    question_types: &[QuestionType; MAX_N],
    solution: &[Answer; MAX_N],
    fp: &mut FlatPuzzle,
    n: usize,
    rng: &mut Rng,
    stats: &mut Stats,
    trace: bool,
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

    #[cfg(debug_assertions)]
    validate_option_values(fp);

    // Step 1: Can the engine solve it?
    if trace {
        eprintln!("--- solve ---");
    }
    let t0 = std::time::Instant::now();
    let (ok, stuck_answers, stuck_elim) = try_solve(fp, stats, trace);
    stats.hint_us += us(t0);
    if trace {
        let answered = (0..n).filter(|&i| stuck_answers[i].is_some()).count();
        eprintln!(
            "hint: {} {}/{}",
            if ok { "solved" } else { "stuck" },
            answered,
            n
        );
    }
    if ok {
        let t0 = std::time::Instant::now();
        let solutions = solve(fp, None, 2);
        stats.solve_us += us(t0);
        if trace {
            eprintln!("uniqueness: {} solution(s)", solutions.len());
        }
        if solutions.len() == 1 {
            return true;
        }
        stats.fail_unique += 1;
        return false;
    }

    // Step 3: Repair — tweak distractors and retry
    let repair_t0 = std::time::Instant::now();
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
            let probe = deduce(fp, &stuck_answers, &stuck_elim);
            if trace {
                trace_repair(qi, &before, &fp.option_nums[qi], probe.len());
            }
            if probe.is_empty() {
                continue;
            }
        } else {
            if trace {
                eprintln!("--- repair Q{}: no change ---", qi + 1);
            }
            continue;
        }

        // The tweak unblocked something — do a full solve
        if trace {
            eprintln!("--- solve (after repair) ---");
        }
        let t0 = std::time::Instant::now();
        let (ok, _, _) = if solved_before == 0 {
            try_solve(fp, stats, trace)
        } else {
            try_solve_from(fp, stuck_answers, stuck_elim, stats, trace)
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
        let (ok, _, _) = try_solve(fp, stats, trace);
        stats.hint_us += us(t0);
        repaired = ok;
    }

    stats.repair_us += us(repair_t0);

    if !repaired {
        return false;
    }

    #[cfg(debug_assertions)]
    validate_option_values(fp);

    // Step 4: After repair, verify uniqueness
    let t0 = std::time::Instant::now();
    let solutions = solve(fp, None, 2);
    stats.solve_us += us(t0);
    stats.repair_us += us(t0);
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
        if qt.has_identity_options() || matches!(qt, QuestionType::TrueStmt) {
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
            for &l in &LETTERS[..fp.option_count] {
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
        QuestionType::LetterDist { .. }
        | QuestionType::LeastCommon
        | QuestionType::MostCommon
        | QuestionType::NoOtherHasAnswer => {
            let correct_val = fp.option_nums[qi][correct_oi];
            // Find closest non-eliminated wrong option, replace with furthest value
            let mut best_oi = None;
            let mut best_dist = u16::MAX;
            for oi in 0..5 {
                if oi != correct_oi && (elim >> oi) & 1 == 0 {
                    let dist = abs_diff(fp.option_nums[qi][oi], correct_val);
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
                            let d = abs_diff(v, correct_val);
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
            let vals = valid_values(&qt, n);
            let max = vals.last().copied().unwrap_or(0);
            // Find the non-eliminated wrong option closest to correct — that's
            // the one the hint engine can't distinguish. Replace just that one.
            let mut best_oi = None;
            let mut best_dist = u16::MAX;
            for oi in 0..5 {
                if oi != correct_oi && (elim >> oi) & 1 == 0 {
                    let dist = abs_diff(fp.option_nums[qi][oi], correct_val);
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
                            let d = abs_diff(v, correct_val);
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
                    let dist = abs_diff(fp.option_nums[qi][oi], correct_val);
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
                    let d = abs_diff(j, correct_val);
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
                        abs_diff(v, correct_val)
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
                        abs_diff(v, correct_val)
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

pub fn fill_options(
    question_types: &[QuestionType; MAX_N],
    solution: &[Answer; MAX_N],
    n: usize,
    option_count: usize,
    rng: &mut Rng,
) -> Option<FlatPuzzle> {
    let mut option_nums = [[NAN_VAL; 5]; MAX_N];
    let mut option_answers = [[0xFFu8; 5]; MAX_N];
    let mut option_claims: [[Option<Claim>; 5]; MAX_N] = [[None; 5]; MAX_N];

    for qi in 0..n {
        let qt = &question_types[qi];
        let correct_oi = solution[qi].idx();

        if qt.has_identity_options() {
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
        let val_pool = valid_values(qt, n);
        let letters = &LETTERS[..option_count];

        match *qt {
            QuestionType::AnswerOf { question_index } => {
                option_answers[qi][correct_oi] = solution[question_index as usize] as u8;
                let correct_answer = solution[question_index as usize];
                let mut pool = [Answer::A; 4];
                let mut plen = 0;
                for &l in letters {
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
                for &l in letters {
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
                for &l in letters {
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
            QuestionType::ConsecIdent
            | QuestionType::LetterDist { .. }
            | QuestionType::OnlyOdd { .. }
            | QuestionType::OnlyEven { .. } => {
                option_nums[qi][correct_oi] = correct_val;
                let distractors = pick_distractors(&val_pool, correct_val, qi, qt, rng);
                place_distractors(&distractors, &mut option_nums[qi], correct_oi);
            }
            _ if is_counting_type(qt) => {
                option_nums[qi][correct_oi] = correct_val;
                let distractors = pick_distractors(&val_pool, correct_val, qi, qt, rng);
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
                let distractors = pick_distractors(&val_pool, correct_val, qi, qt, rng);
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
        option_count,
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

fn pick_distractors(
    vals: &ArrayVec<i16, 20>,
    correct: i16,
    qi: usize,
    qt: &QuestionType,
    rng: &mut Rng,
) -> [i16; 4] {
    let mut pool = [0i16; 20];
    let mut plen = 0;
    for &v in vals {
        if v != correct
            && !matches!(*qt, QuestionType::OnlySame | QuestionType::SameAs if v as usize == qi)
        {
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
        QuestionType::NoOtherHasAnswer => 1300,
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

const CLAIM_TYPES: &[QuestionTypeKind] = &[
    QuestionTypeKind::CountAnswer,
    QuestionTypeKind::CountConsonant,
    QuestionTypeKind::CountVowel,
    QuestionTypeKind::CountAnswerAfter,
    QuestionTypeKind::CountAnswerBefore,
    QuestionTypeKind::AnswerOf,
    QuestionTypeKind::FirstWith,
    QuestionTypeKind::LastWith,
    QuestionTypeKind::MostCommon,
    QuestionTypeKind::ClosestAfter,
    QuestionTypeKind::ClosestBefore,
    QuestionTypeKind::MostCommonCount,
    QuestionTypeKind::LeastCommon,
    QuestionTypeKind::EqualCount,
    QuestionTypeKind::ConsecIdent,
    QuestionTypeKind::OnlyOdd,
    QuestionTypeKind::OnlyEven,
    QuestionTypeKind::SameAsWhich,
];

fn try_make_claim(sol: &[Answer; MAX_N], qi: usize, n: usize, rng: &mut Rng) -> Option<Claim> {
    let kind = rng.pick(CLAIM_TYPES);
    match kind {
        QuestionTypeKind::CountAnswer => {
            let a = rng.pick(&LETTERS);
            Some(Claim {
                question_type: QuestionType::CountAnswer { answer: a },
                value: count_letter(sol, a, n) as i16,
            })
        }
        QuestionTypeKind::CountConsonant => Some(Claim {
            question_type: QuestionType::CountConsonant,
            value: (0..n).filter(|&i| !sol[i].is_vowel()).count() as i16,
        }),
        QuestionTypeKind::CountVowel => Some(Claim {
            question_type: QuestionType::CountVowel,
            value: (0..n).filter(|&i| sol[i].is_vowel()).count() as i16,
        }),
        QuestionTypeKind::CountAnswerAfter => {
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
        QuestionTypeKind::CountAnswerBefore => {
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
        QuestionTypeKind::AnswerOf => {
            let target = rng.int(0, n as i32 - 1) as u8;
            Some(Claim {
                question_type: QuestionType::AnswerOf {
                    question_index: target,
                },
                value: sol[target as usize].idx() as i16,
            })
        }
        QuestionTypeKind::FirstWith => {
            let a = rng.pick(&LETTERS);
            let first = (0..n).find(|&i| sol[i] == a)?;
            Some(Claim {
                question_type: QuestionType::FirstWith { answer: a },
                value: first as i16,
            })
        }
        QuestionTypeKind::LastWith => {
            let a = rng.pick(&LETTERS);
            let last = (0..n).rev().find(|&i| sol[i] == a)?;
            Some(Claim {
                question_type: QuestionType::LastWith { answer: a },
                value: last as i16,
            })
        }
        QuestionTypeKind::MostCommon => {
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
        QuestionTypeKind::ClosestAfter => {
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
        QuestionTypeKind::ClosestBefore => {
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
        QuestionTypeKind::MostCommonCount => {
            let counts = letter_counts(sol, n);
            let max = *counts.iter().max().unwrap_or(&0);
            Some(Claim {
                question_type: QuestionType::MostCommonCount,
                value: max as i16,
            })
        }
        QuestionTypeKind::LeastCommon => {
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
        QuestionTypeKind::EqualCount => {
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
        QuestionTypeKind::ConsecIdent => {
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
        QuestionTypeKind::OnlyOdd => {
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
        QuestionTypeKind::OnlyEven => {
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
        QuestionTypeKind::SameAsWhich => {
            let ref_qi = rng.int(0, n as i32 - 1) as u8;
            let ref_ans = sol[ref_qi as usize];
            let mut matches = ArrayVec::<usize, MAX_N>::new();
            for i in 0..n {
                if i != ref_qi as usize && i != qi && sol[i] == ref_ans {
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
        _ => None,
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
    let pool = valid_values(&claim.question_type, n);
    if pool.is_empty() {
        return None;
    }
    let wrong = rng.pick(&pool);
    if wrong == claim.value {
        return None;
    }
    Some(Claim {
        value: wrong,
        ..claim
    })
}
