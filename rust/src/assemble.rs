use crate::difficulty::DifficultyProfile;
use crate::gen_common::{
    GenerateResult, build_flat_puzzle, check_structural, count_letter, letter_counts,
    validate_and_check,
};
use crate::rng::Rng;
use crate::types::*;

pub fn generate(
    profile: &DifficultyProfile,
    rng: &mut Rng,
    max_attempts: usize,
) -> Option<GenerateResult> {
    for _ in 0..max_attempts {
        if let Some(result) = try_generate(profile, rng) {
            return Some(result);
        }
    }
    None
}

fn try_generate(profile: &DifficultyProfile, rng: &mut Rng) -> Option<GenerateResult> {
    let n = profile.question_count;

    let kinds = pick_types(profile, n, rng);
    let mut rules = [Rule::AnswerIsSelf; MAX_N];
    assign_params(&kinds, n, &mut rules, rng);

    let mut solution = [Answer::A; MAX_N];
    for i in 0..n {
        solution[i] = rng.pick(&LETTERS);
    }

    shape_solution(&rules, &mut solution, n, rng)?;
    reconcile(&rules, &mut solution, n)?;

    for i in 0..n {
        if !check_structural(&rules[i], i, &solution, n) {
            return None;
        }
    }

    let fp = build_flat_puzzle(&rules, &solution, n, rng)?;

    if !validate_and_check(&rules, &solution, &fp, n) {
        return None;
    }

    Some(GenerateResult {
        rules,
        solution,
        fp,
        n,
    })
}

// ── Step 1: Pick question types ──

fn pick_types(profile: &DifficultyProfile, n: usize, rng: &mut Rng) -> [RuleKind; MAX_N] {
    let allowed = profile.allowed_types;
    let max_per_type = ((n as f64) * 0.4).ceil() as usize;
    let mut types = [RuleKind::AnswerIsSelf; MAX_N];
    let mut type_len = 0;
    let mut counts = [0u8; 24]; // indexed by RuleKind as usize

    fn kind_idx(k: RuleKind) -> usize {
        k as usize
    }

    if allowed.contains(&RuleKind::CountAnswer) {
        types[type_len] = RuleKind::CountAnswer;
        type_len += 1;
        counts[kind_idx(RuleKind::CountAnswer)] = 1;
    }

    // Seed positional rules (lookahead entry points)
    let positional_kinds = [
        RuleKind::FirstWith,
        RuleKind::LastWith,
        RuleKind::ClosestAfter,
        RuleKind::ClosestBefore,
    ];
    let mut avail_pos = [RuleKind::FirstWith; 4];
    let mut avail_pos_len = 0;
    for &k in &positional_kinds {
        if allowed.contains(&k) {
            avail_pos[avail_pos_len] = k;
            avail_pos_len += 1;
        }
    }
    if avail_pos_len > 0 {
        let seed_count = 2.min(n / 4);
        for _ in 0..seed_count {
            let k = rng.pick(&avail_pos[..avail_pos_len]);
            types[type_len] = k;
            type_len += 1;
            counts[kind_idx(k)] += 1;
        }
    }

    if allowed.contains(&RuleKind::AnswerOf) {
        let target = 1.max(n / 4);
        for _ in 0..target {
            types[type_len] = RuleKind::AnswerOf;
            type_len += 1;
        }
        counts[kind_idx(RuleKind::AnswerOf)] = target as u8;
    }

    if allowed.contains(&RuleKind::TrueStmt) {
        types[type_len] = RuleKind::TrueStmt;
        type_len += 1;
        counts[kind_idx(RuleKind::TrueStmt)] = 1;
    }

    while type_len < n {
        let kind = rng.pick(allowed);
        let idx = kind_idx(kind);
        if (counts[idx] as usize) < max_per_type {
            types[type_len] = kind;
            type_len += 1;
            counts[idx] += 1;
        }
    }

    rng.shuffle(&mut types[..n]);
    types
}

// ── Step 2: Assign parameters ──

