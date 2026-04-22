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
        if let Some(r) = try_constructive(profile, rng) {
            return Some(r);
        }
    }
    None
}

const ENTRY_TYPES: &[RuleKind] = &[
    RuleKind::CountAnswer,
    RuleKind::CountAnswerBefore,
    RuleKind::CountAnswerAfter,
    RuleKind::CountVowel,
    RuleKind::CountConsonant,
];

const POSITIONAL_TYPES: &[RuleKind] = &[
    RuleKind::FirstWith,
    RuleKind::LastWith,
    RuleKind::ClosestAfter,
    RuleKind::ClosestBefore,
];

const VARIETY_TYPES: &[RuleKind] = &[
    RuleKind::LetterDist,
    RuleKind::ConsecIdent,
    RuleKind::MostCommonCount,
    RuleKind::PrevSame,
    RuleKind::NextSame,
    RuleKind::OnlySame,
    RuleKind::SameAs,
    RuleKind::OnlyOdd,
    RuleKind::LeastCommon,
    RuleKind::MostCommon,
    RuleKind::Unique,
    RuleKind::EqualCount,
    RuleKind::AnswerIsSelf,
    RuleKind::TrueStmt,
];

fn filter_allowed(types: &[RuleKind], profile: &DifficultyProfile) -> Vec<RuleKind> {
    types.iter().copied().filter(|t| profile.allowed_types.contains(t)).collect()
}

fn try_constructive(profile: &DifficultyProfile, rng: &mut Rng) -> Option<GenerateResult> {
    let n = profile.question_count;

    let solution: [Answer; MAX_N] = std::array::from_fn(|i| {
        if i < n { rng.pick(&LETTERS) } else { Answer::A }
    });

    let mut slots: [u8; MAX_N] = std::array::from_fn(|i| i as u8);
    rng.shuffle(&mut slots[..n]);

    let mut rules = [Rule::AnswerIsSelf; MAX_N];
    let mut assigned = 0u16; // bitmask
    let mut assigned_count = 0usize;
    let mut used_rules: [Rule; MAX_N] = [Rule::AnswerIsSelf; MAX_N];
    let mut used_count = 0usize;

    let av_entry = filter_allowed(ENTRY_TYPES, profile);
    let av_positional = filter_allowed(POSITIONAL_TYPES, profile);
    let av_variety = filter_allowed(VARIETY_TYPES, profile);

    // Helper: check if a rule would duplicate an existing question text
    // (same Rule value = same text, since text is deterministic from Rule)
    fn is_dup(rule: &Rule, used: &[Rule; MAX_N], count: usize) -> bool {
        for i in 0..count {
            if used[i] == *rule { return true; }
        }
        false
    }

    macro_rules! place {
        ($kind:expr) => {{
            let qi = slots[assigned_count] as usize;
            let mut ok = false;
            for _ in 0..10 {
                if let Some(rule) = make_rule($kind, qi, n, &solution, assigned, rng) {
                    if !is_dup(&rule, &used_rules, used_count) && check_structural(&rule, qi, &solution, n) {
                        rules[qi] = rule;
                        used_rules[used_count] = rule;
                        used_count += 1;
                        assigned |= 1 << qi;
                        assigned_count += 1;
                        ok = true;
                        break;
                    }
                }
            }
            ok
        }};
    }

    // Phase 1: Counting entry point
    if av_entry.is_empty() { return None; }
    if !place!(rng.pick(&av_entry)) { return None; }

    // Phase 2: 2 answer_of_question (cascade backbone)
    let chain_count = 2.min(n - assigned_count);
    for _ in 0..chain_count {
        if !place!(RuleKind::AnswerOf) { return None; }
    }

    // Phase 3: 2-3 positional rules
    let pos_count = if av_positional.is_empty() { 0 } else { 2.max(n / 5).min(n - assigned_count) };
    for _ in 0..pos_count {
        if !av_positional.is_empty() && assigned_count < n {
            place!(rng.pick(&av_positional));
        }
    }

    // Phase 4: Guaranteed exotic types
    let exotic: &[RuleKind] = &[RuleKind::LetterDist, RuleKind::TrueStmt, RuleKind::ConsecIdent];
    for &kind in exotic {
        if assigned_count >= n { break; }
        if profile.allowed_types.contains(&kind) {
            place!(kind);
        }
    }

    // Phase 5: Fill remaining with variety (no answer_of_question)
    let mut fill_pool: Vec<RuleKind> = Vec::new();
    fill_pool.extend_from_slice(&av_entry);
    fill_pool.extend_from_slice(&av_positional);
    fill_pool.extend_from_slice(&av_variety);
    fill_pool.retain(|k| *k != RuleKind::AnswerOf);

    while assigned_count < n {
        let mut placed = false;
        for _ in 0..20 {
            if !fill_pool.is_empty() && place!(rng.pick(&fill_pool)) {
                placed = true;
                break;
            }
        }
        if !placed {
            if !place!(RuleKind::AnswerOf) && !place!(RuleKind::AnswerIsSelf) {
                return None;
            }
        }
    }

    // Build FlatPuzzle
    let fp = build_flat_puzzle(&rules, &solution, n, rng)?;

    // Validate solution
    let opt_solution = to_optional(&solution, n);
    for i in 0..n {
        if !evaluate(&fp, i, solution[i], &opt_solution) {
            return None;
        }
    }

    // Check uniqueness
    let solutions = solve(&fp, None, 2);
    if solutions.len() != 1 {
        return None;
    }

    // Check solvability
    if !check_solvable(&fp) {
        return None;
    }

    Some(GenerateResult { rules, solution, fp, n })
}

