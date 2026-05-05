use crate::check_validity::check_question_against_solution;
use crate::types::*;

pub fn solve(
    fp: &FlatPuzzle,
    fixed: Option<&[Option<Answer>; MAX_N]>,
    max_solutions: usize,
) -> Vec<[Answer; MAX_N]> {
    let n = fp.n;
    let default_fixed = [None; MAX_N];
    let fixed = fixed.unwrap_or(&default_fixed);
    let mut solutions: Vec<[Answer; MAX_N]> = Vec::new();
    let mut current = [None::<Answer>; MAX_N];
    let order = compute_search_order(fp);
    let all_bits: u16 = (1u16 << n) - 1;
    let mut assigned_bits: u16 = 0;

    let range_masks = compute_range_masks(fp);

    search(
        fp,
        fixed,
        &mut solutions,
        &mut current,
        &order,
        all_bits,
        &mut assigned_bits,
        &range_masks,
        0,
        max_solutions,
    );

    solutions
}

fn compute_search_order(fp: &FlatPuzzle) -> [u8; MAX_N] {
    let n = fp.n;
    let mut ref_count = [0u8; MAX_N];
    for i in 0..n {
        if let QuestionType::AnswerOf { question_index } = fp.question_types[i] {
            ref_count[question_index as usize] += 1;
        }
    }

    let mut indices: [u8; MAX_N] = std::array::from_fn(|i| i as u8);
    indices[..n].sort_by(|&a, &b| {
        let a = a as usize;
        let b = b as usize;
        ref_count[b].cmp(&ref_count[a]).then_with(|| {
            let a_global = fp.question_types[a].is_solver_global() as u8;
            let b_global = fp.question_types[b].is_solver_global() as u8;
            a_global.cmp(&b_global)
        })
    });

    indices
}

fn compute_range_masks(fp: &FlatPuzzle) -> [u16; MAX_N] {
    let n = fp.n;
    let mut masks = [0u16; MAX_N];
    for i in 0..n {
        masks[i] = match fp.question_types[i] {
            QuestionType::NextSame => {
                let mut m = 0u16;
                for j in (i + 1)..n {
                    m |= 1 << j;
                }
                m
            }
            QuestionType::ClosestAfter { after_index, .. }
            | QuestionType::CountAnswerAfter { after_index, .. } => {
                let mut m = 0u16;
                for j in (after_index as usize + 1)..n {
                    m |= 1 << j;
                }
                m
            }
            QuestionType::ClosestBefore { before_index, .. }
            | QuestionType::CountAnswerBefore { before_index, .. } => {
                let mut m = 0u16;
                for j in 0..before_index as usize {
                    m |= 1 << j;
                }
                m
            }
            _ => 0,
        };
    }
    masks
}

#[allow(clippy::too_many_arguments)]
fn search(
    fp: &FlatPuzzle,
    fixed: &[Option<Answer>; MAX_N],
    solutions: &mut Vec<[Answer; MAX_N]>,
    current: &mut [Option<Answer>; MAX_N],
    order: &[u8; MAX_N],
    all_bits: u16,
    assigned_bits: &mut u16,
    range_masks: &[u16; MAX_N],
    depth: usize,
    max_solutions: usize,
) {
    let n = fp.n;
    if solutions.len() >= max_solutions {
        return;
    }

    if depth == n {
        let mut valid = true;
        for i in 0..n {
            if !check_question_against_solution(fp, i, current[i].unwrap(), current) {
                valid = false;
                break;
            }
        }
        if valid {
            let mut copy = [Answer::A; MAX_N];
            for i in 0..n {
                copy[i] = current[i].unwrap();
            }
            solutions.push(copy);
        }
        return;
    }

    let qi = order[depth] as usize;
    let bit = 1u16 << qi;

    if let Some(letter) = fixed[qi] {
        current[qi] = Some(letter);
        *assigned_bits |= bit;
        if !has_contradiction(fp, current, n, qi, *assigned_bits, all_bits, range_masks) {
            search(
                fp,
                fixed,
                solutions,
                current,
                order,
                all_bits,
                assigned_bits,
                range_masks,
                depth + 1,
                max_solutions,
            );
        }
        current[qi] = None;
        *assigned_bits &= !bit;
        return;
    }

    for &letter in &LETTERS {
        current[qi] = Some(letter);
        *assigned_bits |= bit;
        if !has_contradiction(fp, current, n, qi, *assigned_bits, all_bits, range_masks) {
            search(
                fp,
                fixed,
                solutions,
                current,
                order,
                all_bits,
                assigned_bits,
                range_masks,
                depth + 1,
                max_solutions,
            );
            if solutions.len() >= max_solutions {
                current[qi] = None;
                *assigned_bits &= !bit;
                return;
            }
        }
    }
    current[qi] = None;
    *assigned_bits &= !bit;
}

