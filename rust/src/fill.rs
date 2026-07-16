//! Option filling: given a skeleton's question types and answer key, encode each
//! question's full option row — the correct value plus its distractors — and the
//! per-option claims for TrueStmt questions.

use arrayvec::ArrayVec;
use serde_json::{Value, json};

use crate::check_answer::check_claim_fast;
use crate::check_form;
use crate::construct::{format_claim_qt, random_type_params};
use crate::format::format_type_tag;
use crate::rng::Rng;
use crate::types::*;

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
        QuestionType::SameAs | QuestionType::OnlySame => {
            for v in 0..n {
                if v != qi {
                    push_num(v);
                }
            }
            out.push(OptionValue::NONE);
        }
        QuestionType::SameAsWhich { question_index } => {
            // Structural domain only: any other real question except self (qi) and
            // the referenced question. NONE is never valid — a well-formed
            // SameAsWhich always has a match (fill_one_question guarantees it).
            // Whether a candidate is *also* a valid answer (target shares the ref's
            // answer) is key-dependent and enforced downstream by
            // check_well_posed_given_options, not here.
            for v in 0..n {
                if v != qi && v != question_index as usize {
                    push_num(v);
                }
            }
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

pub(crate) fn assert_accepted(fp: &FlatPuzzle, brute_count: usize, label: &str) {
    assert_eq!(
        brute_count, 1,
        "BUG [{label}]: expected 1 solution, got {brute_count}"
    );
    let fe = check_form::check_form(fp);
    assert!(
        fe.is_empty(),
        "BUG [{label}]: form errors: {}",
        fe.iter()
            .map(|e| format!("Q{}: {}", e.qi + 1, e.message))
            .collect::<Vec<_>>()
            .join(", ")
    );
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
            place_numeric_distractors(slots, correct_oi, correct_val, &val_pool, rng);
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
            place_numeric_distractors(slots, correct_oi, correct_val, &val_pool, rng);
        }
        QuestionType::LetterDist { .. } => {
            place_numeric_distractors(slots, correct_oi, correct_val, &val_pool, rng);
        }
        _ if is_counting_type(qt) => {
            place_numeric_distractors(slots, correct_oi, correct_val, &val_pool, rng);
        }
        QuestionType::SameAsWhich { question_index } => {
            if !correct_val.is_num() {
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
            place_numeric_distractors(slots, correct_oi, correct_val, &val_pool, rng);
        }
        QuestionType::ClosestAfter { .. }
        | QuestionType::ClosestBefore { .. }
        | QuestionType::FirstWith { .. }
        | QuestionType::LastWith { .. }
        | QuestionType::PrevSame
        | QuestionType::NextSame => {
            place_numeric_distractors(slots, correct_oi, correct_val, &val_pool, rng);
        }
        _ => unreachable!(),
    }
}

