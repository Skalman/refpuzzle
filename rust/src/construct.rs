use arrayvec::ArrayVec;

use crate::build::{
    GenerateResult, Stats, count_letter, fill_options, letter_counts, solution_satisfies_type,
    validate_and_repair,
};
use crate::difficulty::DifficultyProfile;
use crate::format::format_type_tag;
use crate::rng::Rng;
use crate::types::*;

fn sort_dedup<T: Ord, const N: usize>(v: &mut ArrayVec<T, N>) {
    v.sort();
    let mut i = 1;
    while i < v.len() {
        if v[i] == v[i - 1] {
            v.remove(i);
        } else {
            i += 1;
        }
    }
}

pub fn format_claim_qt(qt: &QuestionType) -> serde_json::Value {
    let type_name = match qt {
        QuestionType::CountAnswer { .. } => "CountAnswer",
        QuestionType::CountConsonant => "CountConsonant",
        QuestionType::CountVowel => "CountVowel",
        QuestionType::CountAnswerAfter { .. } => "CountAnswerAfter",
        QuestionType::CountAnswerBefore { .. } => "CountAnswerBefore",
        QuestionType::AnswerOf { .. } => "AnswerOf",
        QuestionType::FirstWith { .. } => "FirstWith",
        QuestionType::LastWith { .. } => "LastWith",
        QuestionType::MostCommon => "MostCommon",
        QuestionType::LeastCommon => "LeastCommon",
        QuestionType::MostCommonCount => "MostCommonCount",
        QuestionType::NoOtherHasAnswer => "NoOtherHasAnswer",
        QuestionType::ConsecIdent => "ConsecIdent",
        QuestionType::OnlyOdd { .. } => "OnlyOdd",
        QuestionType::OnlyEven { .. } => "OnlyEven",
        QuestionType::EqualCount { .. } => "EqualCount",
        QuestionType::ClosestAfter { .. } => "ClosestAfter",
        QuestionType::ClosestBefore { .. } => "ClosestBefore",
        QuestionType::SameAsWhich { .. } => "SameAsWhich",
        _ => "Unknown",
    };
    let mut obj = serde_json::Map::new();
    obj.insert("type".into(), serde_json::json!(type_name));
    match *qt {
        QuestionType::CountAnswer { answer }
        | QuestionType::FirstWith { answer }
        | QuestionType::LastWith { answer }
        | QuestionType::OnlyOdd { answer }
        | QuestionType::OnlyEven { answer }
        | QuestionType::EqualCount { answer } => {
            obj.insert(
                "answer".into(),
                serde_json::json!(answer.as_char().to_string()),
            );
        }
        QuestionType::CountAnswerAfter {
            answer,
            after_index,
        } => {
            obj.insert(
                "answer".into(),
                serde_json::json!(answer.as_char().to_string()),
            );
            obj.insert("afterIndex".into(), serde_json::json!(after_index));
        }
        QuestionType::CountAnswerBefore {
            answer,
            before_index,
        } => {
            obj.insert(
                "answer".into(),
                serde_json::json!(answer.as_char().to_string()),
            );
            obj.insert("beforeIndex".into(), serde_json::json!(before_index));
        }
        QuestionType::ClosestAfter {
            answer,
            after_index,
        } => {
            obj.insert(
                "answer".into(),
                serde_json::json!(answer.as_char().to_string()),
            );
            obj.insert("afterIndex".into(), serde_json::json!(after_index));
        }
        QuestionType::ClosestBefore {
            answer,
            before_index,
        } => {
            obj.insert(
                "answer".into(),
                serde_json::json!(answer.as_char().to_string()),
            );
            obj.insert("beforeIndex".into(), serde_json::json!(before_index));
        }
        QuestionType::AnswerOf { question_index }
        | QuestionType::LetterDist { question_index }
        | QuestionType::SameAsWhich { question_index } => {
            obj.insert("questionIndex".into(), serde_json::json!(question_index));
        }
        _ => {}
    }
    serde_json::Value::Object(obj)
}

