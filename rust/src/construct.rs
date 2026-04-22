use crate::difficulty::DifficultyProfile;
use crate::gen_common::{GenerateResult, build_flat_puzzle, check_structural, validate_and_check};
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

    let solution: [Answer; MAX_N] =
        std::array::from_fn(|i| if i < n { rng.pick(&LETTERS) } else { Answer::A });

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
            let qi = slots[assigned_count] as usize;
            let mut ok = false;
            for _ in 0..10 {
                if let Some(rule) = make_rule($kind, qi, n, &solution, assigned, rng) {
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
        if !placed && !place!(RuleKind::AnswerOf) && !place!(RuleKind::AnswerIsSelf) {
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
            if n < 4 {
                return None;
            }
            Some(Rule::CountAnswerBefore {
                answer: rng.pick(&LETTERS),
                before_index: rng.int(2, n as i32 - 1) as u8,
            })
        }
        RuleKind::CountAnswerAfter => {
            if n < 4 {
                return None;
            }
            Some(Rule::CountAnswerAfter {
                answer: rng.pick(&LETTERS),
                after_index: rng.int(0, (n as i32 - 3).max(0)) as u8,
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
        RuleKind::PrevSame => Some(Rule::PrevSame),
        RuleKind::NextSame => Some(Rule::NextSame),
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
