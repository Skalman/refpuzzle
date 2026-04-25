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
    types
        .iter()
        .copied()
        .filter(|t| profile.allowed_types.contains(t))
        .collect()
}

fn try_constructive(profile: &DifficultyProfile, rng: &mut Rng) -> Option<GenerateResult> {
    let n = profile.question_count;

    let mut solution: [Answer; MAX_N] =
        std::array::from_fn(|i| if i < n { rng.pick(&LETTERS) } else { Answer::A });

    // Bias toward exactly 1 consecutive pair for levels that allow consecutive_identical
    if profile.allowed_types.contains(&RuleKind::ConsecIdent) && rng.int(0, 1) == 0 {
        let mut pair_positions = [0u8; MAX_N];
        let mut pair_count = 0;
        for i in 0..n - 1 {
            if solution[i] == solution[i + 1] {
                pair_positions[pair_count] = i as u8;
                pair_count += 1;
            }
        }
        if pair_count == 0 {
            let pos = rng.int(0, n as i32 - 2) as usize;
            solution[pos + 1] = solution[pos];
        } else if pair_count > 1 {
            // Keep one random pair, break the rest
            let keep = rng.int(0, pair_count as i32 - 1) as usize;
            for k in 0..pair_count {
                if k != keep {
                    let pos = pair_positions[k] as usize + 1;
                    // Pick a letter different from neighbor
                    loop {
                        let new_letter = rng.pick(&LETTERS);
                        if new_letter != solution[pos - 1]
                            && (pos + 1 >= n || new_letter != solution[pos + 1])
                        {
                            solution[pos] = new_letter;
                            break;
                        }
                    }
                }
            }
        }
    }

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

    fn is_dup(rule: &Rule, used: &[Rule; MAX_N], count: usize) -> bool {
        for i in 0..count {
            if used[i] == *rule {
                return true;
            }
        }
        false
    }

    macro_rules! place {
        ($kind:expr) => {{
            let kind_val = $kind;
            let qi = slots[assigned_count] as usize;
            let mut ok = false;
            if solution_compatible(kind_val, qi, &solution, n) {
                for _ in 0..10 {
                    if let Some(rule) = make_rule(kind_val, qi, n, &solution, assigned, rng) {
                        if !is_dup(&rule, &used_rules, used_count)
                            && check_structural(&rule, qi, &solution, n)
                        {
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
            }
            ok
        }};
    }

    // Phase 1: Counting entry point
    if av_entry.is_empty() {
        return None;
    }
    if !place!(rng.pick(&av_entry)) {
        return None;
    }

    // Phase 2: 2 answer_of_question (cascade backbone)
    let chain_count = 2.min(n - assigned_count);
    for _ in 0..chain_count {
        if !place!(RuleKind::AnswerOf) {
            return None;
        }
    }

    // Phase 3: 2-3 positional rules
    let pos_count = if av_positional.is_empty() {
        0
    } else {
        2.max(n / 5).min(n - assigned_count)
    };
    for _ in 0..pos_count {
        if !av_positional.is_empty() && assigned_count < n {
            place!(rng.pick(&av_positional));
        }
    }

    // Phase 4: Guaranteed exotic types
    let exotic: &[RuleKind] = &[
        RuleKind::LetterDist,
        RuleKind::TrueStmt,
        RuleKind::ConsecIdent,
    ];
    for &kind in exotic {
        if assigned_count >= n {
            break;
        }
        if profile.allowed_types.contains(&kind) {
            place!(kind);
        }
    }

    // Phase 5: Fill remaining, reserving slots for structural rules
    let av_structural: Vec<RuleKind> = av_variety
        .iter()
        .copied()
        .filter(|k| is_structural(*k))
        .collect();
    let structural_reserve = if av_structural.is_empty() {
        0
    } else {
        1.min(n - assigned_count)
    };
    let fill_target = n - structural_reserve;

    let mut fill_pool: Vec<RuleKind> = Vec::new();
    fill_pool.extend_from_slice(&av_entry);
    fill_pool.extend_from_slice(&av_positional);
    fill_pool.extend_from_slice(&av_variety);
    fill_pool.retain(|k| *k != RuleKind::AnswerOf);

    while assigned_count < fill_target {
        let mut placed = false;
        for _ in 0..20 {
            if !fill_pool.is_empty() && place!(rng.pick(&fill_pool)) {
                placed = true;
                break;
            }
        }
        if !placed && !place!(RuleKind::AnswerOf) && !place!(RuleKind::AnswerIsSelf) {
            return None;
        }
    }

    // Phase 6: Structural rules — inspect solution, pick matching types
    for _ in 0..structural_reserve {
        if assigned_count >= n {
            break;
        }
        let qi = slots[assigned_count] as usize;
        let mut fitting: Vec<RuleKind> = av_structural
            .iter()
            .copied()
            .filter(|&k| solution_compatible(k, qi, &solution, n))
            .collect();
        rng.shuffle(&mut fitting);
        let mut placed = false;
        for &kind in &fitting {
            if place!(kind) {
                placed = true;
                break;
            }
        }
        if !placed {
            for _ in 0..20 {
                if !fill_pool.is_empty() && place!(rng.pick(&fill_pool)) {
                    placed = true;
                    break;
                }
            }
            if !placed && !place!(RuleKind::AnswerOf) && !place!(RuleKind::AnswerIsSelf) {
                return None;
            }
        }
    }

    let mut fp = build_flat_puzzle(&rules, &solution, n, rng)?;

    if !validate_and_check(&rules, &solution, &mut fp, n, rng) {
        return None;
    }

    Some(GenerateResult {
        rules,
        solution,
        fp,
        n,
    })
}

fn make_rule(
    kind: RuleKind,
    qi: usize,
    n: usize,
    _solution: &[Answer; MAX_N],
    assigned: u16,
    rng: &mut Rng,
) -> Option<Rule> {
    match kind {
        RuleKind::CountAnswer => Some(Rule::CountAnswer {
            answer: rng.pick(&LETTERS),
        }),
        RuleKind::CountAnswerBefore => {
            if n < 6 {
                return None;
            }
            Some(Rule::CountAnswerBefore {
                answer: rng.pick(&LETTERS),
                before_index: rng.int(4, n as i32 - 1) as u8,
            })
        }
        RuleKind::CountAnswerAfter => {
            if n < 6 {
                return None;
            }
            Some(Rule::CountAnswerAfter {
                answer: rng.pick(&LETTERS),
                after_index: rng.int(0, (n as i32 - 5).max(0)) as u8,
            })
        }
        RuleKind::CountVowel => Some(Rule::CountVowel),
        RuleKind::CountConsonant => Some(Rule::CountConsonant),
        RuleKind::MostCommonCount => Some(Rule::MostCommonCount),
        RuleKind::AnswerOf => {
            let mut pool = [0u8; MAX_N];
            let mut plen = 0;
            for j in 0..n {
                if j != qi && (assigned & (1 << j)) != 0 {
                    pool[plen] = j as u8;
                    plen += 1;
                }
            }
            if plen == 0 {
                return None;
            }
            Some(Rule::AnswerOf {
                question_index: rng.pick(&pool[..plen]),
            })
        }
        RuleKind::LetterDist => {
            let mut pool = [0u8; MAX_N];
            let mut plen = 0;
            for j in 0..n {
                if j != qi && (assigned & (1 << j)) != 0 {
                    pool[plen] = j as u8;
                    plen += 1;
                }
            }
            if plen == 0 {
                for j in 0..n {
                    if j != qi {
                        pool[plen] = j as u8;
                        plen += 1;
                    }
                }
            }
            Some(Rule::LetterDist {
                other_question_index: rng.pick(&pool[..plen]),
            })
        }
        RuleKind::ClosestAfter => Some(Rule::ClosestAfter {
            after_index: rng.int(0, (n as i32 - 5).max(0)) as u8,
            answer: rng.pick(&LETTERS),
        }),
        RuleKind::ClosestBefore => {
            if n < 5 {
                return None;
            }
            Some(Rule::ClosestBefore {
                before_index: rng.int(4, n as i32 - 1) as u8,
                answer: rng.pick(&LETTERS),
            })
        }
        RuleKind::FirstWith => Some(Rule::FirstWith {
            answer: rng.pick(&LETTERS),
        }),
        RuleKind::LastWith => Some(Rule::LastWith {
            answer: rng.pick(&LETTERS),
        }),
        RuleKind::PrevSame => {
            if qi < 4 {
                return None;
            }
            Some(Rule::PrevSame)
        }
        RuleKind::NextSame => {
            if qi + 5 > n {
                return None;
            }
            Some(Rule::NextSame)
        }
        RuleKind::OnlySame => Some(Rule::OnlySame),
        RuleKind::SameAs => Some(Rule::SameAs),
        RuleKind::ConsecIdent => Some(Rule::ConsecIdent),
        RuleKind::OnlyOdd => Some(Rule::OnlyOdd {
            answer: rng.pick(&LETTERS),
        }),
        RuleKind::LeastCommon => Some(Rule::LeastCommon),
        RuleKind::MostCommon => Some(Rule::MostCommon),
        RuleKind::Unique => Some(Rule::Unique),
        RuleKind::EqualCount => Some(Rule::EqualCount {
            answer: rng.pick(&LETTERS),
        }),
        RuleKind::AnswerIsSelf => Some(Rule::AnswerIsSelf),
        RuleKind::TrueStmt => Some(Rule::TrueStmt),
    }
}

fn is_structural(kind: RuleKind) -> bool {
    matches!(
        kind,
        RuleKind::ConsecIdent
            | RuleKind::Unique
            | RuleKind::OnlySame
            | RuleKind::OnlyOdd
            | RuleKind::EqualCount
    )
}

fn solution_has_structural(kind: RuleKind, qi: usize, sol: &[Answer; MAX_N], n: usize) -> bool {
    match kind {
        RuleKind::ConsecIdent => {
            let mut pairs = 0;
            for i in 0..n.saturating_sub(1) {
                if sol[i] == sol[i + 1] {
                    pairs += 1;
                }
            }
            pairs == 1
        }
        RuleKind::Unique => count_letter(sol, sol[qi], n) == 1,
        RuleKind::OnlySame => {
            let mut m = 0;
            for i in 0..n {
                if i != qi && sol[i] == sol[qi] {
                    m += 1;
                }
            }
            m == 1
        }
        RuleKind::OnlyOdd => LETTERS.iter().any(|&letter| {
            let mut m = 0;
            for i in 0..n {
                if (i + 1) % 2 == 1 && sol[i] == letter {
                    m += 1;
                }
            }
            m == 1
        }),
        RuleKind::EqualCount => {
            let counts = letter_counts(sol, n);
            for a in 0..5 {
                for b in (a + 1)..5 {
                    if counts[a] == counts[b] {
                        return true;
                    }
                }
            }
            false
        }
        _ => false,
    }
}

fn solution_compatible(kind: RuleKind, qi: usize, sol: &[Answer; MAX_N], n: usize) -> bool {
    match kind {
        RuleKind::LeastCommon => {
            let counts = letter_counts(sol, n);
            let min = *counts.iter().min().unwrap_or(&0);
            counts[sol[qi].idx()] == min && counts.iter().filter(|&&c| c == min).count() == 1
        }
        RuleKind::MostCommon => {
            let counts = letter_counts(sol, n);
            let max = *counts.iter().max().unwrap_or(&0);
            counts[sol[qi].idx()] == max && counts.iter().filter(|&&c| c == max).count() == 1
        }
        RuleKind::SameAs => {
            for i in 0..n {
                if i != qi && sol[i] == sol[qi] {
                    return true;
                }
            }
            false
        }
        RuleKind::EqualCount => {
            let counts = letter_counts(sol, n);
            let qi_count = counts[sol[qi].idx()];
            LETTERS
                .iter()
                .any(|&l| l != sol[qi] && counts[l.idx()] == qi_count)
        }
        _ if is_structural(kind) => solution_has_structural(kind, qi, sol, n),
        _ => true,
    }
}
