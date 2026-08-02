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
/// completion of the open questions can satisfy the constraint, so pruning on it never
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fill::fill_options;
    use crate::rng::Rng;
    use crate::test_util::{fast_tests, form_invalid, slow_test_duration};

    /// Every full assignment that `check_answers` accepts, by exhaustive
    /// enumeration of `option_count^n` — the pruning-free oracle for `solve`.
    fn exhaustive(fp: &FlatPuzzle) -> Vec<Vec<u8>> {
        let n = fp.n;
        let oc = fp.option_count as u64;
        let mut out = Vec::new();
        let mut answers = [None::<Answer>; MAX_N];
        for code in 0..oc.pow(n as u32) {
            let mut c = code;
            for i in 0..n {
                answers[i] = Some(Answer::from((c % oc) as u8));
                c /= oc;
            }
            if check_answers(fp, &answers) {
                out.push((0..n).map(|i| answers[i].unwrap() as u8).collect());
            }
        }
        out
    }

    fn random_question_type(
        rng: &mut Rng,
        qi: usize,
        n: usize,
        allow_true_stmt: bool,
    ) -> QuestionType {
        match rng.int(0, 25) {
            0 => QuestionType::CountAnswer {
                answer: rng.pick_letter(5),
            },
            1 => QuestionType::CountAnswerBefore {
                answer: rng.pick_letter(5),
                before_index: rng.int(2, n as i32 - 1) as u8,
            },
            2 => QuestionType::CountAnswerAfter {
                answer: rng.pick_letter(5),
                after_index: rng.int(0, n as i32 - 3) as u8,
            },
            3 => QuestionType::CountVowel,
            4 => QuestionType::CountConsonant,
            5 => QuestionType::MostCommonCount,
            6 => QuestionType::ClosestAfter {
                after_index: rng.int(0, n as i32 - 3) as u8,
                answer: rng.pick_letter(5),
            },
            7 => QuestionType::ClosestBefore {
                before_index: rng.int(2, n as i32 - 1) as u8,
                answer: rng.pick_letter(5),
            },
            8 => QuestionType::FirstWith {
                answer: rng.pick_letter(5),
            },
            9 => QuestionType::LastWith {
                answer: rng.pick_letter(5),
            },
            10 if qi >= 2 => QuestionType::PrevSame,
            11 if qi + 2 < n => QuestionType::NextSame,
            12 => QuestionType::OnlySame,
            13 => QuestionType::SameAs,
            14 => QuestionType::OnlyOdd {
                answer: rng.pick_letter(5),
            },
            15 => QuestionType::OnlyEven {
                answer: rng.pick_letter(5),
            },
            16 => QuestionType::ConsecIdent,
            17 => {
                let q = rng.int(0, n as i32 - 1) as u8;
                if q as usize == qi {
                    QuestionType::AnswerIsSelf
                } else {
                    QuestionType::AnswerOf { question_index: q }
                }
            }
            18 => QuestionType::LeastCommon,
            19 => QuestionType::MostCommon,
            20 => QuestionType::NoOtherHasAnswer,
            21 => QuestionType::EqualCount {
                answer: rng.pick_letter(5),
            },
            22 => QuestionType::AnswerIsSelf,
            23 => {
                let q = rng.int(0, n as i32 - 1) as u8;
                if q as usize == qi {
                    QuestionType::AnswerIsSelf
                } else {
                    QuestionType::LetterDist { question_index: q }
                }
            }
            24 => {
                let q = rng.int(0, n as i32 - 1) as u8;
                if q as usize == qi {
                    QuestionType::AnswerIsSelf
                } else {
                    QuestionType::SameAsWhich { question_index: q }
                }
            }
            25 if allow_true_stmt => QuestionType::TrueStmt,
            _ => QuestionType::AnswerIsSelf,
        }
    }

    /// `has_contradiction` prunes partial branches on `check_answer == Invalid`.
    /// That is only sound if no pruned branch had a valid completion, so the
    /// solution set must equal the pruning-free exhaustive enumeration.
    #[test]
    fn solve_matches_exhaustive_enumeration() {
        let deadline = std::time::Instant::now() + slow_test_duration();
        let mut puzzles_tested = 0;
        let mut failures = 0;
        let mut vacuous = 0;
        let mut attempted = 0;
        let mut skipped_precondition = 0;
        let mut skipped_form = 0;
        let mut kind_tally = [0u32; QUESTION_KIND_COUNT];
        // A panic that isn't a precondition rejection is a real bug. Carry it out of
        // the loop rather than asserting in place, so it reports with the hook back.
        let mut unexpected_panic: Option<String> = None;

        // Most seeds hand `fill_options` a solution that one of its type preconditions
        // rejects — this builder skips `construct::random_type_params`' gating, and
        // those asserts are how fill reports the gap. Silence the hook for the loop;
        // it's restored before the assertions, which would otherwise print no message.
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));

        for seed in 0u32.. {
            if seed % 50 == 0 && std::time::Instant::now() > deadline {
                break;
            }
            let mut rng = Rng::new(seed.wrapping_mul(2654435761).wrapping_add(17));
            let n = rng.int(4, 8) as usize;
            let oc = rng.int(3, 5) as usize;

            let solution: [Answer; MAX_N] = std::array::from_fn(|i| {
                if i < n {
                    rng.pick_letter(oc)
                } else {
                    Answer::A
                }
            });

            let mut question_types = [QuestionType::AnswerIsSelf; MAX_N];
            let mut true_stmt_used = false;
            for qi in 0..n {
                question_types[qi] = random_question_type(&mut rng, qi, n, !true_stmt_used);
                if matches!(question_types[qi], QuestionType::TrueStmt) {
                    true_stmt_used = true;
                }
            }

            attempted += 1;
            let fp = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                fill_options(
                    &question_types,
                    &solution,
                    n,
                    oc,
                    &mut Rng::new(seed),
                    false,
                )
            }));
            let fp = match fp {
                Ok(fp) => fp,
                Err(payload) => {
                    let msg = payload
                        .downcast_ref::<String>()
                        .map(String::as_str)
                        .or_else(|| payload.downcast_ref::<&str>().copied())
                        .unwrap_or("<non-string panic payload>");
                    if !msg.contains("missing upstream guard") {
                        unexpected_panic = Some(format!("seed={seed}: {msg}"));
                        break;
                    }
                    skipped_precondition += 1;
                    continue;
                }
            };
            // Ungated random types can leave an UNUSED slot inside `option_count`,
            // which is a fatal form error — `check_answer` asserts on those.
            if form_invalid(&fp) {
                skipped_form += 1;
                continue;
            }
            for qi in 0..n {
                kind_tally[fp.question_types[qi].kind() as usize] += 1;
            }

            let mut got: Vec<Vec<u8>> = solve(&fp, usize::MAX)
                .iter()
                .map(|s| (0..n).map(|i| s[i] as u8).collect())
                .collect();
            let mut want = exhaustive(&fp);
            got.sort();
            want.sort();

            if got != want {
                failures += 1;
                if failures <= 5 {
                    let missing: Vec<_> = want.iter().filter(|s| !got.contains(s)).collect();
                    let extra: Vec<_> = got.iter().filter(|s| !want.contains(s)).collect();
                    eprintln!("MISMATCH seed={seed} n={n} oc={oc}");
                    eprintln!("  exhaustive={} solve={}", want.len(), got.len());
                    eprintln!("  dropped by pruning: {missing:?}");
                    eprintln!("  spurious:           {extra:?}");
                    for qi in 0..n {
                        eprintln!(
                            "  Q{}: {:?} opts={:?}",
                            qi + 1,
                            fp.question_types[qi],
                            &fp.options[qi]
                        );
                    }
                }
            }
            // `fill_options` puts the construction solution's value at each correct
            // option, so it always grades valid. An empty `want` would mean the
            // comparison above compared two empty sets and proved nothing.
            if want.is_empty() {
                vacuous += 1;
                eprintln!("VACUOUS seed={seed}: no assignment grades valid");
            }
            puzzles_tested += 1;
        }

        std::panic::set_hook(hook);

        if let Some(msg) = &unexpected_panic {
            panic!(
                "fill_options panicked for something other than a precondition rejection — {msg}"
            );
        }
        assert_eq!(
            failures, 0,
            "{failures} puzzle(s) where solve != exhaustive"
        );
        assert_eq!(vacuous, 0, "{vacuous} puzzle(s) with no valid assignment");
        // Two thirds of seeds are discarded, and the rejections concentrate in the
        // kinds with the tightest preconditions (`NoOtherHasAnswer` is rejected ~7x
        // more often than it survives), so a passing run says little unless every
        // kind actually reached the comparison. The floor scales with the run and is
        // deliberately loose — it catches a kind dropping out, not a drift in the mix.
        // Only in the full run: the fast one compares too few puzzles for the rarest
        // kinds to show up reliably.
        if !fast_tests() {
            let floor = puzzles_tested / 100;
            for kind in QuestionTypeKind::all() {
                let count = kind_tally[*kind as usize];
                assert!(
                    count >= floor,
                    "only {count} {kind:?} question(s) among {puzzles_tested} compared \
                     puzzles (floor {floor}) — the fuzz builder no longer covers it"
                );
            }
        }
        eprintln!(
            "solve_matches_exhaustive_enumeration: {puzzles_tested} puzzles compared, \
             {failures} mismatch(es) ({attempted} seeds, {skipped_precondition} \
             precondition rejection(s), {skipped_form} form error(s))"
        );
    }
}