fn make_rule(
    kind: RuleKind,
    qi: usize,
    n: usize,
    solution: &[Answer; MAX_N],
    assigned: u16,
    rng: &mut Rng,
) -> Option<Rule> {
    match kind {
        RuleKind::CountAnswer => Some(Rule::CountAnswer { answer: rng.pick(&LETTERS) }),
        RuleKind::CountAnswerBefore => {
            if n < 4 { return None; }
            Some(Rule::CountAnswerBefore { answer: rng.pick(&LETTERS), before_index: rng.int(2, n as i32 - 1) as u8 })
        }
        RuleKind::CountAnswerAfter => {
            if n < 4 { return None; }
            Some(Rule::CountAnswerAfter { answer: rng.pick(&LETTERS), after_index: rng.int(0, (n as i32 - 3).max(0)) as u8 })
        }
        RuleKind::CountVowel => Some(Rule::CountVowel),
        RuleKind::CountConsonant => Some(Rule::CountConsonant),
        RuleKind::MostCommonCount => Some(Rule::MostCommonCount),
        RuleKind::AnswerOf => {
            // Point at already-assigned questions
            let mut pool = [0u8; MAX_N];
            let mut plen = 0;
            for j in 0..n {
                if j != qi && (assigned & (1 << j)) != 0 {
                    pool[plen] = j as u8;
                    plen += 1;
                }
            }
            if plen == 0 { return None; }
            Some(Rule::AnswerOf { question_index: rng.pick(&pool[..plen]) })
        }
        RuleKind::LetterDist => {
            let mut pool = [0u8; MAX_N];
            let mut plen = 0;
            // Prefer already-assigned, fallback to any
            for j in 0..n {
                if j != qi && (assigned & (1 << j)) != 0 {
                    pool[plen] = j as u8;
                    plen += 1;
                }
            }
            if plen == 0 {
                for j in 0..n {
                    if j != qi { pool[plen] = j as u8; plen += 1; }
                }
            }
            Some(Rule::LetterDist { other_question_index: rng.pick(&pool[..plen]) })
        }
        RuleKind::ClosestAfter => Some(Rule::ClosestAfter {
            after_index: rng.int(0, (n as i32 - 5).max(0)) as u8,
            answer: rng.pick(&LETTERS),
        }),
        RuleKind::ClosestBefore => {
            if n < 5 { return None; }
            Some(Rule::ClosestBefore { before_index: rng.int(4, n as i32 - 1) as u8, answer: rng.pick(&LETTERS) })
        }
        RuleKind::FirstWith => Some(Rule::FirstWith { answer: rng.pick(&LETTERS) }),
        RuleKind::LastWith => Some(Rule::LastWith { answer: rng.pick(&LETTERS) }),
        RuleKind::PrevSame => Some(Rule::PrevSame),
        RuleKind::NextSame => Some(Rule::NextSame),
        RuleKind::OnlySame => Some(Rule::OnlySame),
        RuleKind::SameAs => Some(Rule::SameAs),
        RuleKind::ConsecIdent => Some(Rule::ConsecIdent),
        RuleKind::OnlyOdd => Some(Rule::OnlyOdd { answer: rng.pick(&LETTERS) }),
        RuleKind::LeastCommon => Some(Rule::LeastCommon),
        RuleKind::MostCommon => Some(Rule::MostCommon),
        RuleKind::Unique => Some(Rule::Unique),
        RuleKind::EqualCount => Some(Rule::EqualCount { answer: rng.pick(&LETTERS) }),
        RuleKind::AnswerIsSelf => Some(Rule::AnswerIsSelf),
        RuleKind::TrueStmt => Some(Rule::TrueStmt),
    }
}

