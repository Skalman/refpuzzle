//! Concrete content for a question type against a fixed answer key: the random
//! parametrization that turns a kind into a full `QuestionType`, each question's
//! option row — the correct value plus its distractors — and the per-option
//! statements for TrueStmt questions.

use arrayvec::ArrayVec;
use serde_json::{Value, json};

use crate::check_answer::check_claim_fast;
use crate::check_form;
use crate::counts::{count_letter, letter_counts};
use crate::format::{format_stmt_qt, format_type_tag};
use crate::recipes::exact_recipe;
use crate::rng::Rng;
use crate::types::*;

// ── Question-type parametrization ──

/// Draw random parameters for `kind` at slot `qi`, giving a full `QuestionType`, or
/// `None` if the board can't host the kind there — a pool too small to yield
/// `option_count` distinct option values, or a key the kind can't satisfy.
/// `construct`'s skeleton builder draws slot types with this; `fill` draws TrueStmt
/// claim types with it.
pub(crate) fn random_type_params(
    kind: QuestionTypeKind,
    qi: usize,
    n: usize,
    option_count: usize,
    solution: &[Answer; MAX_N],
    rng: &mut Rng,
) -> Option<QuestionType> {
    match kind {
        QuestionTypeKind::CountAnswer => Some(QuestionType::CountAnswer {
            answer: rng.pick_letter(option_count),
        }),
        QuestionTypeKind::CountAnswerBefore => {
            // Need before_index with at least oc distinct count values (0..=before_index).
            if n < option_count {
                return None;
            }
            Some(QuestionType::CountAnswerBefore {
                answer: rng.pick_letter(option_count),
                before_index: rng.int(option_count as i32 - 1, n as i32 - 1) as u8,
            })
        }
        QuestionTypeKind::CountAnswerAfter => {
            // Need after_index with at least oc distinct count values (0..=n-1-after_index).
            if n < option_count {
                return None;
            }
            Some(QuestionType::CountAnswerAfter {
                answer: rng.pick_letter(option_count),
                after_index: rng.int(0, n as i32 - option_count as i32) as u8,
            })
        }
        QuestionTypeKind::CountVowel => Some(QuestionType::CountVowel),
        QuestionTypeKind::CountConsonant => Some(QuestionType::CountConsonant),
        QuestionTypeKind::MostCommonCount => Some(QuestionType::MostCommonCount),
        QuestionTypeKind::AnswerOf => {
            // Placeholder self-pointer; `wire_answer_of_targets` assigns the real
            // target later, once the whole type map is known.
            Some(QuestionType::AnswerOf {
                question_index: qi as u8,
            })
        }
        QuestionTypeKind::LetterDist => {
            let mut pool = [0u8; MAX_N];
            let mut pool_len = 0;
            for j in 0..n {
                if j != qi {
                    pool[pool_len] = j as u8;
                    pool_len += 1;
                }
            }
            Some(QuestionType::LetterDist {
                question_index: rng.pick(&pool[..pool_len]),
            })
        }
        QuestionTypeKind::ClosestAfter => {
            // Need after_index with at least oc distinct option values
            // (positions after_index+1..n, plus null).
            if n < option_count {
                return None;
            }
            Some(QuestionType::ClosestAfter {
                after_index: rng.int(0, n as i32 - option_count as i32) as u8,
                answer: rng.pick_letter(option_count),
            })
        }
        QuestionTypeKind::ClosestBefore => {
            // Need before_index with at least oc distinct option values
            // (positions 0..before_index, plus null).
            if n < option_count {
                return None;
            }
            Some(QuestionType::ClosestBefore {
                before_index: rng.int(option_count as i32 - 1, n as i32 - 1) as u8,
                answer: rng.pick_letter(option_count),
            })
        }
        QuestionTypeKind::FirstWith => Some(QuestionType::FirstWith {
            answer: rng.pick_letter(option_count),
        }),
        QuestionTypeKind::LastWith => Some(QuestionType::LastWith {
            answer: rng.pick_letter(option_count),
        }),
        QuestionTypeKind::PrevSame => {
            // Need oc distinct option values; pool size is qi + 1 (positions [0, qi) + null).
            if qi + 1 < option_count {
                return None;
            }
            Some(QuestionType::PrevSame)
        }
        QuestionTypeKind::NextSame => {
            // Need oc distinct option values; pool size is n - qi (positions (qi, n) + null).
            if n - qi < option_count {
                return None;
            }
            Some(QuestionType::NextSame)
        }
        QuestionTypeKind::OnlySame => Some(QuestionType::OnlySame),
        QuestionTypeKind::OnlySameAmong => {
            // Feasibility: fill needs oc-1 distinct distractor targets. If qi's answer
            // is unique the pool is the n-1 other questions; if a match exists, the
            // same-answer questions are excluded (they'd be alternate correct answers),
            // leaving (n - same_count) differing questions + "none". Must be >= oc-1,
            // else fill_one_question can't build the row.
            let same_count = count_letter(solution, solution[qi], n) as usize;
            let pool = if same_count == 1 {
                n - 1
            } else {
                n - same_count + 1
            };
            if pool < option_count - 1 {
                return None;
            }
            Some(QuestionType::OnlySameAmong)
        }
        QuestionTypeKind::ConsecIdent => Some(QuestionType::ConsecIdent),
        QuestionTypeKind::OnlyOdd | QuestionTypeKind::OnlyEven => {
            let answer = rng.pick_letter(option_count);
            Some(if kind == QuestionTypeKind::OnlyOdd {
                QuestionType::OnlyOdd { answer }
            } else {
                QuestionType::OnlyEven { answer }
            })
        }
        QuestionTypeKind::LeastCommon => Some(QuestionType::LeastCommon),
        QuestionTypeKind::MostCommon => Some(QuestionType::MostCommon),
        QuestionTypeKind::NoOtherHasAnswer => Some(QuestionType::NoOtherHasAnswer),
        QuestionTypeKind::EqualCount => {
            let ref_letter = rng.pick_letter(option_count);
            let ref_count = count_letter(solution, ref_letter, n);
            let has_match = LETTERS[..option_count]
                .iter()
                .any(|&l| l != ref_letter && count_letter(solution, l, n) == ref_count);
            // When no other letter shares ref's count the "equal count" reads as a
            // near-miss, so keep it only ~40% of the time (reject 3 of 5 draws) to
            // thin them out; a natural match (has_match) is always kept.
            if !has_match && rng.int(0, 4) > 1 {
                return None;
            }
            Some(QuestionType::EqualCount { answer: ref_letter })
        }
        QuestionTypeKind::AnswerIsSelf => Some(QuestionType::AnswerIsSelf),
        QuestionTypeKind::TrueStmt => {
            if option_count < 5 {
                return None;
            }
            Some(QuestionType::TrueStmt)
        }
        QuestionTypeKind::OnlySameAsAmong => {
            let mut pool = [0u8; MAX_N];
            let mut pool_len = 0;
            for j in 0..n {
                if j != qi {
                    pool[pool_len] = j as u8;
                    pool_len += 1;
                }
            }
            if pool_len == 0 {
                return None;
            }
            let ref_qi = rng.pick(&pool[..pool_len]) as usize;
            if solution[ref_qi] == solution[qi] {
                return None;
            }
            // No structural match requirement: with a NONE option, "no listed
            // candidate shares ref's answer" is an ordinary answer, so a key where
            // only `ref_qi` holds that letter is placeable.
            // Capacity: need at least oc-1 questions whose answer differs from ref (distractors).
            let distractor_count = (0..n)
                .filter(|&j| j != qi && solution[j] != solution[ref_qi])
                .count();
            if distractor_count < option_count - 1 {
                return None;
            }
            Some(QuestionType::OnlySameAsAmong {
                question_index: ref_qi as u8,
            })
        }
    }
}