/// Emit one question's filled-options trace line (diagnostic `trace` mode only).
/// `true_stmt_types` must be `Some` for a TrueStmt question (its claim types).
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
            if matches!(qt, QuestionType::TrueStmt) || ov.is_none() || ov.is_unused() {
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
                    json!({ "questionType": format_claim_qt(&types[oi]), "value": val })
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

/// The correct option value for `qt` under solution `sol`. `NONE` is a real answer
/// for kinds that allow it; `UNUSED` is the "no valid value" sentinel for degenerate
/// cases.
pub fn correct_option_value(
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
        QuestionType::OnlySame | QuestionType::SameAs => {
            pos_or_none((0..n).find(|&i| i != qi && sol[i] == sol[qi]))
        }
        QuestionType::SameAsWhich { question_index } => {
            // NONE is never valid here; a no-match is degenerate → UNUSED, not NONE.
            let ref_ans = sol[question_index as usize];
            (0..n)
                .find(|&i| i != qi && i != question_index as usize && sol[i] == ref_ans)
                .map_or(OptionValue::UNUSED, num)
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
                .map_or(OptionValue::UNUSED, num)
        }
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
    vals: &ArrayVec<OptionValue, MAX_VALUE_POOL>,
    correct: OptionValue,
    rng: &mut Rng,
) -> [OptionValue; 4] {
    let mut pool = [OptionValue::UNUSED; MAX_VALUE_POOL];
    let mut plen = 0;
    for &v in vals {
        if v != correct {
            pool[plen] = v;
            plen += 1;
        }
    }
    rng.shuffle(&mut pool[..plen]);
    let mut result = [OptionValue::UNUSED; 4];
    result[..4.min(plen)].copy_from_slice(&pool[..4.min(plen)]);
    result
}

/// Distractor placement for number-valued questions (counts, positions,
/// distances, question-indices — everything but the letter-valued AnswerOf /
/// LeastCommon / MostCommon): correct option at `correct_oi`, the rest drawn
/// from `val_pool` (shuffled). `val_pool` is numeric, plus a `NONE` option for
/// the positional/sameness types.
fn place_numeric_distractors(
    slots: &mut [OptionValue; 5],
    correct_oi: usize,
    correct_val: OptionValue,
    val_pool: &ArrayVec<OptionValue, MAX_VALUE_POOL>,
    rng: &mut Rng,
) {
    slots[correct_oi] = correct_val;
    let distractors = pick_distractors(val_pool, correct_val, rng);
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

/// Whether a question kind can be a TrueStmt claim.
const fn is_claim_type(kind: QuestionTypeKind) -> bool {
    use QuestionTypeKind::*;
    match kind {
        CountAnswer | CountAnswerBefore | CountAnswerAfter | CountVowel | CountConsonant
        | MostCommonCount | ClosestAfter | ClosestBefore | FirstWith | LastWith | OnlyOdd
        | OnlyEven | ConsecIdent | LeastCommon | MostCommon | EqualCount | SameAsWhich => true,

        // Relative to own question's answer is slightly confusing for `TrueStmt` claims.
        PrevSame | NextSame | OnlySame | SameAs | LetterDist | AnswerOf | NoOtherHasAnswer => false,
        // Uninteresting.
        AnswerIsSelf => false,
        // Recursive claims aren't supported.
        TrueStmt => false,
    }
}

/// The claim kinds, derived once from `is_claim_type` so the pick pool and the
/// predicate can't drift apart.
const CLAIM_KIND_COUNT: usize = {
    let all = QuestionTypeKind::all();
    let mut count = 0;
    let mut i = 0;
    while i < all.len() {
        if is_claim_type(all[i]) {
            count += 1;
        }
        i += 1;
    }
    count
};

const CLAIM_KINDS: [QuestionTypeKind; CLAIM_KIND_COUNT] = {
    let all = QuestionTypeKind::all();
    let mut out = [QuestionTypeKind::CountAnswer; CLAIM_KIND_COUNT];
    let mut i = 0;
    let mut j = 0;
    while i < all.len() {
        if is_claim_type(all[i]) {
            out[j] = all[i];
            j += 1;
        }
        i += 1;
    }
    out
};

fn try_make_claim(
    sol: &[Answer; MAX_N],
    qi: usize,
    n: usize,
    rng: &mut Rng,
    option_count: usize,
) -> Option<Claim> {
    // Generate a claim type exactly as question types are generated, then take its
    // true value for this solution. `is_num` drops null values (TrueStmt claims never
    // assert null); `check_claim_fast` drops types whose true value isn't a valid
    // unique claim here (non-unique OnlyOdd/ConsecIdent, MostCommon/LeastCommon tie).
    let kind = rng.pick(&CLAIM_KINDS);
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

fn make_true_claim(
    sol: &[Answer; MAX_N],
    qi: usize,
    n: usize,
    rng: &mut Rng,
    option_count: usize,
) -> Claim {
    for _ in 0..20 {
        if let Some(claim) = try_make_claim(sol, qi, n, rng, option_count) {
            return claim;
        }
    }
    let a = rng.pick_letter(option_count);
    Claim {
        question_type: QuestionType::CountAnswer { answer: a },
        value: OptionValue::num(count_letter(sol, a, n) as u8),
    }
}

/// A plausible wrong value for a claim of type `qt` given its correct value: prefer
/// a near-miss (correct ±1/±2) that's a real option, else any other option value.
/// Never NONE — TrueStmt claims don't assert null. `check_claim_fast` at the call
/// site is the final arbiter of falseness (an EqualCount near-miss, say, can land on
/// a second true answer).
fn false_claim_value(
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

fn make_false_claim(
    sol: &[Answer; MAX_N],
    qi: usize,
    n: usize,
    rng: &mut Rng,
    option_count: usize,
) -> Claim {
    for _ in 0..30 {
        let base = make_true_claim(sol, qi, n, rng, option_count);
        if let Some(value) =
            false_claim_value(&base.question_type, base.value, qi, n, option_count, rng)
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
    // Give up: emit a guaranteed-false but in-range CountAnswer(A) claim. The
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
    use serde_json::Value;

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
                    // TrueStmt's answer selects a claim (an index), never a NONE value;
                    // its stored option values are claim values, a different layer.
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
                if !crate::check_answer::check_answers(&fp, &answers) {
                    eprintln!("FAIL: {name} (seed={seed}): check_answers rejected");
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

        // NoOtherHasAnswer / AnswerIsSelf use identity options, TrueStmt uses claims;
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
