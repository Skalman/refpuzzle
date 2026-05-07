use arrayvec::ArrayVec;

use crate::difficulty::DifficultyProfile;
use crate::gen_common::{
    GenerateResult, Stats, build_flat_puzzle, count_letter, letter_counts, solution_satisfies_type,
    validate_and_repair,
};
use crate::rng::Rng;
use crate::types::*;

pub fn generate(
    profile: &DifficultyProfile,
    rng: &mut Rng,
    max_attempts: usize,
    stats: &mut Stats,
) -> Option<GenerateResult> {
    for _ in 0..max_attempts {
        if let Some(r) = try_construct(profile, rng, stats) {
            return Some(r);
        }
    }
    None
}

// ── QuestionType type categories ──

const COUNTING_TYPES: &[QuestionTypeKind] = &[
    QuestionTypeKind::CountAnswer,
    QuestionTypeKind::CountAnswerBefore,
    QuestionTypeKind::CountAnswerAfter,
    QuestionTypeKind::CountVowel,
    QuestionTypeKind::CountConsonant,
];

const POSITIONAL_TYPES: &[QuestionTypeKind] = &[
    QuestionTypeKind::FirstWith,
    QuestionTypeKind::LastWith,
    QuestionTypeKind::ClosestAfter,
    QuestionTypeKind::ClosestBefore,
];

const FILL_TYPES: &[QuestionTypeKind] = &[
    QuestionTypeKind::LetterDist,
    QuestionTypeKind::ConsecIdent,
    QuestionTypeKind::MostCommonCount,
    QuestionTypeKind::PrevSame,
    QuestionTypeKind::NextSame,
    QuestionTypeKind::OnlySame,
    QuestionTypeKind::SameAs,
    QuestionTypeKind::OnlyOdd,
    QuestionTypeKind::OnlyEven,
    QuestionTypeKind::LeastCommon,
    QuestionTypeKind::MostCommon,
    QuestionTypeKind::Unique,
    QuestionTypeKind::EqualCount,
    QuestionTypeKind::AnswerIsSelf,
    QuestionTypeKind::TrueStmt,
];

// ── Placement state ──

struct PlacementState {
    question_types: [QuestionType; MAX_N],
    slots: [u8; MAX_N],
    assigned: u16,
    assigned_count: usize,
    used_types: [QuestionType; MAX_N],
    used_count: usize,
    kind_counts: [u8; 32],
    group_counts: [u8; 8],
    group_caps: [u8; 8],
    caps: [u8; 32],
    count_letter_counts: [u8; 5],
    count_letter_cap: u8,
}

impl PlacementState {
    fn new(n: usize, rng: &mut Rng) -> Self {
        let mut slots: [u8; MAX_N] = std::array::from_fn(|i| i as u8);
        rng.shuffle(&mut slots[..n]);

        let mut caps = [3u8; 32];
        caps[QuestionTypeKind::LetterDist as u8 as usize] = 1;
        caps[QuestionTypeKind::AnswerOf as u8 as usize] = 2;

        let mut group_caps = [3u8; 8];
        let vc_group = symmetric_group(QuestionTypeKind::CountVowel).unwrap() as usize;
        #[allow(clippy::if_same_then_else)]
        if n <= 8 {
            group_caps[vc_group] = 1;
        } else if rng.int(0, 1) == 0 {
            group_caps[vc_group] = 1;
        } else {
            group_caps[vc_group] = 2;
        }

        PlacementState {
            question_types: [QuestionType::AnswerIsSelf; MAX_N],
            slots,
            assigned: 0,
            assigned_count: 0,
            used_types: [QuestionType::AnswerIsSelf; MAX_N],
            used_count: 0,
            kind_counts: [0; 32],
            group_counts: [0; 8],
            group_caps,
            caps,
            count_letter_counts: [0; 5],
            count_letter_cap: if rng.int(0, 3) == 0 { 2 } else { 1 },
        }
    }

