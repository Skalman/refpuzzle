use crate::check_answer::{Validity, check_answer, check_answers};
use crate::types::*;
use arrayvec::ArrayVec;

pub fn solve(fp: &FlatPuzzle, max_solutions: usize) -> Vec<[Answer; MAX_N]> {
    let n = fp.n;
    let mut solutions: Vec<[Answer; MAX_N]> = Vec::new();
    let mut current = [None::<Answer>; MAX_N];
    let order = compute_search_order(fp);
    let all_bits: u16 = (1u16 << n) - 1;
    let mut assigned_bits: u16 = 0;

    search(
        fp,
        &mut solutions,
        &mut current,
        &order,
        all_bits,
        &mut assigned_bits,
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
                let a_global = fp.question_types[a].affected_by_any_answer() as u8;
                let b_global = fp.question_types[b].affected_by_any_answer() as u8;
                a_global.cmp(&b_global)
            })
        })
    });

    indices
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
        if ok && !has_contradiction(fp, current, qi, *assigned_bits, all_bits) {
            search(
                fp,
                solutions,
                current,
                order,
                all_bits,
                assigned_bits,
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

/// Prune this branch when any already-answered question — the ones affected by
/// `just_assigned`, plus every global — is unsatisfiable under the current partial
/// assignment. Delegates to `check_answer`, whose contract is `Invalid` iff no
/// completion of the open cells can satisfy the constraint, so pruning on it never
/// discards a branch that still had a valid completion. The full-board leaf
/// `check_answers` in `search` remains the final authority; this only decides which
/// branches are worth descending.
fn has_contradiction(
    fp: &FlatPuzzle,
    answers: &[Option<Answer>; MAX_N],
    just_assigned: usize,
    assigned: u16,
    all_bits: u16,
) -> bool {
    if assigned == all_bits {
        return !check_answers(fp, answers);
    }

    let state = State {
        answers: *answers,
        eliminated: [fp.initial_eliminated_mask(); MAX_N],
    };

    for i in fp.affected_by[just_assigned].iter() {
        if answers[i].is_some() && check_answer(fp, state, i) == Validity::Invalid {
            return true;
        }
    }

    for i in fp.global_indices.iter() {
        // `just_assigned` is always in its own `affected_by` list, so a global
        // `just_assigned` was already checked in the loop above; skip the re-run.
        if i == just_assigned {
            continue;
        }
        if answers[i].is_some() && check_answer(fp, state, i) == Validity::Invalid {
            return true;
        }
    }

    false
}
