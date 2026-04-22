use crate::evaluate::evaluate;
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
        if let Rule::AnswerOf { question_index } = fp.rules[i] {
            ref_count[question_index as usize] += 1;
        }
    }

    let mut indices: [u8; MAX_N] = std::array::from_fn(|i| i as u8);
    indices[..n].sort_by(|&a, &b| {
        let a = a as usize;
        let b = b as usize;
        ref_count[b].cmp(&ref_count[a]).then_with(|| {
            let a_global = fp.rules[a].is_solver_global() as u8;
            let b_global = fp.rules[b].is_solver_global() as u8;
            a_global.cmp(&b_global)
        })
    });

    indices
}

fn compute_range_masks(fp: &FlatPuzzle) -> [u16; MAX_N] {
    let n = fp.n;
    let mut masks = [0u16; MAX_N];
    for i in 0..n {
        masks[i] = match fp.rules[i] {
            Rule::NextSame => {
                let mut m = 0u16;
                for j in (i + 1)..n {
                    m |= 1 << j;
                }
                m
            }
            Rule::ClosestAfter { after_index, .. } | Rule::CountAnswerAfter { after_index, .. } => {
                let mut m = 0u16;
                for j in (after_index as usize + 1)..n {
                    m |= 1 << j;
                }
                m
            }
            Rule::ClosestBefore { before_index, .. }
            | Rule::CountAnswerBefore { before_index, .. } => {
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
            if !evaluate(fp, i, current[i].unwrap(), current) {
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
            if !evaluate(fp, i, answers[i].unwrap(), answers) {
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
    let r = &fp.rules[i];
    let answer_i = answers[i].unwrap();

    if all_answered || can_fully_evaluate_local(r, assigned, range_masks, i) {
        if !evaluate(fp, i, answer_i, answers) {
            return true;
        }
    }

    // Forward checking for counting rules
    match *r {
        Rule::CountAnswer { answer }
        | Rule::CountAnswerBefore { answer, .. }
        | Rule::CountAnswerAfter { answer, .. } => {
            let opt_val = fp.option_nums[i][answer_i.idx()];
            if opt_val == NAN_VAL {
                return false;
            }

            let (range_start, range_end) = match *r {
                Rule::CountAnswer { .. } => (0, n),
                Rule::CountAnswerBefore { before_index, .. } => (0, before_index as usize),
                Rule::CountAnswerAfter { after_index, .. } => (after_index as usize + 1, n),
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
        Rule::CountVowel | Rule::CountConsonant => {
            let opt_val = fp.option_nums[i][answer_i.idx()];
            if opt_val == NAN_VAL {
                return false;
            }
            let is_vowel = matches!(*r, Rule::CountVowel);
            let mut count: i16 = 0;
            let mut remaining: i16 = 0;
            for j in 0..n {
                if answers[j].is_none() {
                    remaining += 1;
                } else if let Some(a) = answers[j] {
                    if is_vowel == a.is_vowel() {
                        count += 1;
                    }
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
    r: &Rule,
    assigned: u16,
    range_masks: &[u16; MAX_N],
    qi: usize,
) -> bool {
    match *r {
        Rule::AnswerIsSelf => true,
        Rule::PrevSame => {
            let mut mask = 0u16;
            for j in 0..qi {
                mask |= 1 << j;
            }
            (assigned & mask) == mask
        }
        Rule::AnswerOf { question_index } => (assigned & (1 << question_index)) != 0,
        Rule::LetterDist {
            other_question_index,
        } => (assigned & (1 << other_question_index)) != 0,
        Rule::NextSame
        | Rule::ClosestAfter { .. }
        | Rule::ClosestBefore { .. }
        | Rule::CountAnswerBefore { .. }
        | Rule::CountAnswerAfter { .. } => {
            let mask = range_masks[qi];
            (assigned & mask) == mask
        }
        _ => false,
    }
}