    fn try_place(
        &mut self,
        kind: QuestionTypeKind,
        solution: &[Answer; MAX_N],
        n: usize,
        rng: &mut Rng,
    ) -> bool {
        let ki = kind as u8 as usize;
        let gi = symmetric_group(kind);

        if self.kind_counts[ki] >= self.caps[ki] {
            return false;
        }
        if let Some(g) = gi
            && self.group_counts[g as usize] >= self.group_caps[g as usize]
        {
            return false;
        }
        if !solution_fits_type(kind, self.slots[self.assigned_count] as usize, solution, n) {
            return false;
        }

        let qi = self.slots[self.assigned_count] as usize;
        for _ in 0..10 {
            if let Some(qt) = random_type_params(kind, qi, n, solution, self.assigned, rng) {
                if let Some(a) = count_type_answer(&qt)
                    && self.count_letter_counts[a.idx()] >= self.count_letter_cap
                {
                    continue;
                }
                if types_contain(&self.used_types, self.used_count, &qt) {
                    continue;
                }
                if !solution_satisfies_type(&qt, qi, solution, n) {
                    continue;
                }

                if let Some(a) = count_type_answer(&qt) {
                    self.count_letter_counts[a.idx()] += 1;
                }
                self.question_types[qi] = qt;
                self.used_types[self.used_count] = qt;
                self.used_count += 1;
                self.assigned |= 1 << qi;
                self.assigned_count += 1;
                self.kind_counts[ki] += 1;
                if let Some(g) = gi {
                    self.group_counts[g as usize] += 1;
                }
                return true;
            }
        }
        false
    }

    fn swap_slot_to_front(&mut self, needed_qi: usize) {
        for i in self.assigned_count..self.slots.len() {
            if self.slots[i] as usize == needed_qi {
                self.slots.swap(i, self.assigned_count);
                return;
            }
        }
    }

    fn set_extra_chain(&mut self) {
        self.caps[QuestionTypeKind::AnswerOf as u8 as usize] = 3;
        self.caps[QuestionTypeKind::LetterDist as u8 as usize] = 0;
    }
}

// ── Main construction ──