fn assign_params(kinds: &[RuleKind; MAX_N], n: usize, rules: &mut [Rule; MAX_N], rng: &mut Rng) {
    // Track answer_of_question indices
    let mut aofq_set = 0u16; // bitmask
    for i in 0..n {
        if kinds[i] == RuleKind::AnswerOf {
            aofq_set |= 1 << i;
        }
    }

    for i in 0..n {
        rules[i] = match kinds[i] {
            RuleKind::CountAnswer => Rule::CountAnswer {
                answer: rng.pick(&LETTERS),
            },
            RuleKind::CountAnswerBefore => Rule::CountAnswerBefore {
                answer: rng.pick(&LETTERS),
                before_index: rng.int(2, n as i32 - 1) as u8,
            },
            RuleKind::CountAnswerAfter => Rule::CountAnswerAfter {
                answer: rng.pick(&LETTERS),
                after_index: rng.int(0, (n as i32 - 3).max(0)) as u8,
            },
            RuleKind::CountVowel => Rule::CountVowel,
            RuleKind::CountConsonant => Rule::CountConsonant,
            RuleKind::MostCommonCount => Rule::MostCommonCount,
            RuleKind::LeastCommon => Rule::LeastCommon,
            RuleKind::MostCommon => Rule::MostCommon,
            RuleKind::Unique => Rule::Unique,
            RuleKind::AnswerIsSelf => Rule::AnswerIsSelf,
            RuleKind::PrevSame => Rule::PrevSame,
            RuleKind::NextSame => Rule::NextSame,
            RuleKind::OnlySame => Rule::OnlySame,
            RuleKind::SameAs => Rule::SameAs,
            RuleKind::ConsecIdent => Rule::ConsecIdent,
            RuleKind::TrueStmt => Rule::TrueStmt,
            RuleKind::AnswerOf => {
                let mut pool = [0u8; MAX_N];
                let mut pool_len = 0;
                for j in 0..n {
                    if j != i && (aofq_set & (1 << j)) == 0 {
                        pool[pool_len] = j as u8;
                        pool_len += 1;
                    }
                }
                if pool_len == 0 {
                    for j in 0..n {
                        if j != i {
                            pool[pool_len] = j as u8;
                            pool_len += 1;
                        }
                    }
                }
                Rule::AnswerOf {
                    question_index: rng.pick(&pool[..pool_len]),
                }
            }
            RuleKind::ClosestAfter => Rule::ClosestAfter {
                after_index: rng.int(0, (n as i32 - 5).max(0)) as u8,
                answer: rng.pick(&LETTERS),
            },
            RuleKind::ClosestBefore => Rule::ClosestBefore {
                before_index: rng.int(4, n as i32 - 1) as u8,
                answer: rng.pick(&LETTERS),
            },
            RuleKind::FirstWith => Rule::FirstWith {
                answer: rng.pick(&LETTERS),
            },
            RuleKind::LastWith => Rule::LastWith {
                answer: rng.pick(&LETTERS),
            },
            RuleKind::OnlyOdd => Rule::OnlyOdd {
                answer: rng.pick(&LETTERS),
            },
            RuleKind::EqualCount => Rule::EqualCount {
                answer: rng.pick(&LETTERS),
            },
            RuleKind::LetterDist => {
                let mut pool = [0u8; MAX_N];
                let mut pool_len = 0;
                for j in 0..n {
                    if j != i {
                        pool[pool_len] = j as u8;
                        pool_len += 1;
                    }
                }
                Rule::LetterDist {
                    other_question_index: rng.pick(&pool[..pool_len]),
                }
            }
        };
    }
}

// ── Step 3: Shape solution ──