// ── Option value domains ──

/// Upper bound on a single question's candidate-value pool: one value per
/// question index (≤ `MAX_N`) plus a handful of specials (NONE, counts up to n).
const MAX_VALUE_POOL: usize = 20;

pub(crate) fn valid_values(
    qt: &QuestionType,
    qi: usize,
    n: usize,
    oc: usize,
) -> ArrayVec<OptionValue, MAX_VALUE_POOL> {
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
        QuestionType::OnlySameAmong | QuestionType::OnlySame => {
            for v in 0..n {
                if v != qi {
                    push_num(v);
                }
            }
            out.push(OptionValue::NONE);
        }
        QuestionType::OnlySameAsAmong { question_index } => {
            // Structural domain only: any other real question except self (qi) and
            // the referenced question, plus NONE — correct whenever no *listed*
            // candidate shares the reference's answer, which unlisted sharers don't
            // affect. Whether a candidate is *also* a valid answer (its target is
            // the only listed sharer) is key-dependent and enforced downstream by
            // check_well_posed_given_options, not here.
            for v in 0..n {
                if v != qi && v != question_index as usize {
                    push_num(v);
                }
            }
            out.push(OptionValue::NONE);
        }
        QuestionType::FirstWith { .. } | QuestionType::LastWith { .. } => {
            for v in 0..n {
                push_num(v);
            }
            out.push(OptionValue::NONE);
        }
    }
    out
}

// ── Build FlatPuzzle with options ──