fn try_construct(
    profile: &DifficultyProfile,
    rng: &mut Rng,
    stats: &mut Stats,
) -> Option<GenerateResult> {
    let n = profile.question_count;

    let mut solution: [Answer; MAX_N] =
        std::array::from_fn(|i| if i < n { rng.pick(&LETTERS) } else { Answer::A });

    bias_consecutive_pair(&mut solution, n, profile, rng);

    let mut state = PlacementState::new(n, rng);

    let extra_chain = profile
        .allowed_types
        .contains(&QuestionTypeKind::LetterDist)
        && rng.int(0, 3) == 0;
    if extra_chain {
        state.set_extra_chain();
    }

    let av_counting = filter_allowed(COUNTING_TYPES, profile);
    let av_positional = filter_allowed(POSITIONAL_TYPES, profile);
    let av_fill = filter_allowed(FILL_TYPES, profile);

    // Phase 1: Counting entry point
    if av_counting.is_empty() || !state.try_place(rng.pick(&av_counting), &solution, n, rng) {
        return None;
    }

    // Phase 2: answer_of backbone
    let chain_count = if extra_chain {
        3
    } else if n <= 5 && rng.int(0, 1) == 0 {
        1
    } else {
        2
    }
    .min(n - state.assigned_count);
    for _ in 0..chain_count {
        if !state.try_place(QuestionTypeKind::AnswerOf, &solution, n, rng) {
            return None;
        }
    }

    // Phase 3: Optional prev_same/next_same at edge positions
    if rng.int(0, 1) == 0 && state.assigned_count < n {
        let candidates: &[(QuestionTypeKind, usize)] = &[
            (QuestionTypeKind::PrevSame, n - 1),
            (QuestionTypeKind::NextSame, 0),
        ];
        let (kind, needed_qi) = candidates[rng.int(0, 1) as usize];
        if profile.allowed_types.contains(&kind) && (state.assigned & (1 << needed_qi)) == 0 {
            state.swap_slot_to_front(needed_qi);
            state.try_place(kind, &solution, n, rng);
        }
    }

    // Phase 4: Positional types
    let pos_count = if av_positional.is_empty() {
        0
    } else {
        2.max(n / 5).min(n - state.assigned_count)
    };
    for _ in 0..pos_count {
        if !av_positional.is_empty() && state.assigned_count < n {
            state.try_place(rng.pick(&av_positional), &solution, n, rng);
        }
    }

    // Phase 5: Exotic guaranteed types
    let exotic: &[QuestionTypeKind] = &[
        QuestionTypeKind::LetterDist,
        QuestionTypeKind::TrueStmt,
        QuestionTypeKind::ConsecIdent,
    ];
    for &kind in exotic {
        if state.assigned_count >= n {
            break;
        }
        if profile.allowed_types.contains(&kind) {
            state.try_place(kind, &solution, n, rng);
        }
    }

    // Phase 6: Fill remaining (reserving slots for constrained types)
    let av_constrained: ArrayVec<QuestionTypeKind, 32> = av_fill
        .iter()
        .copied()
        .filter(|k| is_constrained_type(*k))
        .collect();
    let constrained_reserve = if av_constrained.is_empty() {
        0
    } else {
        1.min(n - state.assigned_count)
    };
    let fill_target = n - constrained_reserve;

    let mut fill_pool: ArrayVec<QuestionTypeKind, 32> = ArrayVec::new();
    fill_pool.try_extend_from_slice(&av_counting).ok();
    fill_pool.try_extend_from_slice(&av_positional).ok();
    fill_pool.try_extend_from_slice(&av_fill).ok();
    fill_pool.retain(|k| *k != QuestionTypeKind::AnswerOf);

    while state.assigned_count < fill_target {
        let mut placed = false;
        for _ in 0..20 {
            if !fill_pool.is_empty() && state.try_place(rng.pick(&fill_pool), &solution, n, rng) {
                placed = true;
                break;
            }
        }
        if !placed
            && !state.try_place(QuestionTypeKind::AnswerOf, &solution, n, rng)
            && !state.try_place(QuestionTypeKind::AnswerIsSelf, &solution, n, rng)
        {
            return None;
        }
    }

    // Phase 7: Constrained types (need specific solution properties)
    for _ in 0..constrained_reserve {
        if state.assigned_count >= n {
            break;
        }
        let qi = state.slots[state.assigned_count] as usize;
        let mut fitting: ArrayVec<QuestionTypeKind, 32> = av_constrained
            .iter()
            .copied()
            .filter(|&k| solution_fits_type(k, qi, &solution, n))
            .collect();
        rng.shuffle(&mut fitting);
        let mut placed = false;
        for &kind in &fitting {
            if state.try_place(kind, &solution, n, rng) {
                placed = true;
                break;
            }
        }
        if !placed {
            for _ in 0..20 {
                if !fill_pool.is_empty() && state.try_place(rng.pick(&fill_pool), &solution, n, rng)
                {
                    placed = true;
                    break;
                }
            }
            if !placed
                && !state.try_place(QuestionTypeKind::AnswerOf, &solution, n, rng)
                && !state.try_place(QuestionTypeKind::AnswerIsSelf, &solution, n, rng)
            {
                return None;
            }
        }
    }

    let mut fp = build_flat_puzzle(&state.question_types, &solution, n, rng)?;

    if !validate_and_repair(&state.question_types, &solution, &mut fp, n, rng, stats) {
        return None;
    }

    Some(GenerateResult {
        question_types: state.question_types,
        fp,
        n,
    })
}

// ── Helpers ──

