use crate::difficulty::DifficultyProfile;
use crate::evaluate::{evaluate, evaluate_claim};
use crate::hints::{apply_action, find_action_fast, find_lookahead_action};
use crate::rng::Rng;
use crate::solver::solve;
use crate::types::*;

pub struct GenerateResult {
    pub rules: [Rule; MAX_N],
    pub solution: [Answer; MAX_N],
    pub fp: FlatPuzzle,
    pub n: usize,
}

pub fn generate(profile: &DifficultyProfile, rng: &mut Rng, max_attempts: usize) -> Option<GenerateResult> {
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

    for i in 0..n {
        for j in (i + 1)..n {
            if rules[i] == rules[j] {
                return None;
            }
        }
    }

    let fp = build_flat_puzzle(&rules, &solution, n, rng)?;

    let opt_solution = to_optional(&solution, n);
    for i in 0..n {
        if !evaluate(&fp, i, solution[i], &opt_solution) {
            return None;
        }
    }

    let solutions = solve(&fp, None, 2);
    if solutions.len() != 1 {
        return None;
    }

    if !check_solvable(&fp) {
        return None;
    }

    Some(GenerateResult {
        rules,
        solution,
        fp,
        n,
    })
}

fn to_optional(sol: &[Answer; MAX_N], n: usize) -> [Option<Answer>; MAX_N] {
    let mut arr = [None; MAX_N];
    for i in 0..n {
        arr[i] = Some(sol[i]);
    }
    arr
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

fn check_structural(rule: &Rule, qi: usize, sol: &[Answer; MAX_N], n: usize) -> bool {
    match *rule {
        Rule::OnlySame => {
            let mut matches = 0;
            for i in 0..n {
                if i != qi && sol[i] == sol[qi] {
                    matches += 1;
                }
            }
            matches == 1
        }
        Rule::ConsecIdent => {
            let mut pairs = 0;
            for i in 0..n.saturating_sub(1) {
                if sol[i] == sol[i + 1] {
                    pairs += 1;
                }
            }
            pairs == 1
        }
        Rule::OnlyOdd { answer } => {
            let mut matches = 0;
            for i in 0..n {
                if (i + 1) % 2 == 1 && sol[i] == answer {
                    matches += 1;
                }
            }
            matches == 1
        }
        Rule::Unique => count_letter(sol, sol[qi], n) == 1,
        Rule::EqualCount { answer } => {
            let ref_count = count_letter(sol, answer, n);
            let mut has_match = false;
            for &l in &LETTERS {
                if l != answer && count_letter(sol, l, n) == ref_count {
                    has_match = true;
                    break;
                }
            }
            has_match
        }
        _ => true,
    }
}

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
                    let c = counts[l.idx()] as i32;
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
                let has_match = LETTERS.iter().any(|&l| l != answer && count_letter(sol, l, n) == ref_count);
                if has_match {
                    continue;
                }
                let mut closest = Answer::A;
                let mut closest_diff = n as i32;
                for &l in &LETTERS {
                    if l == answer {
                        continue;
                    }
                    let diff = (count_letter(sol, l, n) as i32 - ref_count as i32).abs();
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
            for i in 0..5 {
                if counts[i] == max {
                    return Some(LETTERS[i]);
                }
            }
            None
        }
        Rule::LeastCommon => {
            let counts = letter_counts(solution, n);
            let min = counts.iter().copied().min().unwrap_or(0);
            for i in 0..5 {
                if counts[i] == min {
                    return Some(LETTERS[i]);
                }
            }
            None
        }
        Rule::Unique => {
            if count_letter(solution, solution[qi], n) == 1 {
                return Some(solution[qi]);
            }
            for &l in &LETTERS {
                if count_letter(solution, l, n) == 1 {
                    return Some(l);
                }
            }
            None
        }
        Rule::EqualCount { answer } => {
            let ref_count = count_letter(solution, answer, n);
            for &l in &LETTERS {
                if l != answer && count_letter(solution, l, n) == ref_count {
                    return Some(l);
                }
            }
            None
        }
        Rule::AnswerIsSelf => Some(solution[qi]),
        _ => None,
    }
}

// ── Build FlatPuzzle with options ──