fn check_structural(rule: &Rule, qi: usize, sol: &[Answer; MAX_N], n: usize) -> bool {
    match *rule {
        Rule::OnlySame => {
            let mut m = 0;
            for i in 0..n { if i != qi && sol[i] == sol[qi] { m += 1; } }
            m == 1
        }
        Rule::ConsecIdent => {
            let mut p = 0;
            for i in 0..n.saturating_sub(1) { if sol[i] == sol[i + 1] { p += 1; } }
            p == 1
        }
        Rule::OnlyOdd { answer } => {
            let mut m = 0;
            for i in 0..n { if (i + 1) % 2 == 1 && sol[i] == answer { m += 1; } }
            m == 1
        }
        Rule::Unique => {
            let mut c = 0;
            for i in 0..n { if sol[i] == sol[qi] { c += 1; } }
            c == 1
        }
        Rule::EqualCount { answer } => {
            let rc = (0..n).filter(|&i| sol[i] == answer).count();
            LETTERS.iter().any(|&l| l != answer && (0..n).filter(|&i| sol[i] == l).count() == rc)
        }
        _ => true,
    }
}

fn to_optional(sol: &[Answer; MAX_N], n: usize) -> [Option<Answer>; MAX_N] {
    let mut arr = [None; MAX_N];
    for i in 0..n { arr[i] = Some(sol[i]); }
    arr
}

fn check_solvable(fp: &FlatPuzzle) -> bool {
    let n = fp.n;
    let mut answers = [None; MAX_N];
    let mut eliminated = [0u8; MAX_N];
    for _ in 0..n * 15 {
        if (0..n).all(|i| answers[i].is_some()) { return true; }
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

// ── Build FlatPuzzle (reuses logic from assemble.rs) ──

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
            for oi in 0..5 { option_answers[qi][oi] = oi as u8; }
            continue;
        }
        if matches!(rule, Rule::TrueStmt) {
            build_claims(qi, solution, n, rng, &mut option_claims[qi], &mut option_nums[qi]);
            continue;
        }

        let correct_val = compute_value(rule, qi, solution, n);

        match *rule {
            Rule::AnswerOf { question_index } => {
                option_answers[qi][correct_oi] = solution[question_index as usize] as u8;
                let correct_answer = solution[question_index as usize];
                let mut pool = [Answer::A; 4];
                let mut plen = 0;
                for &l in &LETTERS { if l != correct_answer { pool[plen] = l; plen += 1; } }
                rng.shuffle(&mut pool[..plen]);
                let mut di = 0;
                for oi in 0..5 {
                    if oi != correct_oi { option_answers[qi][oi] = pool[di] as u8; di += 1; }
                }
            }
            Rule::ConsecIdent => {
                option_nums[qi][correct_oi] = correct_val;
                let d = pair_distractors(correct_val, n, rng);
                let mut di = 0;
                for oi in 0..5 { if oi != correct_oi { option_nums[qi][oi] = d[di]; di += 1; } }
            }
            Rule::LetterDist { .. } => {
                option_nums[qi][correct_oi] = correct_val;
                let mut pool = [0i16; 4];
                let mut plen = 0;
                for v in 0..5i16 { if v != correct_val { pool[plen] = v; plen += 1; } }
                rng.shuffle(&mut pool[..plen]);
                let mut di = 0;
                for oi in 0..5 { if oi != correct_oi { option_nums[qi][oi] = pool[di]; di += 1; } }
            }
            _ if is_counting_type(rule) => {
                option_nums[qi][correct_oi] = correct_val;
                let d = count_distractors(correct_val as i32, count_max(rule, n) as i32, rng);
                let mut di = 0;
                for oi in 0..5 { if oi != correct_oi { option_nums[qi][oi] = d[di]; di += 1; } }
            }
            _ => {
                option_nums[qi][correct_oi] = correct_val;
                let d = positional_distractors(correct_val, n, rule, rng);
                let mut di = 0;
                for oi in 0..5 { if oi != correct_oi { option_nums[qi][oi] = d[di]; di += 1; } }
            }
        }
    }

    let (affected_by, global_indices) = FlatPuzzle::build_deps(rules, n);
    Some(FlatPuzzle { rules: *rules, option_nums, option_answers, option_claims, affected_by, global_indices, n })
}