fn bias_consecutive_pair(
    solution: &mut [Answer; MAX_N],
    n: usize,
    profile: &DifficultyProfile,
    rng: &mut Rng,
) {
    if !profile
        .allowed_types
        .contains(&QuestionTypeKind::ConsecIdent)
        || rng.int(0, 1) != 0
    {
        return;
    }
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
        let keep = rng.int(0, pair_count as i32 - 1) as usize;
        for k in 0..pair_count {
            if k != keep {
                let pos = pair_positions[k] as usize + 1;
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

fn filter_allowed(
    types: &[QuestionTypeKind],
    profile: &DifficultyProfile,
) -> ArrayVec<QuestionTypeKind, 32> {
    types
        .iter()
        .copied()
        .filter(|t| profile.allowed_types.contains(t))
        .collect()
}

fn symmetric_group(kind: QuestionTypeKind) -> Option<u8> {
    match kind {
        QuestionTypeKind::FirstWith | QuestionTypeKind::LastWith => Some(0),
        QuestionTypeKind::ClosestAfter | QuestionTypeKind::ClosestBefore => Some(1),
        QuestionTypeKind::NextSame | QuestionTypeKind::PrevSame => Some(2),
        QuestionTypeKind::CountAnswerBefore | QuestionTypeKind::CountAnswerAfter => Some(3),
        QuestionTypeKind::CountVowel | QuestionTypeKind::CountConsonant => Some(4),
        QuestionTypeKind::LeastCommon | QuestionTypeKind::MostCommon => Some(5),
        _ => None,
    }
}

fn count_type_answer(qt: &QuestionType) -> Option<Answer> {
    match qt {
        QuestionType::CountAnswer { answer }
        | QuestionType::CountAnswerBefore { answer, .. }
        | QuestionType::CountAnswerAfter { answer, .. } => Some(*answer),
        _ => None,
    }
}

fn types_contain(used: &[QuestionType; MAX_N], count: usize, qt: &QuestionType) -> bool {
    (0..count).any(|i| used[i] == *qt)
}

fn is_constrained_type(kind: QuestionTypeKind) -> bool {
    matches!(
        kind,
        QuestionTypeKind::ConsecIdent
            | QuestionTypeKind::Unique
            | QuestionTypeKind::OnlySame
            | QuestionTypeKind::OnlyOdd
            | QuestionTypeKind::OnlyEven
    )
}

/// Checks whether the solution has the properties needed for this type at this position.
fn solution_fits_type(kind: QuestionTypeKind, qi: usize, sol: &[Answer; MAX_N], n: usize) -> bool {
    match kind {
        QuestionTypeKind::LeastCommon => {
            let counts = letter_counts(sol, n);
            let min = *counts.iter().min().unwrap_or(&0);
            counts.iter().filter(|&&c| c == min).count() == 1
        }
        QuestionTypeKind::MostCommon => {
            let counts = letter_counts(sol, n);
            let max = *counts.iter().max().unwrap_or(&0);
            counts.iter().filter(|&&c| c == max).count() == 1
        }
        QuestionTypeKind::SameAs => (0..n).any(|i| i != qi && sol[i] == sol[qi]),
        QuestionTypeKind::Unique => {
            let counts = letter_counts(sol, n);
            counts.iter().filter(|&&c| c == 1).count() == 1
        }
        QuestionTypeKind::EqualCount => true,
        _ if is_constrained_type(kind) => solution_satisfies_type_for_kind(kind, qi, sol, n),
        _ => true,
    }
}

fn solution_satisfies_type_for_kind(
    kind: QuestionTypeKind,
    qi: usize,
    sol: &[Answer; MAX_N],
    n: usize,
) -> bool {
    match kind {
        QuestionTypeKind::ConsecIdent => {
            let mut pairs = 0;
            for i in 0..n.saturating_sub(1) {
                if sol[i] == sol[i + 1] {
                    pairs += 1;
                }
            }
            pairs == 1
        }
        QuestionTypeKind::Unique => count_letter(sol, sol[qi], n) == 1,
        QuestionTypeKind::OnlySame => {
            let mut m = 0;
            for i in 0..n {
                if i != qi && sol[i] == sol[qi] {
                    m += 1;
                }
            }
            m == 1
        }
        QuestionTypeKind::OnlyOdd | QuestionTypeKind::OnlyEven => {
            let parity = if kind == QuestionTypeKind::OnlyOdd {
                1
            } else {
                0
            };
            LETTERS.iter().any(|&letter| {
                let mut m = 0;
                for i in 0..n {
                    if (i + 1) % 2 == parity && sol[i] == letter {
                        m += 1;
                    }
                }
                m == 1
            })
        }
        _ => true,
    }
}

fn random_type_params(
    kind: QuestionTypeKind,
    qi: usize,
    n: usize,
    solution: &[Answer; MAX_N],
    assigned: u16,
    rng: &mut Rng,
) -> Option<QuestionType> {
    match kind {
        QuestionTypeKind::CountAnswer => Some(QuestionType::CountAnswer {
            answer: rng.pick(&LETTERS),
        }),
        QuestionTypeKind::CountAnswerBefore => {
            if n < 6 {
                return None;
            }
            Some(QuestionType::CountAnswerBefore {
                answer: rng.pick(&LETTERS),
                before_index: rng.int(4, n as i32 - 1) as u8,
            })
        }
        QuestionTypeKind::CountAnswerAfter => {
            if n < 6 {
                return None;
            }
            Some(QuestionType::CountAnswerAfter {
                answer: rng.pick(&LETTERS),
                after_index: rng.int(0, (n as i32 - 5).max(0)) as u8,
            })
        }
        QuestionTypeKind::CountVowel => Some(QuestionType::CountVowel),
        QuestionTypeKind::CountConsonant => Some(QuestionType::CountConsonant),
        QuestionTypeKind::MostCommonCount => Some(QuestionType::MostCommonCount),
        QuestionTypeKind::AnswerOf => {
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
            Some(QuestionType::AnswerOf {
                question_index: rng.pick(&pool[..plen]),
            })
        }
        QuestionTypeKind::LetterDist => {
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
            Some(QuestionType::LetterDist {
                question_index: rng.pick(&pool[..plen]),
            })
        }
        QuestionTypeKind::ClosestAfter => Some(QuestionType::ClosestAfter {
            after_index: rng.int(0, (n as i32 - 5).max(0)) as u8,
            answer: rng.pick(&LETTERS),
        }),
        QuestionTypeKind::ClosestBefore => {
            if n < 5 {
                return None;
            }
            Some(QuestionType::ClosestBefore {
                before_index: rng.int(4, n as i32 - 1) as u8,
                answer: rng.pick(&LETTERS),
            })
        }
        QuestionTypeKind::FirstWith => Some(QuestionType::FirstWith {
            answer: rng.pick(&LETTERS),
        }),
        QuestionTypeKind::LastWith => Some(QuestionType::LastWith {
            answer: rng.pick(&LETTERS),
        }),
        QuestionTypeKind::PrevSame => {
            if qi < 4 {
                return None;
            }
            Some(QuestionType::PrevSame)
        }
        QuestionTypeKind::NextSame => {
            if qi + 5 > n {
                return None;
            }
            Some(QuestionType::NextSame)
        }
        QuestionTypeKind::OnlySame => Some(QuestionType::OnlySame),
        QuestionTypeKind::SameAs => Some(QuestionType::SameAs),
        QuestionTypeKind::ConsecIdent => Some(QuestionType::ConsecIdent),
        QuestionTypeKind::OnlyOdd | QuestionTypeKind::OnlyEven => {
            let answer = rng.pick(&LETTERS);
            Some(if kind == QuestionTypeKind::OnlyOdd {
                QuestionType::OnlyOdd { answer }
            } else {
                QuestionType::OnlyEven { answer }
            })
        }
        QuestionTypeKind::LeastCommon => Some(QuestionType::LeastCommon),
        QuestionTypeKind::MostCommon => Some(QuestionType::MostCommon),
        QuestionTypeKind::Unique => Some(QuestionType::Unique),
        QuestionTypeKind::EqualCount => {
            let ref_letter = rng.pick(&LETTERS);
            let ref_count = count_letter(solution, ref_letter, n);
            let has_match = LETTERS
                .iter()
                .any(|&l| l != ref_letter && count_letter(solution, l, n) == ref_count);
            if !has_match && rng.int(0, 4) > 1 {
                return None;
            }
            Some(QuestionType::EqualCount { answer: ref_letter })
        }
        QuestionTypeKind::AnswerIsSelf => Some(QuestionType::AnswerIsSelf),
        QuestionTypeKind::TrueStmt => Some(QuestionType::TrueStmt),
    }
}
