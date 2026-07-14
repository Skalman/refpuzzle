use crate::check_answer::{check_answer, check_answers};
use crate::types::*;
use arrayvec::ArrayVec;

pub fn solve(fp: &FlatPuzzle, max_solutions: usize) -> Vec<[Answer; MAX_N]> {
    let n = fp.n;
    let mut solutions: Vec<[Answer; MAX_N]> = Vec::new();
    let mut current = [None::<Answer>; MAX_N];
    let order = compute_search_order(fp);
    let all_bits: u16 = (1u16 << n) - 1;
    let mut assigned_bits: u16 = 0;

    let range_masks = compute_range_masks(fp);

    search(
        fp,
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

/// Branch order for the backtracking `search`, chosen to prune early:
///  - AnswerIsSelf last: every option is self-consistent, so it never contradicts and
///    branching on it early just multiplies the tree (see check_answer's AnswerIsSelf
///    → always Valid).
///  - most-referenced questions first: assigning an AnswerOf target propagates forces
///    to all its referrers, collapsing the most branches per decision.
///  - globals (whole-board rules) after locals, since they can't be evaluated until
///    their inputs are assigned anyway.
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
        let a_self = matches!(fp.question_types[a], QuestionType::AnswerIsSelf) as u8;
        let b_self = matches!(fp.question_types[b], QuestionType::AnswerIsSelf) as u8;
        a_self.cmp(&b_self).then_with(|| {
            ref_count[b].cmp(&ref_count[a]).then_with(|| {
                let a_global = fp.question_types[a].is_solver_global() as u8;
                let b_global = fp.question_types[b].is_solver_global() as u8;
                a_global.cmp(&b_global)
            })
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
            QuestionType::PrevSame => {
                let mut m = 0u16;
                for j in 0..i {
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

/// If question qi is answered, does it force a specific answer at another position?
/// Returns (target_position, forced_letter) if so.
fn get_force(
    fp: &FlatPuzzle,
    current: &[Option<Answer>; MAX_N],
    qi: usize,
) -> Option<(usize, Answer)> {
    let letter = current[qi]?;
    let ai = letter.idx();
    let qt = &fp.question_types[qi];
    match *qt {
        QuestionType::AnswerOf { question_index } => {
            let ov = fp.options[qi][ai];
            if !ov.is_num() {
                return None;
            }
            let claimed = ov.value();
            (claimed < 5).then_some((question_index as usize, Answer::from(claimed)))
        }
        QuestionType::FirstWith { answer }
        | QuestionType::LastWith { answer }
        | QuestionType::ClosestAfter { answer, .. }
        | QuestionType::ClosestBefore { answer, .. }
        | QuestionType::OnlyOdd { answer }
        | QuestionType::OnlyEven { answer } => {
            let ov = fp.options[qi][ai];
            if !ov.is_num() {
                return None;
            }
            let ov = ov.value() as usize;
            (ov < fp.n).then_some((ov, answer))
        }
        QuestionType::SameAs
        | QuestionType::OnlySame
        | QuestionType::PrevSame
        | QuestionType::NextSame => {
            let ov = fp.options[qi][ai];
            if !ov.is_num() {
                return None;
            }
            let ov = ov.value() as usize;
            (ov < fp.n).then_some((ov, letter))
        }
        QuestionType::SameAsWhich { question_index } => {
            let ov = fp.options[qi][ai];
            if !ov.is_num() {
                return None;
            }
            let ov = ov.value() as usize;
            if ov < fp.n {
                current[question_index as usize].map(|ref_ans| (ov, ref_ans))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn propagate_forces(
    fp: &FlatPuzzle,
    current: &mut [Option<Answer>; MAX_N],
    assigned_bits: &mut u16,
    just_assigned: usize,
    forced: &mut ArrayVec<usize, MAX_N>,
) -> bool {
    let mut queue = ArrayVec::<usize, MAX_N>::new();
    queue.push(just_assigned);

    while let Some(qi) = queue.pop() {
        if let Some((target, answer)) = get_force(fp, current, qi) {
            if answer.idx() >= fp.option_count {
                return false;
            }
            if let Some(existing) = current[target] {
                if existing != answer {
                    return false;
                }
            } else {
                current[target] = Some(answer);
                *assigned_bits |= 1 << target;
                forced.push(target);
                queue.push(target);
            }
        }

        // Reverse AnswerOf: if qi was just determined and some unanswered
        // AnswerOf question references qi, we can determine it too — the
        // correct option is whichever one claims qi's actual answer.
        let affected = &fp.affected_by[qi];
        for j in affected.iter() {
            if current[j].is_some() {
                continue;
            }
            if let QuestionType::AnswerOf { question_index } = fp.question_types[j]
                && question_index as usize == qi
            {
                let target_oi = current[qi].unwrap() as u8;
                let mut found: Option<usize> = None;
                for oi in 0..fp.option_count {
                    let ov = fp.options[j][oi];
                    if ov.is_num() && ov.value() == target_oi {
                        if found.is_some() {
                            found = None;
                            break;
                        }
                        found = Some(oi);
                    }
                }
                if let Some(oi) = found {
                    current[j] = Some(Answer::from(oi as u8));
                    *assigned_bits |= 1 << j;
                    forced.push(j);
                    queue.push(j);
                }
            }
        }
    }
    true
}

fn undo_propagation(
    current: &mut [Option<Answer>; MAX_N],
    assigned_bits: &mut u16,
    forced: &ArrayVec<usize, MAX_N>,
) {
    for &qi in forced {
        current[qi] = None;
        *assigned_bits &= !(1 << qi);
    }
}

fn search(
    fp: &FlatPuzzle,
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
        if check_answers(fp, current) {
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

    // Skip if already assigned by propagation
    if current[qi].is_some() {
        search(
            fp,
            solutions,
            current,
            order,
            all_bits,
            assigned_bits,
            range_masks,
            depth + 1,
            max_solutions,
        );
        return;
    }

    for &letter in &LETTERS[..fp.option_count] {
        current[qi] = Some(letter);
        *assigned_bits |= bit;
        let mut forced = ArrayVec::<usize, MAX_N>::new();
        let ok = propagate_forces(fp, current, assigned_bits, qi, &mut forced);
        if ok && !has_contradiction(fp, current, n, qi, *assigned_bits, all_bits, range_masks) {
            search(
                fp,
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
                undo_propagation(current, assigned_bits, &forced);
                current[qi] = None;
                *assigned_bits &= !bit;
                return;
            }
        }
        undo_propagation(current, assigned_bits, &forced);
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
        return !check_answers(fp, answers);
    }

    for i in fp.affected_by[just_assigned].iter() {
        if answers[i].is_none() {
            continue;
        }
        if rule_violated(fp, answers, n, i, all_answered, assigned, range_masks) {
            return true;
        }
    }

    for i in fp.global_indices.iter() {
        // `just_assigned` is always in its own `affected_by` list, so a global
        // `just_assigned` was already checked in the loop above; skip the re-run.
        if i == just_assigned || answers[i].is_none() {
            continue;
        }
        if rule_violated(fp, answers, n, i, all_answered, assigned, range_masks) {
            return true;
        }
    }

    false
}

/// Returns true when question `i`'s rule is *violated* by the current partial
/// assignment — i.e. this branch can be pruned.
fn rule_violated(
    fp: &FlatPuzzle,
    answers: &[Option<Answer>; MAX_N],
    n: usize,
    i: usize,
    all_answered: bool,
    assigned: u16,
    range_masks: &[u16; MAX_N],
) -> bool {
    let qt = &fp.question_types[i];
    let answer_i = answers[i].unwrap();

    if (all_answered || can_fully_evaluate_local(qt, assigned, range_masks, i))
        && !check_answer(
            fp,
            State {
                answers: *answers,
                eliminated: [fp.initial_eliminated_mask(); MAX_N],
            },
            i,
        )
        .is_valid()
    {
        return true;
    }

    // Forward checking for counting types
    match *qt {
        QuestionType::CountAnswer { answer }
        | QuestionType::CountAnswerBefore { answer, .. }
        | QuestionType::CountAnswerAfter { answer, .. } => {
            let ov = fp.options[i][answer_i.idx()];
            if !ov.is_num() {
                return false;
            }
            let ov = ov.value();

            let (range_start, range_end) = match *qt {
                QuestionType::CountAnswer { .. } => (0, n),
                QuestionType::CountAnswerBefore { before_index, .. } => (0, before_index as usize),
                QuestionType::CountAnswerAfter { after_index, .. } => (after_index as usize + 1, n),
                _ => unreachable!(),
            };

            let mut count: u8 = 0;
            let mut remaining: u8 = 0;
            for j in range_start..range_end {
                if answers[j] == Some(answer) {
                    count += 1;
                } else if answers[j].is_none() {
                    remaining += 1;
                }
            }
            if count > ov || count + remaining < ov {
                return true;
            }
        }
        QuestionType::CountVowel | QuestionType::CountConsonant => {
            let ov = fp.options[i][answer_i.idx()];
            if !ov.is_num() {
                return false;
            }
            let ov = ov.value();
            let is_vowel = matches!(*qt, QuestionType::CountVowel);
            let mut count: u8 = 0;
            let mut remaining: u8 = 0;
            for j in 0..n {
                if answers[j].is_none() {
                    remaining += 1;
                } else if let Some(a) = answers[j]
                    && is_vowel == a.is_vowel()
                {
                    count += 1;
                }
            }
            if count > ov || count + remaining < ov {
                return true;
            }
        }
        _ => {}
    }

    false
}

/// Soundness linchpin: true only when `qt`'s verdict at `qi` is already *final* —
/// no future assignment can change it — so `rule_violated` may treat a current
/// Invalid as a real contradiction and prune. Returning true too early prunes
/// valid branches (unsound, dropped solutions); a conservative `false` is always
/// safe, only slower. Add new arms only after confirming the verdict can't flip.
fn can_fully_evaluate_local(
    qt: &QuestionType,
    assigned: u16,
    range_masks: &[u16; MAX_N],
    qi: usize,
) -> bool {
    match *qt {
        QuestionType::AnswerIsSelf => true,
        QuestionType::AnswerOf { question_index } => (assigned & (1 << question_index)) != 0,
        QuestionType::LetterDist { question_index } => (assigned & (1 << question_index)) != 0,
        QuestionType::PrevSame
        | QuestionType::NextSame
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