fn compute_value(rule: &Rule, qi: usize, sol: &[Answer; MAX_N], n: usize) -> i16 {
    match *rule {
        Rule::AnswerOf { question_index } => sol[question_index as usize] as i16,
        Rule::CountAnswer { answer } => (0..n).filter(|&i| sol[i] == answer).count() as i16,
        Rule::CountAnswerBefore { answer, before_index } => (0..before_index as usize).filter(|&i| sol[i] == answer).count() as i16,
        Rule::CountAnswerAfter { answer, after_index } => ((after_index as usize + 1)..n).filter(|&i| sol[i] == answer).count() as i16,
        Rule::CountVowel => (0..n).filter(|&i| sol[i].is_vowel()).count() as i16,
        Rule::CountConsonant => (0..n).filter(|&i| !sol[i].is_vowel()).count() as i16,
        Rule::MostCommonCount => { let c = letter_counts(sol, n); *c.iter().max().unwrap() as i16 }
        Rule::ClosestAfter { after_index, answer } => {
            for i in (after_index as usize + 1)..n { if sol[i] == answer { return (i as i16) + 1; } }
            NONE_VAL
        }
        Rule::ClosestBefore { before_index, answer } => {
            for i in (0..before_index as usize).rev() { if sol[i] == answer { return (i as i16) + 1; } }
            NONE_VAL
        }
        Rule::FirstWith { answer } => {
            for i in 0..n { if sol[i] == answer { return (i as i16) + 1; } }
            NONE_VAL
        }
        Rule::LastWith { answer } => {
            for i in (0..n).rev() { if sol[i] == answer { return (i as i16) + 1; } }
            NONE_VAL
        }
        Rule::PrevSame => {
            for i in (0..qi).rev() { if sol[i] == sol[qi] { return (i as i16) + 1; } }
            NONE_VAL
        }
        Rule::NextSame => {
            for i in (qi + 1)..n { if sol[i] == sol[qi] { return (i as i16) + 1; } }
            NONE_VAL
        }
        Rule::OnlySame | Rule::SameAs => {
            for i in 0..n { if i != qi && sol[i] == sol[qi] { return (i as i16) + 1; } }
            NONE_VAL
        }
        Rule::OnlyOdd { answer } => {
            for i in 0..n { if (i + 1) % 2 == 1 && sol[i] == answer { return (i as i16) + 1; } }
            NONE_VAL
        }
        Rule::ConsecIdent => {
            for i in 0..n.saturating_sub(1) { if sol[i] == sol[i + 1] { return i as i16; } }
            NONE_VAL
        }
        Rule::LetterDist { other_question_index } => {
            (sol[qi].idx() as i16 - sol[other_question_index as usize].idx() as i16).abs()
        }
        _ => NAN_VAL,
    }
}

fn is_counting_type(rule: &Rule) -> bool {
    matches!(rule, Rule::CountAnswer { .. } | Rule::CountAnswerBefore { .. } | Rule::CountAnswerAfter { .. }
        | Rule::CountVowel | Rule::CountConsonant | Rule::MostCommonCount)
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
    for i in 0..=upper { if i != correct { pool[plen] = i as i16; plen += 1; } }
    rng.shuffle(&mut pool[..plen]);
    let mut r = [0i16; 4];
    for i in 0..4.min(plen) { r[i] = pool[i]; }
    r
}

fn positional_distractors(correct: i16, n: usize, rule: &Rule, rng: &mut Rng) -> [i16; 4] {
    let mut min_pos: i16 = 1;
    let mut max_pos = n as i16;
    match *rule {
        Rule::ClosestAfter { after_index, .. } => min_pos = after_index as i16 + 2,
        Rule::ClosestBefore { before_index, .. } => max_pos = before_index as i16,
        _ => {}
    }
    let mut pool = [0i16; 20];
    let mut plen = 0;
    for i in min_pos..=max_pos { if i != correct { pool[plen] = i; plen += 1; } }
    if correct != NONE_VAL { pool[plen] = NONE_VAL; plen += 1; }
    rng.shuffle(&mut pool[..plen]);
    let mut r = [0i16; 4];
    for i in 0..4.min(plen) { r[i] = pool[i]; }
    r
}