fn has_contradiction(
    fp: &FlatPuzzle,
    answers: &[Option<Answer>; MAX_N],
    n: usize,
    just_assigned: usize,
    assigned: u16,
    all_bits: u16,
    range_masks: &[u16; MAX_N],
) -> bool {
    let all_answered = assigned == all_bits;

    if all_answered {
        for i in 0..n {
            if !check_question_against_solution(fp, i, answers[i].unwrap(), answers) {
                return true;
            }
        }
        return false;
    }

    let affected = &fp.affected_by[just_assigned];
    for k in 0..affected.len as usize {
        let i = affected.data[k] as usize;
        if answers[i].is_none() {
            continue;
        }
        if check_rule(fp, answers, n, i, all_answered, assigned, range_masks) {
            return true;
        }
    }

    let globals = &fp.global_indices;
    for k in 0..globals.len as usize {
        let i = globals.data[k] as usize;
        if answers[i].is_none() || i == just_assigned {
            continue;
        }
        if check_rule(fp, answers, n, i, all_answered, assigned, range_masks) {
            return true;
        }
    }

    false
}

fn check_rule(
    fp: &FlatPuzzle,
    answers: &[Option<Answer>; MAX_N],
    n: usize,
    i: usize,
    all_answered: bool,
    assigned: u16,
    range_masks: &[u16; MAX_N],
) -> bool {
    let t = &fp.question_types[i];
    let answer_i = answers[i].unwrap();

    if (all_answered || can_fully_evaluate_local(t, assigned, range_masks, i))
        && !check_question_against_solution(fp, i, answer_i, answers)
    {
        return true;
    }

    // Forward checking for counting types
    match *t {
        QuestionType::CountAnswer { answer }
        | QuestionType::CountAnswerBefore { answer, .. }
        | QuestionType::CountAnswerAfter { answer, .. } => {
            let opt_val = fp.option_nums[i][answer_i.idx()];
            if opt_val == NAN_VAL {
                return false;
            }

            let (range_start, range_end) = match *t {
                QuestionType::CountAnswer { .. } => (0, n),
                QuestionType::CountAnswerBefore { before_index, .. } => (0, before_index as usize),
                QuestionType::CountAnswerAfter { after_index, .. } => (after_index as usize + 1, n),
                _ => unreachable!(),
            };

            let mut count: i16 = 0;
            let mut remaining: i16 = 0;
            for j in range_start..range_end {
                if answers[j] == Some(answer) {
                    count += 1;
                } else if answers[j].is_none() {
                    remaining += 1;
                }
            }
            if count > opt_val || count + remaining < opt_val {
                return true;
            }
        }
        QuestionType::CountVowel | QuestionType::CountConsonant => {
            let opt_val = fp.option_nums[i][answer_i.idx()];
            if opt_val == NAN_VAL {
                return false;
            }
            let is_vowel = matches!(*t, QuestionType::CountVowel);
            let mut count: i16 = 0;
            let mut remaining: i16 = 0;
            for j in 0..n {
                if answers[j].is_none() {
                    remaining += 1;
                } else if let Some(a) = answers[j]
                    && is_vowel == a.is_vowel()
                {
                    count += 1;
                }
            }
            if count > opt_val || count + remaining < opt_val {
                return true;
            }
        }
        _ => {}
    }

    false
}

fn can_fully_evaluate_local(
    t: &QuestionType,
    assigned: u16,
    range_masks: &[u16; MAX_N],
    qi: usize,
) -> bool {
    match *t {
        QuestionType::AnswerIsSelf => true,
        QuestionType::PrevSame => {
            let mut mask = 0u16;
            for j in 0..qi {
                mask |= 1 << j;
            }
            (assigned & mask) == mask
        }
        QuestionType::AnswerOf { question_index } => (assigned & (1 << question_index)) != 0,
        QuestionType::LetterDist { question_index } => (assigned & (1 << question_index)) != 0,
        QuestionType::NextSame
        | QuestionType::ClosestAfter { .. }
        | QuestionType::ClosestBefore { .. }
        | QuestionType::CountAnswerBefore { .. }
        | QuestionType::CountAnswerAfter { .. } => {
            let mask = range_masks[qi];
            (assigned & mask) == mask
        }
        _ => false,
    }
}