fn build_flat_puzzle(
    rules: &[Rule; MAX_N],
    solution: &[Answer; MAX_N],
    n: usize,
    rng: &mut Rng,
) -> Option<FlatPuzzle> {
    let mut option_nums = [[NAN_VAL; 5]; MAX_N];
    let mut option_answers = [[0xFFu8; 5]; MAX_N];
    let mut option_claims = [[Claim::None; 5]; MAX_N];

    for qi in 0..n {
        let rule = &rules[qi];
        let correct_oi = solution[qi].idx();

        if rule.is_constrained() {
            // Options are A-E
            for oi in 0..5 {
                option_answers[qi][oi] = oi as u8;
            }
            continue;
        }

        if matches!(rule, Rule::TrueStmt) {
            build_claims(qi, solution, n, rng, &mut option_claims[qi], &mut option_nums[qi]);
            continue;
        }

        // Compute correct value
        let correct_val = compute_value(rule, qi, solution, n);

        // Place correct value
        match *rule {
            Rule::AnswerOf { question_index } => {
                option_answers[qi][correct_oi] = solution[question_index as usize] as u8;
                // Distractors: other 4 letters
                let correct_answer = solution[question_index as usize];
                let mut pool = [Answer::A; 4];
                let mut plen = 0;
                for &l in &LETTERS {
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
            Rule::ConsecIdent => {
                option_nums[qi][correct_oi] = correct_val;
                // Distractors
                let distractors = pair_distractors(correct_val, n, rng);
                let mut di = 0;
                for oi in 0..5 {
                    if oi != correct_oi {
                        option_nums[qi][oi] = distractors[di];
                        di += 1;
                    }
                }
            }
            Rule::LetterDist { .. } => {
                option_nums[qi][correct_oi] = correct_val;
                let mut pool = [0i16; 4];
                let mut plen = 0;
                for v in 0..5i16 {
                    if v != correct_val {
                        pool[plen] = v;
                        plen += 1;
                    }
                }
                rng.shuffle(&mut pool[..plen]);
                let mut di = 0;
                for oi in 0..5 {
                    if oi != correct_oi {
                        option_nums[qi][oi] = pool[di];
                        di += 1;
                    }
                }
            }
            _ if is_counting_type(rule) => {
                option_nums[qi][correct_oi] = correct_val;
                let distractors = count_distractors(correct_val as i32, count_max(rule, n) as i32, rng);
                let mut di = 0;
                for oi in 0..5 {
                    if oi != correct_oi {
                        option_nums[qi][oi] = distractors[di];
                        di += 1;
                    }
                }
            }
            _ => {
                // Positional rules
                option_nums[qi][correct_oi] = correct_val;
                let distractors = positional_distractors(correct_val, n, rule, rng);
                let mut di = 0;
                for oi in 0..5 {
                    if oi != correct_oi {
                        option_nums[qi][oi] = distractors[di];
                        di += 1;
                    }
                }
            }
        }
    }

    let (affected_by, global_indices) = FlatPuzzle::build_deps(rules, n);

    Some(FlatPuzzle {
        rules: *rules,
        option_nums,
        option_answers,
        option_claims,
        affected_by,
        global_indices,
        n,
    })
}

fn compute_value(rule: &Rule, qi: usize, sol: &[Answer; MAX_N], n: usize) -> i16 {
    match *rule {
        Rule::AnswerOf { question_index } => sol[question_index as usize] as i16,
        Rule::CountAnswer { answer } => count_letter(sol, answer, n) as i16,
        Rule::CountAnswerBefore { answer, before_index } => {
            let mut c = 0i16;
            for i in 0..before_index as usize {
                if sol[i] == answer { c += 1; }
            }
            c
        }
        Rule::CountAnswerAfter { answer, after_index } => {
            let mut c = 0i16;
            for i in (after_index as usize + 1)..n {
                if sol[i] == answer { c += 1; }
            }
            c
        }
        Rule::CountVowel => {
            let mut c = 0i16;
            for i in 0..n {
                if sol[i].is_vowel() { c += 1; }
            }
            c
        }
        Rule::CountConsonant => {
            let mut c = 0i16;
            for i in 0..n {
                if !sol[i].is_vowel() { c += 1; }
            }
            c
        }
        Rule::MostCommonCount => {
            let counts = letter_counts(sol, n);
            *counts.iter().max().unwrap_or(&0) as i16
        }
        Rule::ClosestAfter { after_index, answer } => {
            for i in (after_index as usize + 1)..n {
                if sol[i] == answer { return (i as i16) + 1; }
            }
            NONE_VAL
        }
        Rule::ClosestBefore { before_index, answer } => {
            for i in (0..before_index as usize).rev() {
                if sol[i] == answer { return (i as i16) + 1; }
            }
            NONE_VAL
        }
        Rule::FirstWith { answer } => {
            for i in 0..n {
                if sol[i] == answer { return (i as i16) + 1; }
            }
            NONE_VAL
        }
        Rule::LastWith { answer } => {
            for i in (0..n).rev() {
                if sol[i] == answer { return (i as i16) + 1; }
            }
            NONE_VAL
        }
        Rule::PrevSame => {
            for i in (0..qi).rev() {
                if sol[i] == sol[qi] { return (i as i16) + 1; }
            }
            NONE_VAL
        }
        Rule::NextSame => {
            for i in (qi + 1)..n {
                if sol[i] == sol[qi] { return (i as i16) + 1; }
            }
            NONE_VAL
        }
        Rule::OnlySame => {
            for i in 0..n {
                if i != qi && sol[i] == sol[qi] { return (i as i16) + 1; }
            }
            NONE_VAL
        }
        Rule::SameAs => {
            for i in 0..n {
                if i != qi && sol[i] == sol[qi] { return (i as i16) + 1; }
            }
            NONE_VAL
        }
        Rule::OnlyOdd { answer } => {
            for i in 0..n {
                if (i + 1) % 2 == 1 && sol[i] == answer { return (i as i16) + 1; }
            }
            NONE_VAL
        }
        Rule::ConsecIdent => {
            for i in 0..n.saturating_sub(1) {
                if sol[i] == sol[i + 1] { return i as i16; }
            }
            NONE_VAL
        }
        Rule::LetterDist { other_question_index } => {
            (sol[qi].idx() as i16 - sol[other_question_index as usize].idx() as i16).abs()
        }
        _ => NAN_VAL,
    }
}

// ── Distractor generation ──

fn is_counting_type(rule: &Rule) -> bool {
    matches!(
        rule,
        Rule::CountAnswer { .. }
            | Rule::CountAnswerBefore { .. }
            | Rule::CountAnswerAfter { .. }
            | Rule::CountVowel
            | Rule::CountConsonant
            | Rule::MostCommonCount
    )
}

fn count_max(rule: &Rule, n: usize) -> usize {
    match *rule {
        Rule::CountAnswerBefore { before_index, .. } => before_index as usize,
        Rule::CountAnswerAfter { after_index, .. } => n - after_index as usize - 1,
        _ => n,
    }
}

fn count_distractors(correct: i32, max: i32, rng: &mut Rng) -> [i16; 4] {
    let upper = max.max(4);
    let mut pool = [0i16; 32];
    let mut plen = 0;
    for i in 0..=upper {
        if i != correct {
            pool[plen] = i as i16;
            plen += 1;
        }
    }
    rng.shuffle(&mut pool[..plen]);
    let mut result = [0i16; 4];
    for i in 0..4.min(plen) {
        result[i] = pool[i];
    }
    result
}

fn positional_distractors(correct: i16, n: usize, rule: &Rule, rng: &mut Rng) -> [i16; 4] {
    let mut min_pos: i16 = 1;
    let mut max_pos = n as i16;

    match *rule {
        Rule::ClosestAfter { after_index, .. } => min_pos = after_index as i16 + 2,
        Rule::ClosestBefore { before_index, .. } => max_pos = before_index as i16,
        Rule::CountAnswerAfter { after_index, .. } => min_pos = after_index as i16 + 2,
        Rule::CountAnswerBefore { before_index, .. } => max_pos = before_index as i16,
        _ => {}
    }

    let mut pool = [0i16; 20];
    let mut plen = 0;
    for i in min_pos..=max_pos {
        if i != correct {
            pool[plen] = i;
            plen += 1;
        }
    }
    if correct != NONE_VAL {
        pool[plen] = NONE_VAL;
        plen += 1;
    }
    rng.shuffle(&mut pool[..plen]);
    let mut result = [0i16; 4];
    for i in 0..4.min(plen) {
        result[i] = pool[i];
    }
    result
}

fn pair_distractors(correct: i16, n: usize, rng: &mut Rng) -> [i16; 4] {
    let mut pool = [0i16; 16];
    let mut plen = 0;
    for i in 0..n.saturating_sub(1) {
        let v = i as i16;
        if v != correct {
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
    for i in 0..4.min(plen) {
        result[i] = pool[i];
    }
    result
}

// ── Claims for only_true_statement ──

fn build_claims(
    qi: usize,
    solution: &[Answer; MAX_N],
    n: usize,
    rng: &mut Rng,
    claims: &mut [Claim; 5],
    nums: &mut [i16; 5],
) {
    let target_oi = solution[qi].idx();
    let opt_sol = {
        let mut arr = [None; MAX_N];
        for i in 0..n {
            arr[i] = Some(solution[i]);
        }
        arr
    };

    let true_claim = make_true_claim(solution, n, rng);
    claims[target_oi] = true_claim;
    nums[target_oi] = NAN_VAL; // claims don't have numeric values

    for oi in 0..5 {
        if oi == target_oi {
            continue;
        }
        let mut found = false;
        for _ in 0..30 {
            let fc = make_false_claim(solution, n, rng, &opt_sol);
            if fc != claims[target_oi] && (0..oi).all(|j| j == target_oi || claims[j] != fc) {
                claims[oi] = fc;
                found = true;
                break;
            }
        }
        if !found {
            claims[oi] = make_false_claim(solution, n, rng, &opt_sol);
        }
        nums[oi] = NAN_VAL;
    }
}

fn make_true_claim(sol: &[Answer; MAX_N], n: usize, rng: &mut Rng) -> Claim {
    let claim_type = rng.int(0, 4);
    match claim_type {
        0 => {
            let answer = rng.pick(&LETTERS);
            let value = count_letter(sol, answer, n);
            Claim::CountAnswerEquals {
                answer,
                value: value as u8,
            }
        }
        1 => {
            let mut c = 0u8;
            for i in 0..n {
                if !sol[i].is_vowel() {
                    c += 1;
                }
            }
            Claim::CountConsonantEquals { value: c }
        }
        2 => {
            let mut c = 0u8;
            for i in 0..n {
                if sol[i].is_vowel() {
                    c += 1;
                }
            }
            Claim::CountVowelEquals { value: c }
        }
        3 => {
            let answer = rng.pick(&LETTERS);
            let after_index = rng.int(0, n as i32 - 2) as u8;
            let mut c = 0u8;
            for i in (after_index as usize + 1)..n {
                if sol[i] == answer {
                    c += 1;
                }
            }
            Claim::CountAnswerAfterEquals {
                answer,
                after_index,
                value: c,
            }
        }
        _ => {
            let answer = rng.pick(&LETTERS);
            let before_index = rng.int(1, n as i32 - 1) as u8;
            let mut c = 0u8;
            for i in 0..before_index as usize {
                if sol[i] == answer {
                    c += 1;
                }
            }
            Claim::CountAnswerBeforeEquals {
                answer,
                before_index,
                value: c,
            }
        }
    }
}

fn make_false_claim(
    sol: &[Answer; MAX_N],
    n: usize,
    rng: &mut Rng,
    opt_sol: &[Option<Answer>; MAX_N],
) -> Claim {
    for _ in 0..30 {
        let base = make_true_claim(sol, n, rng);
        let offset = rng.pick(&[-2i8, -1, 1, 2]);
        let base_value = match base {
            Claim::CountAnswerEquals { value, .. } => value as i8,
            Claim::CountConsonantEquals { value } => value as i8,
            Claim::CountVowelEquals { value } => value as i8,
            Claim::CountAnswerAfterEquals { value, .. } => value as i8,
            Claim::CountAnswerBeforeEquals { value, .. } => value as i8,
            Claim::None => continue,
        };
        let new_val = base_value + offset;
        if new_val < 0 || new_val > n as i8 {
            continue;
        }
        let false_claim = set_claim_value(base, new_val as u8);
        if !evaluate_claim(&false_claim, opt_sol, n) {
            return false_claim;
        }
    }
    Claim::CountAnswerEquals {
        answer: Answer::A,
        value: n as u8 + 1,
    }
}

fn set_claim_value(claim: Claim, value: u8) -> Claim {
    match claim {
        Claim::CountAnswerEquals { answer, .. } => Claim::CountAnswerEquals { answer, value },
        Claim::CountConsonantEquals { .. } => Claim::CountConsonantEquals { value },
        Claim::CountVowelEquals { .. } => Claim::CountVowelEquals { value },
        Claim::CountAnswerAfterEquals {
            answer,
            after_index,
            ..
        } => Claim::CountAnswerAfterEquals {
            answer,
            after_index,
            value,
        },
        Claim::CountAnswerBeforeEquals {
            answer,
            before_index,
            ..
        } => Claim::CountAnswerBeforeEquals {
            answer,
            before_index,
            value,
        },
        Claim::None => Claim::None,
    }
}

// ── Solvability check ──

fn check_solvable(fp: &FlatPuzzle) -> bool {
    let n = fp.n;
    let mut answers = [None; MAX_N];
    let mut eliminated = [0u8; MAX_N];

    for _ in 0..n * 15 {
        if (0..n).all(|i| answers[i].is_some()) {
            return true;
        }

        if let Some(action) = find_action_fast(fp, &answers, &eliminated) {
            apply_action(&action, &mut answers, &mut eliminated);
            continue;
        }

        if let Some(action) = find_lookahead_action(fp, &answers, &eliminated) {
            apply_action(&action, &mut answers, &mut eliminated);
            continue;
        }

        return false;
    }
    false
}

// ── Helpers ──

fn letter_counts(sol: &[Answer; MAX_N], n: usize) -> [i32; 5] {
    let mut counts = [0i32; 5];
    for i in 0..n {
        counts[sol[i].idx()] += 1;
    }
    counts
}

fn count_letter(sol: &[Answer; MAX_N], letter: Answer, n: usize) -> i32 {
    let mut c = 0i32;
    for i in 0..n {
        if sol[i] == letter {
            c += 1;
        }
    }
    c
}