fn pair_distractors(correct: i16, n: usize, rng: &mut Rng) -> [i16; 4] {
    let mut pool = [0i16; 16];
    let mut plen = 0;
    for i in 0..n.saturating_sub(1) { let v = i as i16; if v != correct { pool[plen] = v; plen += 1; } }
    if correct != NONE_VAL { pool[plen] = NONE_VAL; plen += 1; }
    rng.shuffle(&mut pool[..plen]);
    let mut r = [0i16; 4];
    for i in 0..4.min(plen) { r[i] = pool[i]; }
    r
}

fn letter_counts(sol: &[Answer; MAX_N], n: usize) -> [i32; 5] {
    let mut c = [0i32; 5];
    for i in 0..n { c[sol[i].idx()] += 1; }
    c
}

// ── Claims for only_true_statement ──

fn build_claims(qi: usize, solution: &[Answer; MAX_N], n: usize, rng: &mut Rng, claims: &mut [Claim; 5], nums: &mut [i16; 5]) {
    let target_oi = solution[qi].idx();
    let opt_sol = { let mut a = [None; MAX_N]; for i in 0..n { a[i] = Some(solution[i]); } a };

    let true_claim = make_true_claim(solution, n, rng);
    claims[target_oi] = true_claim;
    nums[target_oi] = NAN_VAL;

    for oi in 0..5 {
        if oi == target_oi { continue; }
        let mut found = false;
        for _ in 0..30 {
            let fc = make_false_claim(solution, n, rng, &opt_sol);
            if fc != claims[target_oi] && (0..oi).all(|j| j == target_oi || claims[j] != fc) {
                claims[oi] = fc;
                found = true;
                break;
            }
        }
        if !found { claims[oi] = make_false_claim(solution, n, rng, &opt_sol); }
        nums[oi] = NAN_VAL;
    }
}

fn make_true_claim(sol: &[Answer; MAX_N], n: usize, rng: &mut Rng) -> Claim {
    match rng.int(0, 4) {
        0 => { let a = rng.pick(&LETTERS); Claim::CountAnswerEquals { answer: a, value: (0..n).filter(|&i| sol[i] == a).count() as u8 } }
        1 => Claim::CountConsonantEquals { value: (0..n).filter(|&i| !sol[i].is_vowel()).count() as u8 },
        2 => Claim::CountVowelEquals { value: (0..n).filter(|&i| sol[i].is_vowel()).count() as u8 },
        3 => { let a = rng.pick(&LETTERS); let ai = rng.int(0, n as i32 - 2) as u8;
            Claim::CountAnswerAfterEquals { answer: a, after_index: ai, value: ((ai as usize + 1)..n).filter(|&i| sol[i] == a).count() as u8 } }
        _ => { let a = rng.pick(&LETTERS); let bi = rng.int(1, n as i32 - 1) as u8;
            Claim::CountAnswerBeforeEquals { answer: a, before_index: bi, value: (0..bi as usize).filter(|&i| sol[i] == a).count() as u8 } }
    }
}

fn make_false_claim(sol: &[Answer; MAX_N], n: usize, rng: &mut Rng, opt_sol: &[Option<Answer>; MAX_N]) -> Claim {
    for _ in 0..30 {
        let base = make_true_claim(sol, n, rng);
        let offset = rng.pick(&[-2i8, -1, 1, 2]);
        let base_value = match base {
            Claim::CountAnswerEquals { value, .. } | Claim::CountConsonantEquals { value }
            | Claim::CountVowelEquals { value } | Claim::CountAnswerAfterEquals { value, .. }
            | Claim::CountAnswerBeforeEquals { value, .. } => value as i8,
            Claim::None => continue,
        };
        let new_val = base_value + offset;
        if new_val < 0 || new_val > n as i8 { continue; }
        let fc = set_claim_value(base, new_val as u8);
        if !evaluate_claim(&fc, opt_sol, n) { return fc; }
    }
    Claim::CountAnswerEquals { answer: Answer::A, value: n as u8 + 1 }
}

fn set_claim_value(claim: Claim, value: u8) -> Claim {
    match claim {
        Claim::CountAnswerEquals { answer, .. } => Claim::CountAnswerEquals { answer, value },
        Claim::CountConsonantEquals { .. } => Claim::CountConsonantEquals { value },
        Claim::CountVowelEquals { .. } => Claim::CountVowelEquals { value },
        Claim::CountAnswerAfterEquals { answer, after_index, .. } => Claim::CountAnswerAfterEquals { answer, after_index, value },
        Claim::CountAnswerBeforeEquals { answer, before_index, .. } => Claim::CountAnswerBeforeEquals { answer, before_index, value },
        Claim::None => Claim::None,
    }
}
