use arrayvec::ArrayVec;
use serde_json::{Value, json};

use crate::check_answer::{check_answer, check_answers, check_claim_fast};
use crate::check_form;
use crate::construct::format_claim_qt;
use crate::deduce::{DeduceAction, DeduceResult, deduce};
use crate::format::format_type_tag;
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
    option_count: usize,
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
            has_match && distractor_count >= option_count - 1
        }
        _ => true,
    }
}

fn run_hint_engine(fp: &FlatPuzzle, stats: &mut Stats, trace: bool) -> (bool, State) {
    run_hint_engine_from(fp, fp.initial_state, stats, trace)
}

fn run_hint_engine_from(
    fp: &FlatPuzzle,
    mut state: State,
    stats: &mut Stats,
    trace: bool,
) -> (bool, State) {
    let n = fp.n;
    let mut batch = 0u32;

    for _ in 0..n * 15 {
        if (0..n).all(|i| state.answers[i].is_some()) {
            let valid = check_answers(fp, &state.answers);
            return (valid, state);
        }

        stats.deduce_calls += 1;
        let drs = deduce(fp, &state);
        stats.deduce_results += drs.len() as u32;
        if !drs.is_empty() {
            if trace {
                trace_deduce_batch(batch, &drs, n);
                batch += 1;
            }
            for dr in &drs {
                match dr.action {
                    DeduceAction::Force { qi, answer } => {
                        if let Some(existing) = state.answers[qi] {
                            assert_eq!(
                                existing,
                                answer,
                                "conflicting forces for Q{}: {} vs {}",
                                qi + 1,
                                existing.as_char(),
                                answer.as_char()
                            );
                        } else {
                            state.eliminated[qi] = 0b11111 ^ (1 << answer.idx());
                            state.answers[qi] = Some(answer);
                        }
                    }
                    DeduceAction::Eliminate { qi, oi } => {
                        assert!(
                            state.answers[qi].is_none() || state.answers[qi] != Some(LETTERS[oi]),
                            "eliminating Q{} option {} but it's already forced to that answer",
                            qi + 1,
                            LETTERS[oi].as_char()
                        );
                        state.eliminated[qi] |= 1 << oi;
                    }
                    DeduceAction::EliminateMulti {
                        question_mask,
                        option_mask,
                    } => {
                        for i in 0..MAX_N {
                            if (question_mask >> i) & 1 == 1 {
                                state.eliminated[i] |= option_mask;
                            }
                        }
                    }
                }
            }
            continue;
        }

        if fp.option_count < 5 {
            return (false, state);
        }

        stats.lookahead_calls += 1;
        let t_la = std::time::Instant::now();
        let lr = lookahead(fp, &state, 6, true);
        stats.lookahead_us += us(t_la);
        if let Some(lr) = lr {
            stats.lookahead_hits += 1;
            if trace {
                trace_lookahead(&lr);
            }
            state.eliminated[lr.eliminate_qi] |= 1 << lr.eliminate_oi;
            continue;
        }

        return (false, state);
    }
    (false, state)
}

// ── Trace helpers ──

fn action_to_json(action: &DeduceAction, rule: &str) -> Value {
    match *action {
        DeduceAction::Force { qi, answer } => {
            json!({"qi": qi, "answer": answer.as_char().to_string(), "rule": rule})
        }
        DeduceAction::Eliminate { qi, oi } => {
            json!({"qi": qi, "oi": oi, "rule": rule})
        }
        DeduceAction::EliminateMulti {
            question_mask,
            option_mask,
        } => {
            json!({"qi": -1, "qm": question_mask, "om": option_mask, "rule": rule})
        }
    }
}

#[cold]
#[inline(never)]
fn trace_deduce_batch(batch: u32, drs: &[DeduceResult], _n: usize) {
    let actions: Vec<Value> = drs
        .iter()
        .map(|dr| action_to_json(&dr.action, dr.rule.to_str()))
        .collect();
    eprintln!(
        "{}",
        json!({"t": "batch", "batch": batch, "actions": actions})
    );
}

#[cold]
#[inline(never)]
fn trace_lookahead(lr: &crate::lookahead::LookaheadResult) {
    let chain: Vec<Value> = lr
        .chain
        .iter()
        .map(|dr| action_to_json(&dr.action, dr.rule.to_str()))
        .collect();
    eprintln!(
        "{}",
        json!({
            "t": "lookahead",
            "assume": [lr.assumption_qi, lr.assumption_answer.as_char().to_string()],
            "contradiction": lr.contradiction_qi,
            "eliminate": [lr.eliminate_qi, lr.eliminate_oi],
            "chain": chain
        })
    );
}

#[cold]
#[inline(never)]
fn trace_repair(
    qi: usize,
    before: &[OptionValue; 5],
    after: &[OptionValue; 5],
    oc: usize,
    probe_len: usize,
) {
    let to_json = |v: &[OptionValue; 5]| -> Vec<Value> {
        v[..oc]
            .iter()
            .map(|&s| {
                if s.is_none() || s.is_unused() {
                    Value::Null
                } else {
                    json!(s.value())
                }
            })
            .collect()
    };
    eprintln!(
        "{}",
        json!({"t": "repair", "qi": qi, "before": to_json(before), "after": to_json(after), "probe": probe_len})
    );
}

#[cfg(debug_assertions)]
fn validate_option_values(fp: &FlatPuzzle) {
    let n = fp.n;
    let oc = fp.option_count;
    for qi in 0..n {
        let qt = &fp.question_types[qi];
        for oi in 0..oc {
            if let Some(claim) = fp.claim_at(qi, oi) {
                if !claim.value.is_none() {
                    // qi here is the TrueStmt's qi, which would be the wrong reference
                    // for position-dependent types. CLAIM_TYPES enforces (at compile
                    // time, below) that claims can't carry NextSame/PrevSame, so this
                    // qi is irrelevant for every kind that actually reaches here.
                    let pool = valid_values(&claim.question_type, qi, n, oc);
                    assert!(
                        pool.contains(&claim.value),
                        "Q{} option {}: claim {:?} value {:?} not in {:?}",
                        qi + 1,
                        LETTERS[oi].as_char(),
                        claim.question_type,
                        claim.value,
                        &*pool
                    );
                }
                continue;
            }
            let s = fp.options[qi][oi];
            if !s.is_num() {
                continue;
            }
            // Letter-typed and identity-options qts store letter/option indices, not
            // values from valid_values — skip them here.
            if matches!(
                qt,
                QuestionType::AnswerOf { .. }
                    | QuestionType::LeastCommon
                    | QuestionType::MostCommon
            ) || qt.has_identity_options()
            {
                continue;
            }
            let pool = valid_values(qt, qi, n, oc);
            assert!(
                pool.contains(&s),
                "Q{} option {}: type {:?} value {:?} not in {:?}",
                qi + 1,
                LETTERS[oi].as_char(),
                qt,
                s,
                &*pool
            );
        }
    }
}