pub fn generate(
    profile: &DifficultyProfile,
    rng: &mut Rng,
    max_attempts: usize,
    stats: &mut Stats,
    trace: bool,
    label: &str,
) -> Option<GenerateResult> {
    for attempt in 0..max_attempts {
        if let Some(r) = try_construct(profile, rng, stats, trace, attempt, label) {
            if trace {
                eprintln!(
                    "{}",
                    serde_json::json!({"t": "success", "attempt": attempt + 1})
                );
            }
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
    QuestionTypeKind::NoOtherHasAnswer,
    QuestionTypeKind::EqualCount,
    QuestionTypeKind::AnswerIsSelf,
    QuestionTypeKind::TrueStmt,
    QuestionTypeKind::SameAsWhich,
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
        oc: usize,
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
        if !solution_fits_type(
            kind,
            self.slots[self.assigned_count] as usize,
            solution,
            n,
            oc,
        ) {
            return false;
        }

        let qi = self.slots[self.assigned_count] as usize;
        for _ in 0..10 {
            if let Some(qt) = random_type_params(kind, qi, n, oc, solution, self.assigned, rng) {
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
    trace: bool,
    attempt: usize,
    label: &str,
) -> Option<GenerateResult> {
    let n = profile.question_count;
    let oc = profile.option_count;
    let letters = &LETTERS[..oc];

    let mut solution: [Answer; MAX_N] =
        std::array::from_fn(|i| if i < n { rng.pick(letters) } else { Answer::A });

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

    let trace_phase = |name: &str, state: &PlacementState| {
        if trace {
            let placed: Vec<serde_json::Value> = (0..n)
                .filter(|&i| (state.assigned >> i) & 1 == 1)
                .map(|i| {
                    serde_json::json!({"qi": i, "type": format_type_tag(&state.question_types[i])})
                })
                .collect();
            eprintln!(
                "{}",
                serde_json::json!({"t": "phase", "name": name, "placed": placed})
            );
        }
    };

    // Phase 1: Counting entry point (skip 50% of the time for small puzzles)
    let skip_counting = n <= 3 && rng.int(0, 1) == 0;
    if !skip_counting
        && (av_counting.is_empty()
            || !state.try_place(rng.pick(&av_counting), &solution, n, oc, rng))
    {
        {
            if trace {
                eprintln!(
                    "{}",
                    serde_json::json!({"t": "construct_failed", "attempt": attempt + 1})
                );
            }
            return None;
        }
    }

    trace_phase("p1", &state);

    // Phase 2: answer_of backbone
    let chain_count = if extra_chain {
        3
    } else if n <= 3 {
        if rng.int(0, 1) == 0 { 1 } else { 0 }
    } else if n <= 5 && rng.int(0, 1) == 0 {
        1
    } else {
        2
    }
    .min(n - state.assigned_count);
    for _ in 0..chain_count {
        if !state.try_place(QuestionTypeKind::AnswerOf, &solution, n, oc, rng) {
            if trace {
                eprintln!(
                    "{}",
                    serde_json::json!({"t": "construct_failed", "attempt": attempt + 1})
                );
            }
            return None;
        }
    }

    trace_phase("p2", &state);

    // Phase 3: Optional prev_same/next_same at edge positions
    let p3_check = rng.int(0, 1);
    if p3_check == 0 && state.assigned_count < n {
        let candidates: &[(QuestionTypeKind, usize)] = &[
            (QuestionTypeKind::PrevSame, n - 1),
            (QuestionTypeKind::NextSame, 0),
        ];
        let cand_idx = rng.int(0, 1) as usize;
        let (kind, needed_qi) = candidates[cand_idx];
        if profile.allowed_types.contains(&kind) && (state.assigned & (1 << needed_qi)) == 0 {
            state.swap_slot_to_front(needed_qi);
            state.try_place(kind, &solution, n, oc, rng);
        }
    }

    trace_phase("p3", &state);

    // Phase 4: Positional types (skip for tiny puzzles)
    let pos_count = if av_positional.is_empty() || n <= 3 {
        0
    } else {
        2.max(n / 5).min(n - state.assigned_count)
    };
    for _ in 0..pos_count {
        if !av_positional.is_empty() && state.assigned_count < n {
            state.try_place(rng.pick(&av_positional), &solution, n, oc, rng);
        }
    }

    trace_phase("p4", &state);

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
            state.try_place(kind, &solution, n, oc, rng);
        }
    }

    trace_phase("p5", &state);

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
    sort_dedup(&mut fill_pool);

    while state.assigned_count < fill_target {
        let mut placed = false;
        for _ in 0..20 {
            if !fill_pool.is_empty() && state.try_place(rng.pick(&fill_pool), &solution, n, oc, rng)
            {
                placed = true;
                break;
            }
        }
        if !placed
            && !state.try_place(QuestionTypeKind::AnswerOf, &solution, n, oc, rng)
            && !state.try_place(QuestionTypeKind::AnswerIsSelf, &solution, n, oc, rng)
        {
            if trace {
                eprintln!(
                    "{}",
                    serde_json::json!({"t": "construct_failed", "attempt": attempt + 1})
                );
            }
            return None;
        }
    }

    trace_phase("p6", &state);

    // Phase 7: Structural types (need specific solution properties)
    for _ in 0..constrained_reserve {
        if state.assigned_count >= n {
            break;
        }
        let qi = state.slots[state.assigned_count] as usize;
        let mut fitting: ArrayVec<QuestionTypeKind, 32> = av_constrained
            .iter()
            .copied()
            .filter(|&k| solution_fits_type(k, qi, &solution, n, oc))
            .collect();
        rng.shuffle(&mut fitting);
        let mut placed = false;
        for &kind in &fitting {
            if state.try_place(kind, &solution, n, oc, rng) {
                placed = true;
                break;
            }
        }
        if !placed {
            for _ in 0..20 {
                if !fill_pool.is_empty()
                    && state.try_place(rng.pick(&fill_pool), &solution, n, oc, rng)
                {
                    placed = true;
                    break;
                }
            }
            if !placed
                && !state.try_place(QuestionTypeKind::AnswerOf, &solution, n, oc, rng)
                && !state.try_place(QuestionTypeKind::AnswerIsSelf, &solution, n, oc, rng)
            {
                if trace {
                    eprintln!(
                        "{}",
                        serde_json::json!({"t": "construct_failed", "attempt": attempt + 1})
                    );
                }
                return None;
            }
        }
    }

    trace_phase("p7", &state);

    if trace {
        let sol_str: String = solution.iter().take(n).map(|a| a.as_char()).collect();
        eprintln!(
            "{}",
            serde_json::json!({"t": "attempt", "attempt": attempt + 1, "solution": sol_str, "rng": rng.state()})
        );
    }

    let Some(mut fp) = fill_options(
        &state.question_types,
        &solution,
        n,
        profile.option_count,
        rng,
        trace,
    ) else {
        if trace {
            eprintln!(
                "{}",
                serde_json::json!({"t": "construct_failed", "attempt": attempt + 1})
            );
        }
        return None;
    };

    if !validate_and_repair(
        &state.question_types,
        &solution,
        &mut fp,
        n,
        rng,
        stats,
        trace,
        label,
    ) {
        if trace {
            eprintln!(
                "{}",
                serde_json::json!({"t": "failed", "attempt": attempt + 1})
            );
        }
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
                    let new_letter = rng.pick(&LETTERS[..profile.option_count]);
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
            | QuestionTypeKind::NoOtherHasAnswer
            | QuestionTypeKind::OnlySame
            | QuestionTypeKind::OnlyOdd
            | QuestionTypeKind::OnlyEven
    )
}

/// Checks whether the solution has the properties needed for this type at this position.
fn solution_fits_type(
    kind: QuestionTypeKind,
    qi: usize,
    sol: &[Answer; MAX_N],
    n: usize,
    oc: usize,
) -> bool {
    match kind {
        QuestionTypeKind::LeastCommon => {
            let counts = letter_counts(sol, n);
            let min = *counts[..oc].iter().min().unwrap_or(&0);
            counts[..oc].iter().filter(|&&c| c == min).count() == 1
        }
        QuestionTypeKind::MostCommon => {
            let counts = letter_counts(sol, n);
            let max = *counts[..oc].iter().max().unwrap_or(&0);
            counts[..oc].iter().filter(|&&c| c == max).count() == 1
        }
        QuestionTypeKind::SameAs => n > oc && (0..n).any(|i| i != qi && sol[i] == sol[qi]),
        QuestionTypeKind::SameAsWhich => true,
        QuestionTypeKind::NoOtherHasAnswer => {
            count_letter(sol, sol[qi], n) == 1
                && LETTERS[..oc]
                    .iter()
                    .all(|&l| l == sol[qi] || count_letter(sol, l, n) >= 1)
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
        QuestionTypeKind::NoOtherHasAnswer => count_letter(sol, sol[qi], n) == 1,
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
    option_count: usize,
    solution: &[Answer; MAX_N],
    assigned: u16,
    rng: &mut Rng,
) -> Option<QuestionType> {
    let letters = &LETTERS[..option_count];
    match kind {
        QuestionTypeKind::CountAnswer => Some(QuestionType::CountAnswer {
            answer: rng.pick(letters),
        }),
        QuestionTypeKind::CountAnswerBefore => {
            if n < 6 {
                return None;
            }
            Some(QuestionType::CountAnswerBefore {
                answer: rng.pick(letters),
                before_index: rng.int(4, n as i32 - 1) as u8,
            })
        }
        QuestionTypeKind::CountAnswerAfter => {
            if n < 6 {
                return None;
            }
            Some(QuestionType::CountAnswerAfter {
                answer: rng.pick(letters),
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
            answer: rng.pick(letters),
        }),
        QuestionTypeKind::ClosestBefore => {
            if n < 5 {
                return None;
            }
            Some(QuestionType::ClosestBefore {
                before_index: rng.int(4, n as i32 - 1) as u8,
                answer: rng.pick(letters),
            })
        }
        QuestionTypeKind::FirstWith => Some(QuestionType::FirstWith {
            answer: rng.pick(letters),
        }),
        QuestionTypeKind::LastWith => Some(QuestionType::LastWith {
            answer: rng.pick(letters),
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
            let answer = rng.pick(letters);
            Some(if kind == QuestionTypeKind::OnlyOdd {
                QuestionType::OnlyOdd { answer }
            } else {
                QuestionType::OnlyEven { answer }
            })
        }
        QuestionTypeKind::LeastCommon => Some(QuestionType::LeastCommon),
        QuestionTypeKind::MostCommon => Some(QuestionType::MostCommon),
        QuestionTypeKind::NoOtherHasAnswer => Some(QuestionType::NoOtherHasAnswer),
        QuestionTypeKind::EqualCount => {
            let ref_letter = rng.pick(letters);
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
        QuestionTypeKind::SameAsWhich => {
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
            let ref_qi = rng.pick(&pool[..plen]) as usize;
            if solution[ref_qi] == solution[qi] {
                return None;
            }
            if !(0..n).any(|j| j != qi && j != ref_qi && solution[j] == solution[ref_qi]) {
                return None;
            }
            Some(QuestionType::SameAsWhich {
                question_index: ref_qi as u8,
            })
        }
    }
}