fn shape_solution(
    rules: &[Rule; MAX_N],
    sol: &mut [Answer; MAX_N],
    n: usize,
    rng: &mut Rng,
) -> Option<()> {
    let mut locked = 0u16; // bitmask
    for i in 0..n {
        if rules[i].is_constrained() {
            locked |= 1 << i;
        }
    }

    let pick_free = |rng: &mut Rng, exclude: Answer| -> Answer {
        let mut pool = [Answer::A; 4];
        let mut len = 0;
        for &l in &LETTERS {
            if l != exclude {
                pool[len] = l;
                len += 1;
            }
        }
        rng.pick(&pool[..len])
    };

    for qi in 0..n {
        match rules[qi] {
            Rule::Unique => {
                if (locked >> qi) & 1 == 1 {
                    continue;
                }
                let mut counts = letter_counts(sol, n);
                counts[sol[qi].idx()] -= 1;
                let mut best = Answer::A;
                let mut best_count = n as i32;
                for &l in &LETTERS {
                    let c = counts[l.idx()];
                    if c < best_count {
                        best_count = c;
                        best = l;
                    }
                }
                for i in 0..n {
                    if i != qi && (locked >> i) & 1 == 0 && sol[i] == best {
                        sol[i] = pick_free(rng, best);
                    }
                }
                sol[qi] = best;
            }
            Rule::OnlySame => {
                if (locked >> qi) & 1 == 1 {
                    continue;
                }
                let my_letter = sol[qi];
                let mut positions = [0u8; MAX_N];
                let mut pos_len = 0;
                for i in 0..n {
                    if i != qi && sol[i] == my_letter {
                        positions[pos_len] = i as u8;
                        pos_len += 1;
                    }
                }
                if pos_len == 1 {
                    continue;
                }
                if pos_len == 0 {
                    let mut candidates = [0u8; MAX_N];
                    let mut cand_len = 0;
                    for i in 0..n {
                        if i != qi && (locked >> i) & 1 == 0 {
                            candidates[cand_len] = i as u8;
                            cand_len += 1;
                        }
                    }
                    if cand_len > 0 {
                        let idx = rng.pick(&candidates[..cand_len]) as usize;
                        sol[idx] = my_letter;
                    }
                } else {
                    let keep = rng.pick(&positions[..pos_len]) as usize;
                    for pi in 0..pos_len {
                        let p = positions[pi] as usize;
                        if p != keep && (locked >> p) & 1 == 0 {
                            sol[p] = pick_free(rng, my_letter);
                        }
                    }
                }
            }
            Rule::ConsecIdent => {
                let mut pairs = [0u8; MAX_N];
                let mut pair_count = 0;
                for i in 0..n.saturating_sub(1) {
                    if sol[i] == sol[i + 1] {
                        pairs[pair_count] = i as u8;
                        pair_count += 1;
                    }
                }
                if pair_count == 1 {
                    continue;
                }
                if pair_count == 0 {
                    let mut candidates = [0u8; MAX_N];
                    let mut cand_len = 0;
                    for i in 0..n.saturating_sub(1) {
                        if (locked >> (i + 1)) & 1 == 0 {
                            candidates[cand_len] = i as u8;
                            cand_len += 1;
                        }
                    }
                    if cand_len > 0 {
                        let pos = rng.pick(&candidates[..cand_len]) as usize;
                        sol[pos + 1] = sol[pos];
                    }
                }
                for _ in 0..5 {
                    let mut extra = [0u8; MAX_N];
                    let mut extra_count = 0;
                    for i in 0..n.saturating_sub(1) {
                        if sol[i] == sol[i + 1] {
                            extra[extra_count] = i as u8;
                            extra_count += 1;
                        }
                    }
                    if extra_count <= 1 {
                        break;
                    }
                    let break_at = extra[extra_count - 1] as usize;
                    if (locked >> (break_at + 1)) & 1 == 0 {
                        sol[break_at + 1] = pick_free(rng, sol[break_at]);
                    } else if (locked >> break_at) & 1 == 0 {
                        sol[break_at] = pick_free(rng, sol[break_at + 1]);
                    }
                }
            }
            Rule::OnlyOdd { answer } => {
                let mut odd_positions = [0u8; MAX_N];
                let mut odd_len = 0;
                for i in 0..n {
                    if (i + 1) % 2 == 1 {
                        odd_positions[odd_len] = i as u8;
                        odd_len += 1;
                    }
                }
                let mut with_answer = [0u8; MAX_N];
                let mut wa_len = 0;
                for oi in 0..odd_len {
                    let p = odd_positions[oi] as usize;
                    if sol[p] == answer {
                        with_answer[wa_len] = p as u8;
                        wa_len += 1;
                    }
                }
                if wa_len == 1 {
                    continue;
                }
                if wa_len == 0 {
                    let mut candidates = [0u8; MAX_N];
                    let mut cand_len = 0;
                    for oi in 0..odd_len {
                        let p = odd_positions[oi] as usize;
                        if (locked >> p) & 1 == 0 {
                            candidates[cand_len] = p as u8;
                            cand_len += 1;
                        }
                    }
                    if cand_len > 0 {
                        let idx = rng.pick(&candidates[..cand_len]) as usize;
                        sol[idx] = answer;
                    }
                } else {
                    let keep = rng.pick(&with_answer[..wa_len]) as usize;
                    for wi in 0..wa_len {
                        let p = with_answer[wi] as usize;
                        if p != keep && (locked >> p) & 1 == 0 {
                            sol[p] = pick_free(rng, answer);
                        }
                    }
                }
            }
            Rule::EqualCount { answer } => {
                let ref_count = count_letter(sol, answer, n);
                let has_match = LETTERS
                    .iter()
                    .any(|&l| l != answer && count_letter(sol, l, n) == ref_count);
                if has_match {
                    continue;
                }
                let mut closest = Answer::A;
                let mut closest_diff = n as i32;
                for &l in &LETTERS {
                    if l == answer {
                        continue;
                    }
                    let diff = (count_letter(sol, l, n) - ref_count).abs();
                    if diff < closest_diff {
                        closest_diff = diff;
                        closest = l;
                    }
                }
                let cur_count = count_letter(sol, closest, n);
                if cur_count < ref_count {
                    for i in 0..n {
                        if count_letter(sol, closest, n) >= ref_count {
                            break;
                        }
                        if (locked >> i) & 1 == 0 && sol[i] != closest && sol[i] != answer {
                            sol[i] = closest;
                        }
                    }
                } else {
                    for i in (0..n).rev() {
                        if count_letter(sol, closest, n) <= ref_count {
                            break;
                        }
                        if (locked >> i) & 1 == 0 && sol[i] == closest {
                            sol[i] = pick_free(rng, closest);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Some(())
}

// ── Step 4: Reconcile ──

fn reconcile(rules: &[Rule; MAX_N], solution: &mut [Answer; MAX_N], n: usize) -> Option<()> {
    for _ in 0..30 {
        let mut changed = false;
        for i in 0..n {
            if !rules[i].is_constrained() {
                continue;
            }
            if let Some(needed) = compute_constrained_answer(&rules[i], i, solution, n) {
                if needed != solution[i] {
                    solution[i] = needed;
                    changed = true;
                }
            }
        }
        if !changed {
            return Some(());
        }
    }
    None
}

fn compute_constrained_answer(
    rule: &Rule,
    qi: usize,
    solution: &[Answer; MAX_N],
    n: usize,
) -> Option<Answer> {
    match *rule {
        Rule::MostCommon => {
            let counts = letter_counts(solution, n);
            let max = counts.iter().copied().max().unwrap_or(0);
            LETTERS.iter().copied().find(|l| counts[l.idx()] == max)
        }
        Rule::LeastCommon => {
            let counts = letter_counts(solution, n);
            let min = counts.iter().copied().min().unwrap_or(0);
            LETTERS.iter().copied().find(|l| counts[l.idx()] == min)
        }
        Rule::Unique => {
            if count_letter(solution, solution[qi], n) == 1 {
                return Some(solution[qi]);
            }
            LETTERS
                .iter()
                .copied()
                .find(|&l| count_letter(solution, l, n) == 1)
        }
        Rule::EqualCount { answer } => {
            let ref_count = count_letter(solution, answer, n);
            LETTERS
                .iter()
                .copied()
                .find(|&l| l != answer && count_letter(solution, l, n) == ref_count)
        }
        Rule::AnswerIsSelf => Some(solution[qi]),
        _ => None,
    }
}