fn valid_values(qt: &QuestionType, qi: usize, n: usize, oc: usize) -> ArrayVec<OptionValue, 20> {
    let mut out = ArrayVec::new();
    let mut push_num = |v: usize| out.push(OptionValue::num(v as u8));
    match *qt {
        QuestionType::CountAnswer { .. }
        | QuestionType::CountVowel
        | QuestionType::CountConsonant
        | QuestionType::MostCommonCount => {
            for v in 0..=n {
                push_num(v);
            }
        }
        QuestionType::CountAnswerBefore { before_index, .. } => {
            for v in 0..=usize::from(before_index) {
                push_num(v);
            }
        }
        QuestionType::CountAnswerAfter { after_index, .. } => {
            for v in 0..=(n - 1 - usize::from(after_index)) {
                push_num(v);
            }
        }
        QuestionType::AnswerOf { .. }
        | QuestionType::LeastCommon
        | QuestionType::MostCommon
        | QuestionType::NoOtherHasAnswer
        | QuestionType::LetterDist { .. } => {
            for v in 0..oc {
                push_num(v);
            }
        }
        QuestionType::EqualCount { answer } => {
            for v in 0..oc {
                if v != answer.idx() {
                    push_num(v);
                }
            }
            out.push(OptionValue::NONE);
        }
        QuestionType::ClosestAfter { after_index, .. } => {
            for v in (usize::from(after_index) + 1)..n {
                push_num(v);
            }
            out.push(OptionValue::NONE);
        }
        QuestionType::ClosestBefore { before_index, .. } => {
            for v in 0..usize::from(before_index) {
                push_num(v);
            }
            out.push(OptionValue::NONE);
        }
        QuestionType::NextSame => {
            for v in (qi + 1)..n {
                push_num(v);
            }
            out.push(OptionValue::NONE);
        }
        QuestionType::PrevSame => {
            for v in 0..qi {
                push_num(v);
            }
            out.push(OptionValue::NONE);
        }
        QuestionType::OnlyOdd { .. } => {
            for v in (0..n).step_by(2) {
                push_num(v);
            }
            out.push(OptionValue::NONE);
        }
        QuestionType::OnlyEven { .. } => {
            for v in (1..n).step_by(2) {
                push_num(v);
            }
            out.push(OptionValue::NONE);
        }
        QuestionType::ConsecIdent => {
            for v in 0..n.saturating_sub(1) {
                push_num(v);
            }
            out.push(OptionValue::NONE);
        }
        QuestionType::TrueStmt | QuestionType::AnswerIsSelf => {}
        QuestionType::SameAs | QuestionType::OnlySame => {
            for v in 0..n {
                if v != qi {
                    push_num(v);
                }
            }
            out.push(OptionValue::NONE);
        }
        _ => {
            for v in 0..n {
                push_num(v);
            }
            out.push(OptionValue::NONE);
        }
    }
    out
}

fn assert_accepted(
    fp: &FlatPuzzle,
    solution: &[Answer; MAX_N],
    n: usize,
    brute_count: usize,
    label: &str,
) {
    assert_eq!(
        brute_count, 1,
        "BUG [{label}]: expected 1 solution, got {brute_count}"
    );
    let fe = check_form::check_form(fp, Some(&solution[..n]));
    assert!(
        fe.is_empty(),
        "BUG [{label}]: form errors: {}",
        fe.iter()
            .map(|e| format!("Q{}: {}", e.qi + 1, e.message))
            .collect::<Vec<_>>()
            .join(", ")
    );
}