pub(crate) fn fill_one_question(
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
        for oi in 0..option_count {
            slots[oi] = OptionValue::num(oi as u8);
        }
        return;
    }

    if matches!(qt, QuestionType::TrueStmt) {
        build_stmts(
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

    let none_rate = none_distractor_rate(qt.kind(), n, option_count);

    // The scoped-sameness types read their correct value *off* the sampled
    // candidate list instead of having it fixed by the key, so they return before
    // `correct_option_value` — which no longer implements them.
    match *qt {
        QuestionType::OnlySameAmong => {
            fill_scoped_sameness(
                qi,
                solution[qi],
                None,
                solution,
                n,
                option_count,
                none_rate,
                rng,
                slots,
            );
            return;
        }
        QuestionType::OnlySameAsAmong { question_index } => {
            let ref_qi = usize::from(question_index);
            fill_scoped_sameness(
                qi,
                solution[ref_qi],
                Some(ref_qi),
                solution,
                n,
                option_count,
                none_rate,
                rng,
                slots,
            );
            return;
        }
        _ => {}
    }

    let correct_val = correct_option_value(qt, qi, solution, n, option_count);
    let val_pool = valid_values(qt, qi, n, option_count);
    let letters = &LETTERS[..option_count];

    match *qt {
        QuestionType::AnswerOf { question_index } => {
            let correct_answer = solution[question_index as usize];
            place_letter_distractors(
                slots,
                correct_oi,
                correct_answer,
                letters,
                option_count,
                rng,
            );
        }
        QuestionType::LeastCommon | QuestionType::MostCommon => {
            let counts = letter_counts(solution, n);
            let opt_counts: ArrayVec<i32, 5> = letters.iter().map(|l| counts[l.idx()]).collect();
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
            let correct_letter = *letters
                .iter()
                .find(|&&l| counts[l.idx()] == target_count)
                .unwrap();
            place_letter_distractors(
                slots,
                correct_oi,
                correct_letter,
                letters,
                option_count,
                rng,
            );
        }
        QuestionType::EqualCount { answer } => {
            slots[correct_oi] = correct_val;
            let mut pool = [OptionValue::UNUSED; 4];
            let mut plen = 0;
            for &l in letters {
                let lv = OptionValue::num(l as u8);
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
            place_numeric_distractors(
                slots,
                correct_oi,
                correct_val,
                &val_pool,
                option_count,
                none_rate,
                rng,
            );
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
            place_numeric_distractors(
                slots,
                correct_oi,
                correct_val,
                &val_pool,
                option_count,
                none_rate,
                rng,
            );
        }
        QuestionType::LetterDist { .. } => {
            place_numeric_distractors(
                slots,
                correct_oi,
                correct_val,
                &val_pool,
                option_count,
                none_rate,
                rng,
            );
        }
        _ if is_counting_type(qt) => {
            place_numeric_distractors(
                slots,
                correct_oi,
                correct_val,
                &val_pool,
                option_count,
                none_rate,
                rng,
            );
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
            place_numeric_distractors(
                slots,
                correct_oi,
                correct_val,
                &val_pool,
                option_count,
                none_rate,
                rng,
            );
        }
        QuestionType::ClosestAfter { .. }
        | QuestionType::ClosestBefore { .. }
        | QuestionType::FirstWith { .. }
        | QuestionType::LastWith { .. }
        | QuestionType::PrevSame
        | QuestionType::NextSame => {
            place_numeric_distractors(
                slots,
                correct_oi,
                correct_val,
                &val_pool,
                option_count,
                none_rate,
                rng,
            );
        }
        _ => unreachable!(),
    }
}

/// Emit one question's filled-options trace line (diagnostic `trace` mode only).
/// `true_stmt_types` must be `Some` for a TrueStmt question (its statement types).
fn trace_question(
    qi: usize,
    qt: &QuestionType,
    options_qi: &[OptionValue; 5],
    option_count: usize,
    true_stmt_types: Option<&[QuestionType; 5]>,
    rng: &Rng,
) {
    let vals: Vec<Value> = (0..option_count)
        .map(|oi| {
            let ov = options_qi[oi];
            if matches!(qt, QuestionType::TrueStmt) || !ov.is_num() {
                Value::Null
            } else {
                json!(ov.value())
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
        let types = true_stmt_types.expect("TrueStmt qi must have populated claim types");
        let claims: Vec<Value> = (0..option_count)
            .map(|oi| {
                let ov = options_qi[oi];
                if ov.is_unused() {
                    Value::Null
                } else {
                    let val = if ov.is_none() {
                        Value::Null
                    } else {
                        json!(ov.value())
                    };
                    json!({ "questionType": format_stmt_qt(&types[oi]), "value": val })
                }
            })
            .collect();
        obj["claims"] = json!(claims);
    }
    eprintln!("{obj}");
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
            trace_question(
                qi,
                qt,
                &options[qi],
                option_count,
                true_stmt_question_types.as_ref(),
                rng,
            );
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

/// Option row for the scoped-sameness types, built list-first: the key doesn't fix
/// which option is correct, the sampled candidate list does. Key-first can't produce
/// the row where "none of these" is right *although* unlisted sharers exist.
///
/// Sample `option_count` of the eligible values — every question but `qi` and `exclude`
/// (the reference, for `OnlySameAsAmong`), plus NONE as an ordinary member — then repair the
/// sample to hold exactly one candidate answered `matched`:
///
/// - **1** → that candidate is the correct value;
/// - **0** → NONE is correct, swapped in if it wasn't sampled;
/// - **≥ 2** → keep one, swap the rest for unsampled non-sharers (or NONE).
///
/// Repair rather than re-sample: one pass, and it only touches samples that already have
/// a sharer, so the none-answered rate stays whatever sampling gave it.
///
/// With a sharer kept, NONE is a distractor like any other and its row rate is then held
/// to `none_rate` ([`none_distractor_rate`]) — the one case where this function moves NONE
/// for a reason other than correctness.
///
/// Needs `option_count` eligible values with `option_count - 1` non-sharers among them —
/// `random_type_params`' capacity gate, rearranged. Without it the ≥ 2 repair
/// runs out of substitutes and leaves a second sharer as an alternate correct answer.
fn fill_scoped_sameness(
    qi: usize,
    matched: Answer,
    exclude: Option<usize>,
    solution: &[Answer; MAX_N],
    n: usize,
    option_count: usize,
    none_rate: f64,
    rng: &mut Rng,
    slots: &mut [OptionValue; 5],
) {
    let shares = |v: OptionValue| v.is_num() && solution[usize::from(v.value())] == matched;

    // Eligible values, shuffled: the first `option_count` are the sample, the rest
    // are the substitutes the repair below draws from.
    let mut eligible: ArrayVec<OptionValue, { MAX_N + 1 }> = (0..n)
        .filter(|&j| j != qi && Some(j) != exclude)
        .map(|j| OptionValue::num(j as u8))
        .chain(std::iter::once(OptionValue::NONE))
        .collect();
    rng.shuffle(&mut eligible);
    let non_sharers = eligible.iter().filter(|&&v| !shares(v)).count();
    if eligible.len() < option_count || non_sharers < option_count - 1 {
        panic!(
            "fill_one_question: scoped sameness at qi={qi} matching {matched:?} — \
             {} eligible values ({non_sharers} non-sharing) can't fill {option_count} \
             options — missing upstream guard",
            eligible.len()
        );
    }
    // Keep the first sampled sharer; swap every later one out for an unsampled
    // non-sharer (NONE counts, if it isn't already in the row). A sample with one
    // sharer falls through unchanged — same loop, nothing to swap.
    let mut kept = None;
    let mut next_substitute = option_count;
    for i in 0..option_count {
        if !shares(eligible[i]) {
            continue;
        }
        if kept.is_none() {
            kept = Some(eligible[i]);
            continue;
        }
        // Guaranteed by the capacity check: at least `option_count - 1` non-sharers
        // are eligible and fewer than that are sampled, so the tail always has one.
        let substitute = (next_substitute..eligible.len())
            .find(|&t| !shares(eligible[t]))
            .expect("capacity check guarantees a substitute");
        eligible.swap(i, substitute);
        next_substitute = substitute + 1;
    }

    let correct_val = match kept {
        Some(sharer) => sharer,
        None => {
            // No listed sharer, so NONE is correct — displacing an arbitrary sampled
            // value if it wasn't drawn (all of them are non-sharers here).
            if !eligible[..option_count].iter().any(|v| v.is_none()) {
                let none_at = eligible
                    .iter()
                    .position(|v| v.is_none())
                    .expect("NONE is always eligible");
                eligible.swap(0, none_at);
            }
            OptionValue::NONE
        }
    };

    // NONE is a distractor only when a sharer was kept, so hold its rate there. Both swaps
    // keep "exactly one listed sharer": moving NONE in displaces a non-sharer (never
    // `correct_val`, which has to stay listed), and moving it out brings in a non-sharer — a
    // sharer would become a second valid answer.
    if kept.is_some() {
        let show = rng.next_f64() < none_rate;
        let at = eligible
            .iter()
            .position(|v| v.is_none())
            .expect("NONE is always eligible");
        if (at < option_count) != show {
            if show {
                let displaceable: ArrayVec<usize, 5> = (0..option_count)
                    .filter(|&i| eligible[i] != correct_val)
                    .collect();
                eligible.swap(at, rng.pick(&displaceable));
            } else if let Some(slot) =
                (option_count..eligible.len()).find(|&t| !shares(eligible[t]))
            {
                eligible.swap(at, slot);
            }
        }
    }

    let correct_oi = solution[qi].idx();
    slots[correct_oi] = correct_val;
    let mut distractors = [OptionValue::UNUSED; 4];
    let mut di = 0;
    for &v in &eligible[..option_count] {
        if v != correct_val {
            distractors[di] = v;
            di += 1;
        }
    }
    rng.shuffle(&mut distractors[..di]);
    place_distractors(&distractors, slots, correct_oi);
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

/// The correct option value for `qt` under solution `sol`. `NONE` is a real answer for
/// the kinds that allow it.
///
/// Five kinds have no such value and assert instead: identity options and `TrueStmt`,
/// whose rows aren't value-encoded, and the scoped-sameness pair, whose correct value
/// depends on which candidates the row lists rather than on the key (see
/// `fill_scoped_sameness`). Nothing asks — `fill_one_question` returns before this call
/// for all five, and `STMT_KINDS` excludes them.
fn correct_option_value(
    qt: &QuestionType,
    qi: usize,
    sol: &[Answer; MAX_N],
    n: usize,
    option_count: usize,
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
        QuestionType::OnlySame => pos_or_none((0..n).find(|&i| i != qi && sol[i] == sol[qi])),
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
            LETTERS[..option_count]
                .iter()
                .find(|&&l| l != answer && count_letter(sol, l, n) == ref_count)
                .map_or(OptionValue::NONE, |l| num(l.idx()))
        }
        QuestionType::LetterDist { question_index } => num(usize::from(
            (sol[qi] as u8).abs_diff(sol[question_index as usize] as u8),
        )),
        // Letter-valued: the correct option is the extreme-count letter's index,
        // matching how `place_letter_distractors` encodes these rows. Ties resolve
        // to the first such letter and are rejected by `check_claim_fast`.
        QuestionType::MostCommon | QuestionType::LeastCommon => {
            let counts = &letter_counts(sol, n)[..option_count];
            let target = if matches!(*qt, QuestionType::MostCommon) {
                counts.iter().max()
            } else {
                counts.iter().min()
            };
            target
                .and_then(|&t| counts.iter().position(|&c| c == t))
                .map(num)
                .expect("counts is non-empty, so its extremum is one of its elements")
        }
        // No key-determined value. Listed rather than a catch-all so a new variant is
        // a compile error here until its correct value is decided.
        QuestionType::NoOtherHasAnswer
        | QuestionType::AnswerIsSelf
        | QuestionType::TrueStmt
        | QuestionType::OnlySameAmong
        | QuestionType::OnlySameAsAmong { .. } => {
            unreachable!("{:?} has no correct value fixed by the key", qt.kind())
        }
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

/// Share of `kind`'s instances at `level` (0-based, as `recipes::RECIPES` is indexed) whose
/// answer is NONE, or `None` where the kind isn't used at that level. Measured with
/// `type-stats --attempts 10000 --seed 1`, to two decimals — a third digit sits below the
/// measurement's own run-to-run reproducibility.
///
/// Only shares up to `1/option_count` (0.2 at oc=5) can be neutralized from the option row;
/// past that only placement or row sampling can bring one down. `1.0` would be a degenerate
/// type — NONE always the answer, so never a distractor.
///
/// Absence is spelled `0.00`, with two traps. A kind that *is* used but never none-answered
/// wants NONE withheld rather than offered freely, so it must not be written that way;
/// nothing is in that position, and such a kind would show up as `may_be_none` with no NONE
/// answer anywhere in a shipped year. And a true share under 0.005 rounds to `0.00` and reads
/// as absent — the smallest real value is 0.03.
///
/// This measures the generator's own output, so any change to generation stales it.
fn none_correct_rate(kind: QuestionTypeKind, level: usize) -> Option<f64> {
    // A row per kind and a column per level: a kind's progression across levels is the
    // thing worth reading, and a column left stale by a recipe change shows up as an
    // outlier next to its neighbors. `0.00` = the kind isn't used at that level.
    const fn by_level(kind: QuestionTypeKind) -> [f64; 6] {
        use QuestionTypeKind::*;
        match kind {
            //               L1    L2    L3    L4    L5    L6
            ClosestAfter => [0.65, 0.00, 0.41, 0.25, 0.23, 0.18],
            ClosestBefore => [0.67, 0.00, 0.42, 0.26, 0.22, 0.19],
            FirstWith => [0.39, 0.31, 0.33, 0.09, 0.07, 0.03],
            LastWith => [0.40, 0.32, 0.32, 0.10, 0.06, 0.03],
            PrevSame => [0.50, 0.00, 0.43, 0.32, 0.26, 0.21],
            NextSame => [0.51, 0.00, 0.45, 0.32, 0.25, 0.21],
            OnlySame => [0.00, 0.00, 0.00, 0.09, 0.05, 0.03],
            OnlySameAmong => [0.69, 0.00, 0.52, 0.42, 0.38, 0.36],
            OnlySameAsAmong => [0.00, 0.00, 0.00, 0.00, 0.43, 0.40],
            OnlyOdd => [0.00, 0.00, 0.00, 0.00, 0.45, 0.42],
            OnlyEven => [0.00, 0.00, 0.00, 0.00, 0.44, 0.42],
            ConsecIdent => [0.00, 0.00, 0.00, 0.00, 0.12, 0.10],
            EqualCount => [0.00, 0.00, 0.00, 0.00, 0.49, 0.42],

            // No NONE option, so no rate to hold. Listed rather than a catch-all so a
            // new kind has to decide, the way `may_be_none` does.
            CountAnswer | CountAnswerBefore | CountAnswerAfter | CountVowel | CountConsonant
            | MostCommonCount | AnswerOf | LeastCommon | MostCommon | NoOtherHasAnswer
            | AnswerIsSelf | LetterDist | TrueStmt => [0.0; 6],
        }
    }
    let rate = by_level(kind)[level];
    debug_assert!(
        (0.0..1.0).contains(&rate),
        "{kind:?} L{}: NONE-correct rate {rate} is not a share below 1",
        level + 1
    );
    (rate > 0.0).then_some(rate)
}

/// How often NONE should be offered as a distractor, given that it isn't the answer.
///
/// Returns a rate between 0 and 1:
///
/// - 0 — never offer NONE as a distractor.
/// - 1 — offer it whenever it isn't the answer.
/// - 0.5 — offer it in half the cases where it isn't the answer.
///
/// Derived from how often NONE is the *answer* for this kind, as measured by `type-stats`. The
/// aim is for NONE's presence to tell the player nothing: on a row that offers it, NONE is
/// correct `1/option_count` of the time, so neither picking it on sight nor eliminating it
/// beats a guess.
///
/// A target, not a guarantee: generation rejects some of the puzzles that showing NONE
/// produces, so accepted ones under-represent it, and a kind whose value pool is no larger
/// than `option_count` shows every value anyway. `type-stats` measures what came out.
fn none_distractor_rate(kind: QuestionTypeKind, n: usize, option_count: usize) -> f64 {
    // Most questions are of a kind with no NONE in its value pool, which never reads this.
    // Same answer as the lookup would give, without doing it.
    if !kind.may_be_none() {
        return 1.0;
    }
    let Some(p) = exact_recipe(n).and_then(|r| none_correct_rate(kind, r.level_index)) else {
        // Uncalibrated, so don't restrict NONE: a kind the level doesn't list, or a board size
        // no level uses — which only the property tests produce, fuzzing `n`.
        return 1.0;
    };

    // Share of all this kind's questions that should show NONE as a distractor. A question has
    // one correct slot but `oc-1` distractor slots, so a distractor-share is diluted by that
    // factor — matching a correct-share of `p` takes `p·(oc-1)`.
    let d = p * (option_count - 1) as f64;

    // Rescaled over just the `1 - p` where NONE isn't the answer — the only ones a caller gets
    // to choose for.
    let rate_when_not_the_answer = d / (1.0 - p);

    // Most calibrated kind/level pairs answer NONE more often than `1/oc`, which asks for it as
    // a distractor on more questions than it is wrong on. Neutral is then out of reach from the
    // row, so show NONE whenever it isn't the answer and leave the rest to `p`.
    if rate_when_not_the_answer > 1.0 {
        return 1.0;
    }

    rate_when_not_the_answer
}

/// Move NONE in or out of the visible prefix of a shuffled distractor pool, to match the rate
/// [`none_distractor_rate`] asks for. No-op if the pool holds no NONE, or if the move is
/// impossible: pushing NONE *out* needs a slot past `visible` to park it in, which a `pool` no
/// longer than `visible` hasn't got.
fn set_none_shown(pool: &mut [OptionValue], visible: usize, show: bool, rng: &mut Rng) {
    if visible == 0 {
        return;
    }
    let Some(at) = pool.iter().position(|v| v.is_none()) else {
        return;
    };
    if (at < visible) == show {
        return;
    }
    if show {
        pool.swap(at, rng.int(0, visible as i32 - 1) as usize);
    } else if pool.len() > visible {
        pool.swap(at, rng.int(visible as i32, pool.len() as i32 - 1) as usize);
    }
}

fn pick_distractors(
    vals: &ArrayVec<OptionValue, MAX_VALUE_POOL>,
    correct: OptionValue,
    option_count: usize,
    none_rate: f64,
    rng: &mut Rng,
) -> [OptionValue; 4] {
    let mut pool = [OptionValue::UNUSED; MAX_VALUE_POOL];
    let mut plen = 0;
    let mut has_none = false;
    for &v in vals {
        if v != correct {
            has_none |= v.is_none();
            pool[plen] = v;
            plen += 1;
        }
    }
    rng.shuffle(&mut pool[..plen]);
    // Only the first `option_count - 1` entries reach visible slots (see `place_distractors`),
    // so NONE is on the row iff it lands in that prefix. Drawn only when a NONE is present,
    // leaving the rng stream of kinds without one untouched.
    if has_none {
        let show = rng.next_f64() < none_rate;
        set_none_shown(&mut pool[..plen], option_count - 1, show, rng);
    }
    let mut result = [OptionValue::UNUSED; 4];
    result[..4.min(plen)].copy_from_slice(&pool[..4.min(plen)]);
    result
}

/// Distractor placement for number-valued questions (counts, positions,
/// distances, question-indices — everything but the letter-valued AnswerOf /
/// LeastCommon / MostCommon): correct option at `correct_oi`, the rest drawn
/// from `val_pool` (shuffled). `val_pool` is numeric, plus a `NONE` option for
/// the positional/sameness types, offered at `none_rate` (see
/// [`none_distractor_rate`]).
fn place_numeric_distractors(
    slots: &mut [OptionValue; 5],
    correct_oi: usize,
    correct_val: OptionValue,
    val_pool: &ArrayVec<OptionValue, MAX_VALUE_POOL>,
    option_count: usize,
    none_rate: f64,
    rng: &mut Rng,
) {
    slots[correct_oi] = correct_val;
    let distractors = pick_distractors(val_pool, correct_val, option_count, none_rate, rng);
    place_distractors(&distractors, slots, correct_oi);
}

/// Letter-distractor placement for AnswerOf / LeastCommon / MostCommon, whose
/// distractors are simply "the other letters": put `correct_letter` at
/// `correct_oi`, then the remaining `letters` (shuffled) in the other slots.
fn place_letter_distractors(
    slots: &mut [OptionValue; 5],
    correct_oi: usize,
    correct_letter: Answer,
    letters: &[Answer],
    option_count: usize,
    rng: &mut Rng,
) {
    slots[correct_oi] = OptionValue::num(correct_letter as u8);
    let mut pool = [Answer::A; 4];
    let mut plen = 0;
    for &l in letters {
        if l != correct_letter {
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

// ── Statements for TrueStmt ──

fn stmt_category(claim: &Claim) -> u16 {
    match claim.question_type {
        QuestionType::CountAnswer { answer } => 100 + answer as u16,
        QuestionType::CountConsonant => 200,
        QuestionType::CountVowel => 201,
        QuestionType::CountAnswerAfter { answer, .. } => 300 + answer as u16,
        QuestionType::CountAnswerBefore { answer, .. } => 400 + answer as u16,
        QuestionType::AnswerOf { question_index } => 500 + question_index as u16,
        QuestionType::FirstWith { answer } => 600 + answer as u16,
        QuestionType::LastWith { answer } => 700 + answer as u16,
        QuestionType::MostCommon => 800,
        // ClosestAfter/Before vary by both answer and index, so fold them into one
        // category number. Stride 20 (> MAX_N) gives each answer its own index band,
        // and 5 answers * 20 + max index stays under the +100 gap to the next kind.
        QuestionType::ClosestAfter {
            answer,
            after_index,
        } => 900 + answer as u16 * 20 + after_index as u16,
        QuestionType::ClosestBefore {
            answer,
            before_index,
        } => 1000 + answer as u16 * 20 + before_index as u16,
        QuestionType::MostCommonCount => 1100,
        QuestionType::LeastCommon => 1200,
        QuestionType::NoOtherHasAnswer => 1300,
        QuestionType::EqualCount { answer } => 1400 + answer as u16,
        QuestionType::ConsecIdent => 1500,
        QuestionType::OnlyOdd { answer } => 1600 + answer as u16,
        QuestionType::OnlyEven { answer } => 1700 + answer as u16,
        // Uncategorized: none of these is a generated statement kind, so
        // `try_make_stmt` never produces one. Listed rather than a catch-all so a new
        // variant is a compile error here until it's given a category.
        QuestionType::PrevSame
        | QuestionType::NextSame
        | QuestionType::OnlySame
        | QuestionType::OnlySameAmong
        | QuestionType::OnlySameAsAmong { .. }
        | QuestionType::AnswerIsSelf
        | QuestionType::LetterDist { .. }
        | QuestionType::TrueStmt => {
            unreachable!("{:?} has no statement category", claim.question_type.kind())
        }
    }
}

fn build_stmts(
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

    let true_stmt = make_true_stmt(solution, qi, n, rng, option_count);
    local[target_oi] = Some(true_stmt);

    for oi in 0..option_count {
        if oi == target_oi {
            continue;
        }
        let mut found = false;
        for _ in 0..30 {
            let fc = make_false_stmt(solution, qi, n, rng, option_count);
            let cat = stmt_category(&fc);
            if cat != stmt_category(local[target_oi].as_ref().unwrap())
                && (0..oi).all(|j| {
                    j == target_oi || local[j].as_ref().is_none_or(|c| stmt_category(c) != cat)
                })
            {
                local[oi] = Some(fc);
                found = true;
                break;
            }
        }
        if !found {
            local[oi] = Some(make_false_stmt(solution, qi, n, rng, option_count));
        }
    }

    // Split into SoA: values live in `slots`, types in `true_stmt_question_types`.
    // Slots with no statement (oi >= option_count) stay UNUSED; the matching type
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

/// The statement kinds, derived at compile time from `check_form::check_stmt_kind`
/// so the pick pool and the form check can't drift apart. Only kinds it passes
/// silently are generated — a warning there exists to tolerate what older corpora
/// already shipped, not to license new ones.
const STMT_KIND_COUNT: usize = {
    let all = QuestionTypeKind::all();
    let mut count = 0;
    let mut i = 0;
    while i < all.len() {
        if check_form::check_stmt_kind(all[i]).is_none() {
            count += 1;
        }
        i += 1;
    }
    count
};

const STMT_KINDS: [QuestionTypeKind; STMT_KIND_COUNT] = {
    let all = QuestionTypeKind::all();
    let mut out = [QuestionTypeKind::CountAnswer; STMT_KIND_COUNT];
    let mut i = 0;
    let mut j = 0;
    while i < all.len() {
        if check_form::check_stmt_kind(all[i]).is_none() {
            out[j] = all[i];
            j += 1;
        }
        i += 1;
    }
    out
};

fn try_make_stmt(
    sol: &[Answer; MAX_N],
    qi: usize,
    n: usize,
    rng: &mut Rng,
    option_count: usize,
) -> Option<Claim> {
    // Generate a statement type exactly as question types are generated, then take its
    // true value for this solution. `is_num` drops null values (TrueStmt statements never
    // assert null); `check_claim_fast` drops types whose true value isn't a valid
    // unique statement here (non-unique OnlyOdd/ConsecIdent, MostCommon/LeastCommon tie).
    let kind = rng.pick(&STMT_KINDS);
    let question_type = random_type_params(kind, qi, n, option_count, sol, rng)?;
    let value = correct_option_value(&question_type, qi, sol, n, option_count);
    if !value.is_num() {
        return None;
    }
    let claim = Claim {
        question_type,
        value,
    };
    check_claim_fast(option_count, &sol[..n], qi, &claim).then_some(claim)
}

fn make_true_stmt(
    sol: &[Answer; MAX_N],
    qi: usize,
    n: usize,
    rng: &mut Rng,
    option_count: usize,
) -> Claim {
    for _ in 0..20 {
        if let Some(claim) = try_make_stmt(sol, qi, n, rng, option_count) {
            return claim;
        }
    }
    let a = rng.pick_letter(option_count);
    Claim {
        question_type: QuestionType::CountAnswer { answer: a },
        value: OptionValue::num(count_letter(sol, a, n) as u8),
    }
}

/// A plausible wrong value for a statement of type `qt` given its correct value: prefer
/// a near-miss (correct ±1/±2) that's a real option, else any other option value.
/// Never NONE — TrueStmt statements don't assert null. `check_claim_fast` at the call
/// site is the final arbiter of falseness (an EqualCount near-miss, say, can land on
/// a second true answer).
fn false_stmt_value(
    qt: &QuestionType,
    correct: OptionValue,
    qi: usize,
    n: usize,
    option_count: usize,
    rng: &mut Rng,
) -> Option<OptionValue> {
    let pool = valid_values(qt, qi, n, option_count);
    let c = correct.value() as i32;
    let near: ArrayVec<OptionValue, 4> = [-2, -1, 1, 2]
        .into_iter()
        .filter(|&off| c + off >= 0)
        .map(|off| OptionValue::num((c + off) as u8))
        .filter(|&v| v != correct && pool.contains(&v))
        .collect();
    if !near.is_empty() {
        return Some(rng.pick(&near));
    }
    let rest: ArrayVec<OptionValue, MAX_VALUE_POOL> = pool
        .iter()
        .copied()
        .filter(|&v| v.is_num() && v != correct)
        .collect();
    (!rest.is_empty()).then(|| rng.pick(&rest))
}

fn make_false_stmt(
    sol: &[Answer; MAX_N],
    qi: usize,
    n: usize,
    rng: &mut Rng,
    option_count: usize,
) -> Claim {
    for _ in 0..30 {
        let base = make_true_stmt(sol, qi, n, rng, option_count);
        if let Some(value) =
            false_stmt_value(&base.question_type, base.value, qi, n, option_count, rng)
        {
            let fc = Claim {
                question_type: base.question_type,
                value,
            };
            if !check_claim_fast(option_count, &sol[..n], qi, &fc) {
                return fc;
            }
        }
    }
    // Give up: emit a guaranteed-false but in-range CountAnswer(A) statement. The
    // true count of A is `count_a`; any other value in 0..=n is false, so use
    // count+1 (or count-1 when the count is already at the ceiling n).
    let count_a = count_letter(sol, Answer::A, n);
    let value = if count_a < n as i32 {
        count_a + 1
    } else {
        count_a - 1
    };
    Claim {
        question_type: QuestionType::CountAnswer { answer: Answer::A },
        value: OptionValue::num(value as u8),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::check_answer::check_answer;
    use crate::recipes::RECIPES;
    use serde_json::Value;

    /// `none_correct_rate` stores one column per level, so a recipe added or removed
    /// leaves its rows the wrong width.
    #[test]
    fn none_correct_rate_has_a_column_per_level() {
        assert_eq!(
            RECIPES.len(),
            6,
            "none_correct_rate's rows are 6 wide but there are {} levels",
            RECIPES.len()
        );
    }

    /// `none_distractor_rate` is actually honored by the row builder: over random keys,
    /// the share of non-NONE-answered rows that show NONE tracks the requested rate.
    /// Measured straight out of `fill_one_question`, so the generator's accept gate
    /// can't mask or flatter the mechanism.
    #[test]
    fn none_rate_is_honored_at_fill_level() {
        // (kind-bearing type, n, oc). Pools here are all bigger than `oc`, so the rate
        // is reachable in both directions.
        let cases: [(QuestionType, usize, usize); 4] = [
            (QuestionType::FirstWith { answer: Answer::A }, 3, 3),
            (QuestionType::FirstWith { answer: Answer::A }, 12, 5),
            (QuestionType::OnlySame, 12, 5),
            (
                QuestionType::ClosestAfter {
                    after_index: 0,
                    answer: Answer::A,
                },
                12,
                5,
            ),
        ];
        for (qt, n, option_count) in cases {
            for &rate in &[0.0, 0.25, 0.6, 1.0] {
                let mut shown = 0u32;
                let mut eligible = 0u32;
                let mut rng = Rng::new(0xC0FFEE);
                for _ in 0..4000 {
                    let mut solution = [Answer::A; MAX_N];
                    for i in 0..n {
                        solution[i] = rng.pick_letter(option_count);
                    }
                    let mut slots = [OptionValue::UNUSED; 5];
                    let correct_oi = solution[0].idx();
                    let val_pool = valid_values(&qt, 0, n, option_count);
                    let correct_val = correct_option_value(&qt, 0, &solution, n, option_count);
                    if correct_val.is_none() {
                        continue; // NONE is the answer, not a distractor
                    }
                    eligible += 1;
                    place_numeric_distractors(
                        &mut slots,
                        correct_oi,
                        correct_val,
                        &val_pool,
                        option_count,
                        rate,
                        &mut rng,
                    );
                    if (0..option_count).any(|oi| oi != correct_oi && slots[oi].is_none()) {
                        shown += 1;
                    }
                }
                assert!(eligible > 200, "{qt:?} n={n}: only {eligible} usable keys");
                let got = shown as f64 / eligible as f64;
                assert!(
                    (got - rate).abs() < 0.05,
                    "{qt:?} n={n} oc={option_count}: asked for NONE on {rate:.2} of rows, \
                     got {got:.2}"
                );
            }
        }
    }

    /// A NONE-answerable kind needs a rate at every level that actually allows it, or
    /// generation quietly takes the uncalibrated path there. And no kind lacking a NONE option
    /// may carry a rate, which would do nothing.
    #[test]
    fn none_correct_rate_matches_recipe_pools() {
        for &kind in QuestionTypeKind::all() {
            for (level, recipe) in RECIPES.iter().enumerate() {
                let calibrated = none_correct_rate(kind, level).is_some();
                assert!(
                    !calibrated || kind.may_be_none(),
                    "{kind:?} L{} has a NONE-correct rate but no NONE option",
                    level + 1
                );
                let allowed = recipe.allowed.contains(&kind)
                    || recipe.required.iter().any(|&(k, _)| k == kind);
                if allowed && kind.may_be_none() {
                    assert!(
                        calibrated,
                        "{kind:?} is allowed at L{} but uncalibrated there — re-run type-stats \
                         and fill in the column",
                        level + 1
                    );
                }
            }
        }
    }

    /// `may_be_none` must agree with a whole shipped year: a NONE correct answer may
    /// only occur for a `may_be_none` kind, and every `may_be_none` kind must actually
    /// occur with a NONE answer somewhere in the year — not witnessable across a full
    /// year ⇒ not meaningfully reachable.
    #[test]
    fn may_be_none_agrees_with_corpus() {
        use crate::serialize::parse_puzzle;
        use crate::solve_brute::solve;

        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../public/puzzles/daily/2027.json");
        let text = std::fs::read_to_string(&path).expect("read 2027.json");
        let data: Value = serde_json::from_str(&text).unwrap();

        let mut witnessed = [false; QUESTION_KIND_COUNT];
        for levels in data.as_object().unwrap().values() {
            for puzzle in levels.as_object().unwrap().values() {
                let fp = parse_puzzle(puzzle).expect("parse 2027 puzzle");
                let sols = solve(&fp, 2);
                assert_eq!(sols.len(), 1, "2027 puzzle is not uniquely solvable");
                for qi in 0..fp.n {
                    let kind = fp.question_types[qi].kind();
                    // TrueStmt's answer selects a statement (an index), never a NONE value;
                    // its stored option values are statement values, a different layer.
                    if kind == QuestionTypeKind::TrueStmt {
                        continue;
                    }
                    if fp.options[qi][sols[0][qi].idx()].is_none() {
                        assert!(
                            kind.may_be_none(),
                            "NONE is the answer for non-may_be_none {kind:?}"
                        );
                        witnessed[kind as usize] = true;
                    }
                }
            }
        }

        for &kind in QuestionTypeKind::all() {
            assert_eq!(
                kind.may_be_none(),
                witnessed[kind as usize],
                "{kind:?}: may_be_none={} but corpus-witnessed={}",
                kind.may_be_none(),
                witnessed[kind as usize]
            );
        }
    }

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
                solution[i] = Answer::from(ch as u8 - b'A');
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
                if !crate::check_answer::check_all_answers(&fp, &answers) {
                    eprintln!("FAIL: {name} (seed={seed}): check_all_answers rejected");
                    for qi in 0..n {
                        let state = State {
                            answers,
                            eliminated: [fp.initial_eliminated_mask(); MAX_N],
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

                // Scoped-sameness invariant (what `expectedCorrect` can no longer
                // pin, since list-first sampling makes the correct value
                // rng-dependent): the correct slot holds the *only* listed
                // candidate sharing the matched letter, or NONE exactly when no
                // listed candidate shares it. Unlisted sharers are irrelevant.
                for qi in 0..n {
                    let (matched, excluded) = match fp.question_types[qi] {
                        QuestionType::OnlySameAmong => (solution[qi], None),
                        QuestionType::OnlySameAsAmong { question_index } => {
                            let r = usize::from(question_index);
                            (solution[r], Some(r))
                        }
                        _ => continue,
                    };
                    let correct_oi = solution[qi].idx();
                    let listed_sharers: Vec<u8> = (0..oc)
                        .filter_map(|oi| {
                            let ov = fp.options[qi][oi];
                            let j = ov.is_num().then(|| usize::from(ov.value()))?;
                            (j < n && j != qi && Some(j) != excluded && solution[j] == matched)
                                .then_some(j as u8)
                        })
                        .collect();
                    let stored = fp.options[qi][correct_oi];
                    let ok = match listed_sharers.as_slice() {
                        [] => stored.is_none(),
                        [only] => stored.is_num() && stored.value() == *only,
                        _ => false,
                    };
                    if !ok {
                        eprintln!(
                            "FAIL: {name} (seed={seed}) Q{}: correct slot {stored:?} vs listed sharers {listed_sharers:?} in {:?}",
                            qi + 1,
                            &fp.options[qi][..oc]
                        );
                        case_failed = true;
                        break;
                    }
                }
                if case_failed {
                    break;
                }

                // Distinctness: distractor option values must differ from the correct value
                // and from each other (across the active option count). TrueStmt is skipped
                // — its rows hold per-claim values, which may legitimately repeat.
                for qi in 0..n {
                    let qt = &fp.question_types[qi];
                    if matches!(qt, QuestionType::TrueStmt) {
                        continue;
                    }
                    let slots = &fp.options[qi];
                    let mut seen: Vec<u8> = Vec::new();
                    for &ov in &slots[..oc] {
                        if !ov.is_num() {
                            continue;
                        }
                        let ov = ov.value();
                        if seen.contains(&ov) {
                            eprintln!(
                                "FAIL: {name} (seed={seed}) Q{}: duplicate option value {ov} in {:?}",
                                qi + 1,
                                &slots[..oc]
                            );
                            case_failed = true;
                            break;
                        }
                        seen.push(ov);
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

        // NoOtherHasAnswer / AnswerIsSelf use identity options, TrueStmt uses statements;
        // the parser overrides input, so checkForm's range warning is unobservable.
        let exempt = [
            QuestionTypeKind::NoOtherHasAnswer,
            QuestionTypeKind::AnswerIsSelf,
            QuestionTypeKind::TrueStmt,
        ];
        let value_typed: Vec<QuestionTypeKind> = QuestionTypeKind::all()
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
                let errors = check_form(&fp);
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