#[allow(clippy::too_many_arguments)]
pub fn validate_and_repair(
    question_types: &[QuestionType; MAX_N],
    solution: &[Answer; MAX_N],
    fp: &mut FlatPuzzle,
    n: usize,
    rng: &mut Rng,
    stats: &mut Stats,
    trace: bool,
    label: &str,
) -> bool {
    stats.attempts += 1;

    // Assert construction correctness
    let opt_solution = to_optional(solution, n);
    for i in 0..n {
        if !check_answer(
            fp,
            State {
                answers: opt_solution,
                eliminated: [fp.phantom_mask(); MAX_N],
            },
            i,
        )
        .is_valid()
        {
            panic!(
                "BUG: check_answer failed for Q{} type={:?} answer={:?} solution={:?}",
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
        eprintln!("{}", json!({"t": "solve", "label": "initial"}));
    }
    let t0 = std::time::Instant::now();
    let (ok, stuck_state) = run_hint_engine(fp, stats, trace);
    stats.hint_us += us(t0);
    if trace {
        let answered = (0..n).filter(|&i| stuck_state.answers[i].is_some()).count();
        eprintln!(
            "{}",
            json!({"t": "hint", "solved": ok, "answered": answered, "n": n})
        );
    }
    if ok {
        let t0 = std::time::Instant::now();
        let solutions = solve(fp, None, 2);
        stats.solve_us += us(t0);
        if trace {
            eprintln!(
                "{}",
                json!({"t": "uniqueness", "solutions": solutions.len()})
            );
        }
        assert_accepted(fp, solution, n, solutions.len(), label);
        return true;
    }

    // Step 3: Repair — tweak distractors and retry
    let repair_t0 = std::time::Instant::now();
    let solved_before = (0..n).filter(|&i| stuck_state.answers[i].is_some()).count();
    stats.fail_solve += 1;
    if solved_before == 0 {
        stats.fail_solve_zero_progress += 1;
    }

    let candidates = rank_repair_candidates(fp, &stuck_state.answers);
    let mut repaired = false;
    let mut any_changed = false;

    for &qi in &candidates {
        stats.repair_attempts += 1;

        let before: [OptionValue; 5] = fp.options[qi];
        repair_one_question(fp, qi, solution, &stuck_state.eliminated, rng);
        if fp.options[qi] != before {
            any_changed = true;
            let probe = deduce(fp, &stuck_state);
            if trace {
                trace_repair(qi, &before, &fp.options[qi], fp.option_count, probe.len());
            }
            if probe.is_empty() {
                continue;
            }
        } else {
            if trace {
                eprintln!("{}", json!({"t": "repair", "qi": qi, "changed": false}));
            }
            continue;
        }

        // The tweak unblocked something — do a full solve
        if trace {
            eprintln!("{}", json!({"t": "solve", "label": "after_repair"}));
        }
        let t0 = std::time::Instant::now();
        let (ok, _) = if solved_before == 0 {
            run_hint_engine(fp, stats, trace)
        } else {
            run_hint_engine_from(fp, stuck_state, stats, trace)
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

    stats.repair_us += us(repair_t0);

    if trace {
        eprintln!("{}", json!({"t": "solve", "label": "final"}));
    }
    let t0 = std::time::Instant::now();
    let (ok, _) = run_hint_engine(fp, stats, trace);
    stats.hint_us += us(t0);
    if !ok {
        return false;
    }

    #[cfg(debug_assertions)]
    validate_option_values(fp);

    // Step 4: After repair, verify uniqueness
    let t0 = std::time::Instant::now();
    let solutions = solve(fp, None, 2);
    stats.solve_us += us(t0);
    stats.repair_us += us(t0);
    if trace {
        eprintln!(
            "{}",
            json!({"t": "uniqueness", "solutions": solutions.len()})
        );
    }
    assert_accepted(fp, solution, n, solutions.len(), label);
    stats.repair_ok += 1;
    true
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
    let oc = fp.option_count;
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
            for oi in 0..oc {
                if oi != correct_oi && (elim >> oi) & 1 == 0 && di < plen {
                    fp.options[qi][oi] = OptionValue::num(pool[di] as u8);
                    di += 1;
                }
            }
        }
        QuestionType::LetterDist { .. } | QuestionType::NoOtherHasAnswer => {
            // All values are numeric letter indices in `0..oc` — no NONE.
            let correct_val = fp.options[qi][correct_oi].value();
            let best_oi = (0..oc)
                .filter(|&oi| oi != correct_oi && (elim >> oi) & 1 == 0)
                .min_by_key(|&oi| fp.options[qi][oi].value().abs_diff(correct_val));
            if let Some(oi) = best_oi {
                let old_val = fp.options[qi][oi].value();
                let mut best_new = old_val;
                let mut best_new_dist = 0u8;
                for v in 0..oc as u8 {
                    if v == correct_val || v == old_val {
                        continue;
                    }
                    let in_use =
                        (0..oc).any(|k| k != oi && fp.options[qi][k] == OptionValue::num(v));
                    if !in_use {
                        let d = v.abs_diff(correct_val);
                        if d > best_new_dist {
                            best_new_dist = d;
                            best_new = v;
                        }
                    }
                }
                fp.options[qi][oi] = OptionValue::num(best_new);
            }
        }
        QuestionType::LeastCommon | QuestionType::MostCommon => {
            // Stored as letter indices in `0..oc` — pure u8.
            let correct_val = fp.options[qi][correct_oi].value();
            let best_oi = (0..oc)
                .filter(|&oi| oi != correct_oi && (elim >> oi) & 1 == 0)
                .min_by_key(|&oi| fp.options[qi][oi].value().abs_diff(correct_val));
            if let Some(oi) = best_oi {
                let old_val = fp.options[qi][oi].value();
                let mut best_new = old_val;
                let mut best_new_dist = 0u8;
                for v in 0..oc as u8 {
                    if v == correct_val || v == old_val {
                        continue;
                    }
                    let in_use = (0..oc).any(|k| k != oi && fp.options[qi][k].value() == v);
                    if !in_use {
                        let d = v.abs_diff(correct_val);
                        if d > best_new_dist {
                            best_new_dist = d;
                            best_new = v;
                        }
                    }
                }
                fp.options[qi][oi] = OptionValue::num(best_new);
            }
        }
        _ if is_counting_type(&qt) => {
            // Count values are pure u8 in `0..=n`; pool has no NONE.
            let correct_val = fp.options[qi][correct_oi].value();
            let vals = valid_values(&qt, qi, n, oc);
            let max = vals.last().copied().map_or(0, |v| v.value());
            // Find the non-eliminated wrong option closest to correct — that's
            // the one the hint engine can't distinguish. Replace just that one.
            let best_oi = (0..oc)
                .filter(|&oi| oi != correct_oi && (elim >> oi) & 1 == 0)
                .min_by_key(|&oi| fp.options[qi][oi].value().abs_diff(correct_val));
            if let Some(oi) = best_oi {
                // Replace with the furthest available value from correct.
                let old_val = fp.options[qi][oi].value();
                let mut best_new = old_val;
                let mut best_new_dist = 0u8;
                for v in 0..=max {
                    if v == correct_val || v == old_val {
                        continue;
                    }
                    let in_use =
                        (0..oc).any(|k| k != oi && fp.options[qi][k] == OptionValue::num(v));
                    if !in_use {
                        let d = v.abs_diff(correct_val);
                        if d > best_new_dist {
                            best_new_dist = d;
                            best_new = v;
                        }
                    }
                }
                fp.options[qi][oi] = OptionValue::num(best_new);
            }
        }
        QuestionType::SameAsWhich { question_index } => {
            // The correct value is a question index (guaranteed numeric here:
            // SameAsWhich is only generated when a match exists). Distractors
            // are also numeric question indices.
            let ref_ans = solution[question_index as usize];
            let correct_val = fp.options[qi][correct_oi].value();
            let best_oi = (0..oc)
                .filter(|&oi| oi != correct_oi && (elim >> oi) & 1 == 0)
                .min_by_key(|&oi| fp.options[qi][oi].value().abs_diff(correct_val));
            if let Some(oi) = best_oi {
                let old_val = fp.options[qi][oi].value();
                let mut best_new = old_val;
                let mut best_new_dist = 0u8;
                for j in 0..n as u8 {
                    let ju = j as usize;
                    if ju == qi || ju == question_index as usize || solution[ju] == ref_ans {
                        continue;
                    }
                    if j == correct_val || j == old_val {
                        continue;
                    }
                    let in_use =
                        (0..oc).any(|k| k != oi && fp.options[qi][k] == OptionValue::num(j));
                    if in_use {
                        continue;
                    }
                    let d = j.abs_diff(correct_val);
                    if d > best_new_dist {
                        best_new_dist = d;
                        best_new = j;
                    }
                }
                fp.options[qi][oi] = OptionValue::num(best_new);
            }
        }
        _ => {
            // Positional, ConsecIdent, OnlyOdd, etc.: same strategy — find
            // closest-to-correct non-eliminated wrong option, then replace it
            // with the furthest available value from `valid_values`.
            //
            // Distance: numeric-vs-numeric is the usual `abs_diff`. Anything
            // involving NONE is by convention "close" (=1) when picking the
            // option to replace (NONE is hard to distinguish), and "far"
            // (=pool_max+1) when picking its replacement (NONE makes a
            // visibly different distractor).
            let correct_val = fp.options[qi][correct_oi];
            let dist_close = |a: OptionValue, b: OptionValue| match (a.is_num(), b.is_num()) {
                (true, true) => a.value().abs_diff(b.value()),
                _ => 1,
            };
            let best_oi = (0..oc)
                .filter(|&oi| oi != correct_oi && (elim >> oi) & 1 == 0)
                .min_by_key(|&oi| dist_close(fp.options[qi][oi], correct_val));
            if let Some(oi) = best_oi {
                let pool = valid_values(&qt, qi, n, oc);
                let pool_max = pool
                    .iter()
                    .filter(|v| v.is_num())
                    .map(|v| v.value())
                    .max()
                    .unwrap_or(0);
                let old_val = fp.options[qi][oi];
                let mut best_new = old_val;
                let mut best_new_dist = 0u8;
                for &v in &pool {
                    if v == correct_val || v == old_val {
                        continue;
                    }
                    // SameAs: same-answer questions would be alternate correct answers.
                    if matches!(qt, QuestionType::SameAs)
                        && v.is_num()
                        && solution[usize::from(v.value())] == solution[qi]
                    {
                        continue;
                    }
                    let in_use = (0..oc).any(|k| k != oi && fp.options[qi][k] == v);
                    if in_use {
                        continue;
                    }
                    let d = match (v.is_num(), correct_val.is_num()) {
                        (true, true) => v.value().abs_diff(correct_val.value()),
                        _ => pool_max + 1, // NONE: visibly far
                    };
                    if d > best_new_dist {
                        best_new_dist = d;
                        best_new = v;
                    }
                }
                fp.options[qi][oi] = best_new;
            }
        }
    }
}

// ── Build FlatPuzzle with options ──

#[allow(clippy::too_many_arguments)]
fn fill_one_question(
    qt: &QuestionType,
    qi: usize,
    solution: &[Answer; MAX_N],
    n: usize,
    option_count: usize,
    rng: &mut Rng,
    slots: &mut [OptionValue; 5],
    true_stmt_question_types: &mut Option<[QuestionType; 5]>,
) {
    let correct_oi = solution[qi].idx();

    if qt.has_identity_options() {
        if matches!(qt, QuestionType::NoOtherHasAnswer) {
            let self_ans = solution[qi];
            if (0..n).any(|j| j != qi && solution[j] == self_ans) {
                panic!(
                    "fill_one_question: NoOtherHasAnswer at qi={qi} but another question shares answer {self_ans:?} — missing upstream guard"
                );
            }
        }
        for oi in 0..5 {
            slots[oi] = OptionValue::num(oi as u8);
        }
        return;
    }

    if matches!(qt, QuestionType::TrueStmt) {
        build_claims(
            qi,
            solution,
            n,
            rng,
            slots,
            true_stmt_question_types,
            option_count,
        );
        return;
    }

    let correct_val = correct_option_value(qt, qi, solution, n);
    let val_pool = valid_values(qt, qi, n, option_count);
    let letters = &LETTERS[..option_count];

    match *qt {
        QuestionType::AnswerOf { question_index } => {
            slots[correct_oi] = OptionValue::num(solution[question_index as usize] as u8);
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
            for oi in 0..option_count {
                if oi != correct_oi {
                    slots[oi] = OptionValue::num(pool[di] as u8);
                    di += 1;
                }
            }
        }
        QuestionType::LeastCommon | QuestionType::MostCommon => {
            let counts = letter_counts(solution, n);
            let opt_counts: Vec<i32> = letters.iter().map(|l| counts[l.idx()]).collect();
            let target_count = if matches!(*qt, QuestionType::LeastCommon) {
                *opt_counts.iter().min().unwrap()
            } else {
                *opt_counts.iter().max().unwrap()
            };
            if opt_counts.iter().filter(|&&c| c == target_count).count() != 1 {
                panic!(
                    "fill_one_question: {qt:?} at qi={qi} but two letters tie for the extreme count — missing upstream guard"
                );
            }
            let correct_letter = LETTERS
                .iter()
                .find(|&&l| counts[l.idx()] == target_count)
                .unwrap();
            slots[correct_oi] = OptionValue::num(*correct_letter as u8);
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
            for oi in 0..option_count {
                if oi != correct_oi {
                    slots[oi] = OptionValue::num(pool[di] as u8);
                    di += 1;
                }
            }
        }
        QuestionType::EqualCount { answer } => {
            slots[correct_oi] = correct_val;
            let mut pool = [OptionValue::UNUSED; 4];
            let mut plen = 0;
            for &l in letters {
                let lv = OptionValue::num(l.idx() as u8);
                if l != answer && lv != correct_val {
                    pool[plen] = lv;
                    plen += 1;
                }
            }
            if !correct_val.is_none() {
                pool[plen] = OptionValue::NONE;
                plen += 1;
            }
            rng.shuffle(&mut pool[..plen]);
            place_distractors(&pool, slots, correct_oi);
        }
        QuestionType::OnlyOdd { answer } | QuestionType::OnlyEven { answer } => {
            let parity = if matches!(*qt, QuestionType::OnlyOdd { .. }) {
                1usize
            } else {
                0usize
            };
            let matches = (0..n)
                .filter(|&i| (i + 1) % 2 == parity && solution[i] == answer)
                .count();
            if matches > 1 {
                panic!(
                    "fill_one_question: {qt:?} at qi={qi} but more than one same-parity question has answer {answer:?} — missing upstream guard"
                );
            }
            slots[correct_oi] = correct_val;
            let distractors = pick_distractors(&val_pool, correct_val, qi, qt, rng);
            place_distractors(&distractors, slots, correct_oi);
        }
        QuestionType::ConsecIdent => {
            let pairs = (0..n.saturating_sub(1))
                .filter(|&i| solution[i] == solution[i + 1])
                .count();
            if pairs > 1 {
                panic!(
                    "fill_one_question: ConsecIdent at qi={qi} but more than one consecutive identical pair exists — missing upstream guard"
                );
            }
            slots[correct_oi] = correct_val;
            let distractors = pick_distractors(&val_pool, correct_val, qi, qt, rng);
            place_distractors(&distractors, slots, correct_oi);
        }
        QuestionType::LetterDist { .. } => {
            slots[correct_oi] = correct_val;
            let distractors = pick_distractors(&val_pool, correct_val, qi, qt, rng);
            place_distractors(&distractors, slots, correct_oi);
        }
        _ if is_counting_type(qt) => {
            slots[correct_oi] = correct_val;
            let distractors = pick_distractors(&val_pool, correct_val, qi, qt, rng);
            place_distractors(&distractors, slots, correct_oi);
        }
        QuestionType::SameAsWhich { question_index } => {
            if correct_val.is_none() {
                panic!(
                    "fill_one_question: SameAsWhich at qi={qi} ref={question_index} but no other question shares the referenced answer — missing upstream guard"
                );
            }
            let ref_ans = solution[question_index as usize];
            slots[correct_oi] = correct_val;
            let mut pool = [OptionValue::UNUSED; MAX_N];
            let mut plen = 0;
            for j in 0..n {
                if j != qi && j != question_index as usize && solution[j] != ref_ans {
                    pool[plen] = OptionValue::num(j as u8);
                    plen += 1;
                }
            }
            rng.shuffle(&mut pool[..plen]);
            let mut distractors = [OptionValue::UNUSED; 4];
            distractors[..4.min(plen)].copy_from_slice(&pool[..4.min(plen)]);
            place_distractors(&distractors, slots, correct_oi);
        }
        QuestionType::SameAs => {
            let self_ans = solution[qi];
            let mut pool = [OptionValue::UNUSED; MAX_N];
            let mut plen = 0;
            if correct_val.is_none() {
                // "none" is correct (qi's answer is unique): every other question is a distractor.
                for j in 0..n {
                    if j != qi {
                        pool[plen] = OptionValue::num(j as u8);
                        plen += 1;
                    }
                }
            } else {
                // A match exists: distractors are differing-answer questions plus "none".
                // Same-answer questions are excluded — they'd be alternate correct answers.
                for j in 0..n {
                    let jv = OptionValue::num(j as u8);
                    if j != qi && jv != correct_val && solution[j] != self_ans {
                        pool[plen] = jv;
                        plen += 1;
                    }
                }
                pool[plen] = OptionValue::NONE;
                plen += 1;
            }
            if plen < option_count - 1 {
                panic!(
                    "fill_one_question: SameAs at qi={qi} pool too small ({plen} < {}) — missing upstream guard",
                    option_count - 1
                );
            }
            slots[correct_oi] = correct_val;
            rng.shuffle(&mut pool[..plen]);
            let mut distractors = [OptionValue::UNUSED; 4];
            distractors[..4.min(plen)].copy_from_slice(&pool[..4.min(plen)]);
            place_distractors(&distractors, slots, correct_oi);
        }
        QuestionType::OnlySame => {
            let self_ans = solution[qi];
            let others = (0..n)
                .filter(|&j| j != qi && solution[j] == self_ans)
                .count();
            if others > 1 {
                panic!(
                    "fill_one_question: OnlySame at qi={qi} but {others} other questions share answer {self_ans:?} — missing upstream guard"
                );
            }
            slots[correct_oi] = correct_val;
            let distractors = pick_distractors(&val_pool, correct_val, qi, qt, rng);
            place_distractors(&distractors, slots, correct_oi);
        }
        QuestionType::ClosestAfter { .. }
        | QuestionType::ClosestBefore { .. }
        | QuestionType::FirstWith { .. }
        | QuestionType::LastWith { .. }
        | QuestionType::PrevSame
        | QuestionType::NextSame => {
            slots[correct_oi] = correct_val;
            let distractors = pick_distractors(&val_pool, correct_val, qi, qt, rng);
            place_distractors(&distractors, slots, correct_oi);
        }
        _ => unreachable!(),
    }
}

pub fn fill_options(
    question_types: &[QuestionType; MAX_N],
    solution: &[Answer; MAX_N],
    n: usize,
    option_count: usize,
    rng: &mut Rng,
    trace: bool,
) -> FlatPuzzle {
    let mut options = [[OptionValue::UNUSED; 5]; MAX_N];
    let mut true_stmt_question_types: Option<[QuestionType; 5]> = None;

    for qi in 0..n {
        let qt = &question_types[qi];
        let mut local_types: Option<[QuestionType; 5]> = None;
        fill_one_question(
            qt,
            qi,
            solution,
            n,
            option_count,
            rng,
            &mut options[qi],
            &mut local_types,
        );
        if let Some(types) = local_types {
            true_stmt_question_types = Some(types);
        }

        if trace {
            let vals: Vec<Value> = (0..option_count)
                .map(|oi| {
                    let s = options[qi][oi];
                    if matches!(qt, QuestionType::TrueStmt) || s.is_none() || s.is_unused() {
                        Value::Null
                    } else {
                        json!(s.value())
                    }
                })
                .collect();
            let mut obj = json!({
                "t": "question",
                "qi": qi,
                "type": format_type_tag(qt),
                "options": vals,
                "rng": rng.state(),
            });
            if matches!(qt, QuestionType::TrueStmt) {
                let types = true_stmt_question_types
                    .as_ref()
                    .expect("TrueStmt qi must have populated claim types");
                let claims: Vec<Value> = (0..option_count)
                    .map(|oi| {
                        let v = options[qi][oi];
                        if v.is_unused() {
                            Value::Null
                        } else {
                            let val = if v.is_none() {
                                Value::Null
                            } else {
                                json!(v.value())
                            };
                            let mut co = json!({"questionType": "", "value": val});
                            co["questionType"] = format_claim_qt(&types[oi]);
                            co
                        }
                    })
                    .collect();
                obj["claims"] = json!(claims);
            }
            eprintln!("{}", obj);
        }
    }

    let (affected_by, global_indices) = FlatPuzzle::build_deps(question_types, n);

    FlatPuzzle {
        question_types: *question_types,
        options,
        true_stmt_question_types,
        affected_by,
        global_indices,
        n,
        option_count,
        initial_state: State::initial(option_count),
    }
}

fn place_distractors(
    distractors: &[OptionValue; 4],
    slots: &mut [OptionValue; 5],
    correct_oi: usize,
) {
    let mut di = 0;
    for oi in 0..5 {
        if oi != correct_oi {
            slots[oi] = distractors[di];
            di += 1;
        }
    }
}

pub fn correct_option_value(
    qt: &QuestionType,
    qi: usize,
    sol: &[Answer; MAX_N],
    n: usize,
) -> OptionValue {
    fn num(v: usize) -> OptionValue {
        OptionValue::num(v as u8)
    }
    fn pos_or_none(p: Option<usize>) -> OptionValue {
        p.map_or(OptionValue::NONE, num)
    }
    match *qt {
        QuestionType::AnswerOf { question_index } => num(sol[question_index as usize].idx()),
        QuestionType::CountAnswer { answer } => num(count_letter(sol, answer, n) as usize),
        QuestionType::CountAnswerBefore {
            answer,
            before_index,
        } => num((0..before_index as usize)
            .filter(|&i| sol[i] == answer)
            .count()),
        QuestionType::CountAnswerAfter {
            answer,
            after_index,
        } => num(((after_index as usize + 1)..n)
            .filter(|&i| sol[i] == answer)
            .count()),
        QuestionType::CountVowel => num((0..n).filter(|&i| sol[i].is_vowel()).count()),
        QuestionType::CountConsonant => num((0..n).filter(|&i| !sol[i].is_vowel()).count()),
        QuestionType::MostCommonCount => num(*letter_counts(sol, n).iter().max().unwrap() as usize),
        QuestionType::ClosestAfter {
            after_index,
            answer,
        } => pos_or_none(((after_index as usize + 1)..n).find(|&i| sol[i] == answer)),
        QuestionType::ClosestBefore {
            before_index,
            answer,
        } => pos_or_none((0..before_index as usize).rev().find(|&i| sol[i] == answer)),
        QuestionType::FirstWith { answer } => pos_or_none((0..n).find(|&i| sol[i] == answer)),
        QuestionType::LastWith { answer } => pos_or_none((0..n).rev().find(|&i| sol[i] == answer)),
        QuestionType::PrevSame => pos_or_none((0..qi).rev().find(|&i| sol[i] == sol[qi])),
        QuestionType::NextSame => pos_or_none(((qi + 1)..n).find(|&i| sol[i] == sol[qi])),
        QuestionType::OnlySame | QuestionType::SameAs => {
            pos_or_none((0..n).find(|&i| i != qi && sol[i] == sol[qi]))
        }
        QuestionType::SameAsWhich { question_index } => {
            let ref_ans = sol[question_index as usize];
            pos_or_none(
                (0..n).find(|&i| i != qi && i != question_index as usize && sol[i] == ref_ans),
            )
        }
        QuestionType::OnlyOdd { answer } | QuestionType::OnlyEven { answer } => {
            let parity = match qt {
                QuestionType::OnlyOdd { .. } => 1,
                _ => 0,
            };
            pos_or_none((0..n).find(|&i| (i + 1) % 2 == parity && sol[i] == answer))
        }
        QuestionType::ConsecIdent => {
            pos_or_none((0..n.saturating_sub(1)).find(|&i| sol[i] == sol[i + 1]))
        }
        QuestionType::EqualCount { answer } => {
            let ref_count = count_letter(sol, answer, n);
            LETTERS
                .iter()
                .find(|&&l| l != answer && count_letter(sol, l, n) == ref_count)
                .map_or(OptionValue::NONE, |l| num(l.idx()))
        }
        QuestionType::LetterDist { question_index } => num(usize::from(
            (sol[qi].idx() as u8).abs_diff(sol[question_index as usize].idx() as u8),
        )),
        _ => OptionValue::UNUSED,
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
    vals: &ArrayVec<OptionValue, 20>,
    correct: OptionValue,
    qi: usize,
    qt: &QuestionType,
    rng: &mut Rng,
) -> [OptionValue; 4] {
    let mut pool = [OptionValue::UNUSED; 20];
    let mut plen = 0;
    for &v in vals {
        let exclude_self = matches!(*qt, QuestionType::OnlySame | QuestionType::SameAs)
            && v.is_num()
            && usize::from(v.value()) == qi;
        if v != correct && !exclude_self {
            pool[plen] = v;
            plen += 1;
        }
    }
    rng.shuffle(&mut pool[..plen]);
    let mut result = [OptionValue::UNUSED; 4];
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
    slots: &mut [OptionValue; 5],
    true_stmt_question_types: &mut Option<[QuestionType; 5]>,
    option_count: usize,
) {
    let target_oi = solution[qi].idx();
    let mut local: [Option<Claim>; 5] = [None; 5];

    let true_claim = make_true_claim(solution, qi, n, rng, option_count);
    local[target_oi] = Some(true_claim);

    for oi in 0..option_count {
        if oi == target_oi {
            continue;
        }
        let mut found = false;
        for _ in 0..30 {
            let fc = make_false_claim(solution, qi, n, rng, option_count);
            let cat = claim_category(&fc);
            if cat != claim_category(local[target_oi].as_ref().unwrap())
                && (0..oi).all(|j| {
                    j == target_oi || local[j].as_ref().is_none_or(|c| claim_category(c) != cat)
                })
            {
                local[oi] = Some(fc);
                found = true;
                break;
            }
        }
        if !found {
            local[oi] = Some(make_false_claim(solution, qi, n, rng, option_count));
        }
    }

    // Split into SoA: values live in `slots`, types in `true_stmt_question_types`.
    // Slots with no claim (oi >= option_count) stay UNUSED; the matching type
    // entry is a harmless placeholder since `claim_at` gates on slot validity.
    let mut types = [QuestionType::AnswerIsSelf; 5];
    for oi in 0..option_count {
        if let Some(c) = local[oi] {
            types[oi] = c.question_type;
            slots[oi] = c.value;
        }
    }
    *true_stmt_question_types = Some(types);
}

// Position-dependent kinds (NextSame, PrevSame) must NOT appear here: a claim
// has no qi of its own, so valid_values can't compute the right pool for them.
// The const block below enforces this at compile time.
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

const _: () = {
    let mut i = 0;
    while i < CLAIM_TYPES.len() {
        match CLAIM_TYPES[i] {
            QuestionTypeKind::NextSame | QuestionTypeKind::PrevSame => {
                panic!("CLAIM_TYPES must not contain position-dependent kinds");
            }
            _ => {}
        }
        i += 1;
    }
};

fn try_make_claim(sol: &[Answer; MAX_N], qi: usize, n: usize, rng: &mut Rng) -> Option<Claim> {
    let kind = rng.pick(CLAIM_TYPES);
    match kind {
        QuestionTypeKind::CountAnswer => {
            let a = rng.pick(&LETTERS);
            Some(Claim {
                question_type: QuestionType::CountAnswer { answer: a },
                value: OptionValue::num(count_letter(sol, a, n) as u8),
            })
        }
        QuestionTypeKind::CountConsonant => Some(Claim {
            question_type: QuestionType::CountConsonant,
            value: OptionValue::num((0..n).filter(|&i| !sol[i].is_vowel()).count() as u8),
        }),
        QuestionTypeKind::CountVowel => Some(Claim {
            question_type: QuestionType::CountVowel,
            value: OptionValue::num((0..n).filter(|&i| sol[i].is_vowel()).count() as u8),
        }),
        QuestionTypeKind::CountAnswerAfter => {
            let a = rng.pick(&LETTERS);
            let ai = rng.int(0, (n as i32 - 5).max(0)) as u8;
            Some(Claim {
                question_type: QuestionType::CountAnswerAfter {
                    answer: a,
                    after_index: ai,
                },
                value: OptionValue::num(
                    ((ai as usize + 1)..n).filter(|&i| sol[i] == a).count() as u8
                ),
            })
        }
        QuestionTypeKind::CountAnswerBefore => {
            if n < 5 {
                return None;
            }
            let a = rng.pick(&LETTERS);
            let bi = rng.int(4, n as i32 - 1) as u8;
            Some(Claim {
                question_type: QuestionType::CountAnswerBefore {
                    answer: a,
                    before_index: bi,
                },
                value: OptionValue::num((0..bi as usize).filter(|&i| sol[i] == a).count() as u8),
            })
        }
        QuestionTypeKind::AnswerOf => {
            // Pick a target qi != qi (the TrueStmt's own position).
            if n < 2 {
                return None;
            }
            let mut target = rng.int(0, n as i32 - 2) as usize;
            if target >= qi {
                target += 1;
            }
            Some(Claim {
                question_type: QuestionType::AnswerOf {
                    question_index: target as u8,
                },
                value: OptionValue::num(sol[target].idx() as u8),
            })
        }
        QuestionTypeKind::FirstWith => {
            let a = rng.pick(&LETTERS);
            let first = (0..n).find(|&i| sol[i] == a)?;
            Some(Claim {
                question_type: QuestionType::FirstWith { answer: a },
                value: OptionValue::num(first as u8),
            })
        }
        QuestionTypeKind::LastWith => {
            let a = rng.pick(&LETTERS);
            let last = (0..n).rev().find(|&i| sol[i] == a)?;
            Some(Claim {
                question_type: QuestionType::LastWith { answer: a },
                value: OptionValue::num(last as u8),
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
                value: OptionValue::num(most[0].idx() as u8),
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
                value: OptionValue::num(target as u8),
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
                value: OptionValue::num(target as u8),
            })
        }
        QuestionTypeKind::MostCommonCount => {
            let counts = letter_counts(sol, n);
            let max = *counts.iter().max().unwrap_or(&0);
            Some(Claim {
                question_type: QuestionType::MostCommonCount,
                value: OptionValue::num(max as u8),
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
                value: OptionValue::num(idx as u8),
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
                value: OptionValue::num(target.idx() as u8),
            })
        }
        QuestionTypeKind::ConsecIdent => {
            let mut pair_pos: Option<usize> = None;
            let mut pair_count = 0;
            for i in 0..n.saturating_sub(1) {
                if sol[i] == sol[i + 1] {
                    if pair_count == 0 {
                        pair_pos = Some(i);
                    }
                    pair_count += 1;
                }
            }
            if pair_count > 1 {
                return None;
            }
            Some(Claim {
                question_type: QuestionType::ConsecIdent,
                value: pair_pos.map_or(OptionValue::NONE, |p| OptionValue::num(p as u8)),
            })
        }
        QuestionTypeKind::OnlyOdd | QuestionTypeKind::OnlyEven => {
            let a = rng.pick(&LETTERS);
            let parity = if kind == QuestionTypeKind::OnlyOdd {
                1
            } else {
                0
            };
            let mut found: Option<usize> = None;
            let mut count = 0;
            for i in 0..n {
                if (i + 1) % 2 == parity && sol[i] == a {
                    found = Some(i);
                    count += 1;
                }
            }
            if count != 1 {
                return None;
            }
            let value = found.map_or(OptionValue::NONE, |p| OptionValue::num(p as u8));
            let question_type = if kind == QuestionTypeKind::OnlyOdd {
                QuestionType::OnlyOdd { answer: a }
            } else {
                QuestionType::OnlyEven { answer: a }
            };
            Some(Claim {
                question_type,
                value,
            })
        }
        QuestionTypeKind::SameAsWhich => {
            // Pick a ref_qi != qi (the TrueStmt's own position).
            if n < 2 {
                return None;
            }
            let mut ref_qi = rng.int(0, n as i32 - 2) as usize;
            if ref_qi >= qi {
                ref_qi += 1;
            }
            let ref_ans = sol[ref_qi];
            let mut matches = ArrayVec::<usize, MAX_N>::new();
            for i in 0..n {
                if i != ref_qi && i != qi && sol[i] == ref_ans {
                    matches.push(i);
                }
            }
            if matches.is_empty() {
                return None;
            }
            let target = rng.pick(&matches);
            Some(Claim {
                question_type: QuestionType::SameAsWhich {
                    question_index: ref_qi as u8,
                },
                value: OptionValue::num(target as u8),
            })
        }
        _ => None,
    }
}

fn make_true_claim(
    sol: &[Answer; MAX_N],
    qi: usize,
    n: usize,
    rng: &mut Rng,
    option_count: usize,
) -> Claim {
    for _ in 0..20 {
        if let Some(claim) = try_make_claim(sol, qi, n, rng) {
            debug_assert!(check_claim_fast(option_count, &sol[..n], qi, &claim));
            return claim;
        }
    }
    let a = rng.pick(&LETTERS);
    Claim {
        question_type: QuestionType::CountAnswer { answer: a },
        value: OptionValue::num(count_letter(sol, a, n) as u8),
    }
}

fn make_false_claim(
    sol: &[Answer; MAX_N],
    qi: usize,
    n: usize,
    rng: &mut Rng,
    option_count: usize,
) -> Claim {
    for _ in 0..30 {
        let base = make_true_claim(sol, qi, n, rng, option_count);
        let fc = perturb_claim(base, n, rng);
        if let Some(fc) = fc
            && !check_claim_fast(option_count, &sol[..n], qi, &fc)
        {
            return fc;
        }
    }
    Claim {
        question_type: QuestionType::CountAnswer { answer: Answer::A },
        // Intentionally out of range: caller treats this as "give up" sentinel.
        value: OptionValue::num(n as u8 + 1),
    }
}

fn perturb_claim(claim: Claim, n: usize, rng: &mut Rng) -> Option<Claim> {
    // Count-type perturbation: offset the existing claim value. Done in i8
    // because the offset is signed (-2..=2) and the base may be NONE
    // (treated as -1 for "the count was null"). Range is [-3, MAX_N+2].
    let perturb_count = |max: u8, rng: &mut Rng| -> Option<u8> {
        let offsets: [i8; 4] = [-2, -1, 1, 2];
        let base: i8 = if claim.value.is_num() {
            claim.value.value() as i8
        } else {
            -1
        };
        let v = base + rng.pick(&offsets);
        u8::try_from(v).ok().filter(|&v| v <= max)
    };
    let new_val: u8 = match claim.question_type {
        QuestionType::CountAnswer { .. }
        | QuestionType::CountConsonant
        | QuestionType::CountVowel
        | QuestionType::MostCommonCount => perturb_count(n as u8, rng)?,
        QuestionType::CountAnswerAfter { after_index, .. } => {
            perturb_count(n as u8 - after_index - 1, rng)?
        }
        QuestionType::CountAnswerBefore { before_index, .. } => perturb_count(before_index, rng)?,
        QuestionType::FirstWith { .. }
        | QuestionType::LastWith { .. }
        | QuestionType::SameAsWhich { .. } => rng.int(0, n as i32 - 1) as u8,
        QuestionType::ConsecIdent => {
            // Valid pool: [0, n-1) — pair (v, v+1) requires v+1 < n.
            if n < 2 {
                return None;
            }
            rng.int(0, n as i32 - 2) as u8
        }
        QuestionType::OnlyOdd { .. } => {
            // Valid pool: 0-indexed even positions {0, 2, 4, …} (= 1-indexed odd).
            if n == 0 {
                return None;
            }
            (rng.int(0, (n as i32 - 1) / 2) * 2) as u8
        }
        QuestionType::OnlyEven { .. } => {
            // Valid pool: 0-indexed odd positions {1, 3, 5, …} (= 1-indexed even).
            if n < 2 {
                return None;
            }
            (rng.int(0, (n as i32 - 2) / 2) * 2 + 1) as u8
        }
        QuestionType::ClosestAfter { after_index, .. } => {
            // Valid pool: (after_index, n).
            if (after_index as usize) + 1 >= n {
                return None;
            }
            rng.int(after_index as i32 + 1, n as i32 - 1) as u8
        }
        QuestionType::ClosestBefore { before_index, .. } => {
            // Valid pool: [0, before_index).
            if before_index == 0 {
                return None;
            }
            rng.int(0, before_index as i32 - 1) as u8
        }
        QuestionType::AnswerOf { .. }
        | QuestionType::MostCommon
        | QuestionType::LeastCommon
        | QuestionType::NoOtherHasAnswer => rng.pick(&LETTERS).idx() as u8,
        QuestionType::EqualCount { answer } => {
            let v = rng.pick(&LETTERS).idx() as u8;
            if v == answer.idx() as u8 {
                return None;
            }
            v
        }
        _ => return None,
    };
    let new_value = OptionValue::num(new_val);
    if new_value == claim.value {
        return None;
    }
    Some(Claim {
        value: new_value,
        ..claim
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn test_shared_fill_options() {
        let json_str = std::fs::read_to_string("../tests/fill-options.json")
            .expect("can't read tests/fill-options.json");
        let suite: Value = serde_json::from_str(&json_str).unwrap();
        let tests = suite["tests"].as_array().unwrap();

        const SEEDS: u32 = 16;
        let mut passed = 0;
        let mut failed = 0;

        for test in tests {
            if test.get("section").is_some() {
                continue;
            }
            let name = test["name"].as_str().unwrap();
            let n = test["n"].as_u64().unwrap() as usize;
            let oc = test["oc"].as_u64().unwrap() as usize;
            let sol_str = test["solution"].as_str().unwrap();
            let types_json = test["types"].as_array().unwrap();

            let mut question_types = [QuestionType::AnswerIsSelf; MAX_N];
            for (qi, t) in types_json.iter().enumerate() {
                question_types[qi] = serde_json::from_value(t.clone())
                    .unwrap_or_else(|e| panic!("{name}: parse type Q{}: {e}", qi + 1));
            }

            let mut solution = [Answer::A; MAX_N];
            for (i, ch) in sol_str.chars().enumerate() {
                solution[i] = LETTERS[(ch as u8 - b'A') as usize];
            }

            // Fixture entry: `null` = skip the check for this question;
            // `<integer>` = expected numeric value at the correct option.
            let expected_correct: Vec<Option<u8>> = test["expectedCorrect"]
                .as_array()
                .unwrap_or_else(|| panic!("{name}: missing expectedCorrect"))
                .iter()
                .map(|v| {
                    if v.is_null() {
                        None
                    } else {
                        Some(
                            v.as_u64()
                                .expect("expectedCorrect entry must be a non-negative int or null")
                                as u8,
                        )
                    }
                })
                .collect();
            assert_eq!(
                expected_correct.len(),
                n,
                "{name}: expectedCorrect length must equal n"
            );

            let mut case_failed = false;
            for seed in 0..SEEDS {
                let mut rng = Rng::new(seed.wrapping_mul(2654435761));
                let fp = fill_options(&question_types, &solution, n, oc, &mut rng, false);

                let answers: [Option<Answer>; MAX_N] =
                    std::array::from_fn(|i| if i < n { Some(solution[i]) } else { None });
                if !check_answers(&fp, &answers) {
                    eprintln!("FAIL: {name} (seed={seed}): check_answers rejected");
                    for qi in 0..n {
                        let state = State {
                            answers,
                            eliminated: [fp.phantom_mask(); MAX_N],
                        };
                        let v = check_answer(&fp, state, qi);
                        eprintln!("  Q{}: {:?}", qi + 1, v);
                    }
                    case_failed = true;
                    break;
                }

                // expectedCorrect: cross-check that the value stored at the correct option
                // matches the hand-computed expectation in the fixture. Catches semantic
                // drift in correct_option_value that check_answer would miss.
                for qi in 0..n {
                    let Some(expected) = expected_correct[qi] else {
                        continue;
                    };
                    let correct_oi = solution[qi].idx();
                    let stored = fp.options[qi][correct_oi];
                    if !stored.is_num() || stored.value() != expected {
                        eprintln!(
                            "FAIL: {name} (seed={seed}) Q{}: stored {stored:?} != expected {expected}",
                            qi + 1
                        );
                        case_failed = true;
                        break;
                    }
                }
                if case_failed {
                    break;
                }

                // Distinctness: distractor option values must differ from the correct value
                // and from each other (across the active option count). Identity-option
                // and TrueStmt types don't store distinct distractor values, so skip them.
                for qi in 0..n {
                    let qt = &fp.question_types[qi];
                    if qt.has_identity_options() || matches!(qt, QuestionType::TrueStmt) {
                        continue;
                    }
                    let slots = &fp.options[qi];
                    let mut seen: Vec<u8> = Vec::new();
                    for &s in &slots[..oc] {
                        if !s.is_num() {
                            continue;
                        }
                        let v = s.value();
                        if seen.contains(&v) {
                            eprintln!(
                                "FAIL: {name} (seed={seed}) Q{}: duplicate option value {v} in {:?}",
                                qi + 1,
                                &slots[..oc]
                            );
                            case_failed = true;
                            break;
                        }
                        seen.push(v);
                    }
                    if case_failed {
                        break;
                    }
                }
                if case_failed {
                    break;
                }
            }

            if case_failed {
                failed += 1;
            } else {
                passed += 1;
            }
        }

        eprintln!("{passed}/{} passed", passed + failed);
        assert_eq!(failed, 0, "{failed} test(s) failed");
    }

    #[test]
    fn test_shared_valid_values() {
        use crate::check_form::check_form;
        use crate::serialize::parse_puzzle;
        use serde_json::json;

        let json_str = std::fs::read_to_string("../tests/valid-values.json")
            .expect("can't read tests/valid-values.json");
        let suite: Value = serde_json::from_str(&json_str).unwrap();
        let tests = suite["tests"].as_array().unwrap();

        // NoOtherHasAnswer / AnswerIsSelf use identity options, TrueStmt uses claims;
        // the parser overrides input, so checkForm's range warning is unobservable.
        let exempt = [
            QuestionTypeKind::NoOtherHasAnswer,
            QuestionTypeKind::AnswerIsSelf,
            QuestionTypeKind::TrueStmt,
        ];
        let value_typed: Vec<QuestionTypeKind> = QuestionTypeKind::all_flat()
            .iter()
            .copied()
            .filter(|k| !exempt.contains(k))
            .collect();
        let mut covered: std::collections::HashSet<QuestionTypeKind> =
            std::collections::HashSet::new();

        let build_puzzle =
            |type_json: &Value, qi: usize, n: usize, oc: usize, v: Option<i64>| -> Value {
                let mut qs = Vec::with_capacity(n);
                let mut opts: Vec<Value> = Vec::with_capacity(n);
                for i in 0..n {
                    if i == qi {
                        qs.push(type_json.clone());
                        let mut row: Vec<Value> = Vec::with_capacity(oc);
                        row.push(match v {
                            Some(x) => json!(x),
                            None => Value::Null,
                        });
                        for _ in 1..oc {
                            row.push(Value::Null);
                        }
                        opts.push(json!(row));
                    } else {
                        qs.push(json!({ "t": "AnswerIsSelf" }));
                        opts.push(json!(vec![Value::Null; oc]));
                    }
                }
                json!({ "q": qs, "o": opts })
            };

        for test in tests {
            if test.get("section").is_some() {
                continue;
            }
            let name = test["name"].as_str().unwrap();
            let qi = test["qi"].as_u64().unwrap() as usize;
            let n = test["n"].as_u64().unwrap() as usize;
            let oc = test["oc"].as_u64().unwrap() as usize;
            let type_json = &test["type"];

            let qt: QuestionType = serde_json::from_value(type_json.clone())
                .unwrap_or_else(|e| panic!("{name}: parse type: {e}"));
            covered.insert(qt.kind());

            // 1) validValues output matches fixture
            let got = valid_values(&qt, qi, n, oc);
            let mut got_set: Vec<String> = got
                .iter()
                .map(|&v| {
                    if v.is_none() {
                        "null".into()
                    } else {
                        v.value().to_string()
                    }
                })
                .collect();
            got_set.sort();
            let mut exp_set: Vec<String> = test["valid"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| {
                    if v.is_null() {
                        "null".into()
                    } else {
                        v.to_string()
                    }
                })
                .collect();
            exp_set.sort();
            assert_eq!(got_set, exp_set, "{name}: pool mismatch");

            // 2) & 3) Cross-check checkForm: any message at qi mentioning
            // "option 0" (the slot we vary) must fire iff the value isn't in
            // the pool. The "option 0" scope filters out incidental errors on
            // the other (null-filled) options.
            // Skip negatives: JSON -1 would parse as OptionValue::NONE via parse_puzzle.
            let pool_ints: std::collections::HashSet<i64> = test["valid"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|v| v.as_i64())
                .collect();
            let null_in_pool = test["valid"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v.is_null());
            let max_v = n.max(oc) as i64 + 1;
            let mut candidates: Vec<Option<i64>> = (0..=max_v).map(Some).collect();
            candidates.push(None);
            for v in candidates {
                let in_pool = match v {
                    Some(x) => pool_ints.contains(&x),
                    None => null_in_pool,
                };
                let puzzle = build_puzzle(type_json, qi, n, oc, v);
                let fp = parse_puzzle(&puzzle).expect("parse_puzzle failed");
                let errors = check_form(&fp, None);
                let flagged = errors.iter().any(|e| {
                    e.qi == qi && (e.message.contains("option 0") || e.message.contains("Option 0"))
                });
                let v_str = match v {
                    Some(x) => x.to_string(),
                    None => "null".to_string(),
                };
                assert_eq!(
                    flagged,
                    !in_pool,
                    "{name} v={v_str}: pool={}, checkForm={} (disagree)",
                    if in_pool { "in" } else { "out" },
                    if flagged { "flagged" } else { "ok" }
                );
            }
        }

        // 4) Coverage
        for ty in &value_typed {
            assert!(
                covered.contains(ty),
                "valid-values: missing fixture coverage for {ty:?}"
            );
        }
    }
}
