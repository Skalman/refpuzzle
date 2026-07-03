use arrayvec::ArrayVec;

use crate::build::{
    GenerateResult, Stats, count_letter, fill_options, solution_satisfies_type, validate_and_repair,
};
use crate::check_answerable::answerable;
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
    let n = profile.question_count;
    let oc = profile.option_count;

    // Solution is fixed across retries; only the rule-placement attempts vary.
    let mut solution: [Answer; MAX_N] = std::array::from_fn(|i| {
        if i < n {
            rng.pick_letter(oc)
        } else {
            Answer::A
        }
    });
    bias_consecutive_pair(&mut solution, n, profile, rng);

    for attempt in 0..max_attempts {
        if let Some(r) = try_construct(profile, rng, stats, trace, attempt, label, &solution) {
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
    fn new(profile: &DifficultyProfile, rng: &mut Rng) -> Self {
        let n = profile.question_count;
        let mut slots: [u8; MAX_N] = std::array::from_fn(|i| i as u8);
        rng.shuffle(&mut slots[..n]);

        let caps = profile.caps;

        let mut group_caps = [3u8; 8];
        let vc_group = symmetric_group(QuestionTypeKind::CountVowel).unwrap() as usize;
        group_caps[vc_group] = profile.vowel_consonant_cap.sample(rng);

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
            count_letter_cap: profile.count_letter_cap.sample(rng),
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
        if !solution_fits_kind(
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
                if !solution_satisfies_type(&qt, qi, solution, n, oc) {
                    continue;
                }
                // Damp the "no match" outcome for Only*/ConsecIdent so the
                // correct%/distractor% ratio for None stays near 1.0 — without
                // damping it's >2x because matches==0 is naturally common.
                if is_only_no_match(&qt, qi, solution, n)
                    && rng.next_f64() < no_match_reject_prob(&qt)
                {
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

    /// A long AnswerOf chain (>=3) needs its cap raised to fit, and suppresses
    /// LetterDist — the chain already supplies the self-reference spine.
    fn prepare_chain(&mut self, chain_count: usize) {
        if chain_count >= 3 {
            self.caps[QuestionTypeKind::AnswerOf as usize] = chain_count as u8;
            self.caps[QuestionTypeKind::LetterDist as usize] = 0;
        }
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
    solution: &[Answer; MAX_N],
) -> Option<GenerateResult> {
    let n = profile.question_count;
    let oc = profile.option_count;

    let mut state = PlacementState::new(profile, rng);

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

    // Phase 1: counting anchor (profile sets the count; family pick per slot)
    let n_counting = profile.phase_1_n_counting.sample(rng);
    for _ in 0..n_counting {
        if av_counting.is_empty() || !state.try_place(rng.pick(&av_counting), solution, n, oc, rng)
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

    // Phase 2: answer_of chain (self-reference backbone).
    let chain_count = profile
        .phase_2_n_answer_of
        .sample(rng)
        .min(n - state.assigned_count);
    state.prepare_chain(chain_count);
    for _ in 0..chain_count {
        if !state.try_place(QuestionTypeKind::AnswerOf, solution, n, oc, rng) {
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

    // Phase 3: optionally place one PrevSame/NextSame at an edge slot. The
    // count is profile-controlled; the edge-vs-edge pick stays internal.
    let n_adjacent = profile.phase_3_n_adjacent_same.sample(rng);
    if n_adjacent >= 1 && state.assigned_count < n {
        let candidates: &[(QuestionTypeKind, usize)] = &[
            (QuestionTypeKind::PrevSame, n - 1),
            (QuestionTypeKind::NextSame, 0),
        ];
        let cand_idx = rng.int(0, 1) as usize;
        let (kind, needed_qi) = candidates[cand_idx];
        if profile.allowed_types.contains(&kind) && (state.assigned & (1 << needed_qi)) == 0 {
            state.swap_slot_to_front(needed_qi);
            state.try_place(kind, solution, n, oc, rng);
        }
    }

    trace_phase("p3", &state);

    // Phase 4: Positional types (count from profile; family pick per slot)
    let pos_count = if av_positional.is_empty() {
        0
    } else {
        profile
            .phase_4_n_positional
            .sample(rng)
            .min(n - state.assigned_count)
    };
    for _ in 0..pos_count {
        if !av_positional.is_empty() && state.assigned_count < n {
            state.try_place(rng.pick(&av_positional), solution, n, oc, rng);
        }
    }

    trace_phase("p4", &state);

    // Phase 5: Featured rare types — try for at least one of each, in order.
    for &kind in profile.phase_5_featured {
        if state.assigned_count >= n {
            break;
        }
        state.try_place(kind, solution, n, oc, rng);
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
            if !fill_pool.is_empty() && state.try_place(rng.pick(&fill_pool), solution, n, oc, rng)
            {
                placed = true;
                break;
            }
        }
        if !placed
            && !state.try_place(QuestionTypeKind::AnswerOf, solution, n, oc, rng)
            && !state.try_place(QuestionTypeKind::AnswerIsSelf, solution, n, oc, rng)
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
            .filter(|&k| solution_fits_kind(k, qi, solution, n, oc))
            .collect();
        rng.shuffle(&mut fitting);
        let mut placed = false;
        for &kind in &fitting {
            if state.try_place(kind, solution, n, oc, rng) {
                placed = true;
                break;
            }
        }
        if !placed {
            for _ in 0..20 {
                if !fill_pool.is_empty()
                    && state.try_place(rng.pick(&fill_pool), solution, n, oc, rng)
                {
                    placed = true;
                    break;
                }
            }
            if !placed
                && !state.try_place(QuestionTypeKind::AnswerOf, solution, n, oc, rng)
                && !state.try_place(QuestionTypeKind::AnswerIsSelf, solution, n, oc, rng)
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

    let mut fp = fill_options(
        &state.question_types,
        solution,
        n,
        profile.option_count,
        rng,
        trace,
    );

    if !validate_and_repair(
        &state.question_types,
        solution,
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
                    let new_letter = rng.pick_letter(profile.option_count);
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
pub(crate) fn solution_fits_kind(
    kind: QuestionTypeKind,
    qi: usize,
    sol: &[Answer; MAX_N],
    n: usize,
    oc: usize,
) -> bool {
    match kind {
        // MC/LC answerability (unique extreme) lives in `check_answerable` — the one
        // build-time source, shared with v2's parametrize and `refpuzzle check`.
        QuestionTypeKind::LeastCommon => answerable(&QuestionType::LeastCommon, sol, n, oc),
        QuestionTypeKind::MostCommon => answerable(&QuestionType::MostCommon, sol, n, oc),
        QuestionTypeKind::SameAs => {
            // Pool capacity for SameAs at qi depends on how many questions share qi's answer:
            //   same_count == 1 (qi is unique): correct = null, pool = n-1 other Qs, no null.
            //   same_count >= 2: correct = a same-answer Q, pool = (n - same_count) differing-Q + 1 null.
            // We need pool >= oc - 1 (one distractor per non-correct option).
            let same_count = count_letter(sol, sol[qi], n) as usize;
            let pool = if same_count == 1 {
                n - 1
            } else {
                n - same_count + 1
            };
            pool >= oc - 1
        }
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
            pairs <= 1
        }
        QuestionTypeKind::NoOtherHasAnswer => count_letter(sol, sol[qi], n) == 1,
        QuestionTypeKind::OnlySame => {
            let mut m = 0;
            for i in 0..n {
                if i != qi && sol[i] == sol[qi] {
                    m += 1;
                }
            }
            m <= 1
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
                m <= 1
            })
        }
        _ => true,
    }
}

/// For `OnlySame`/`OnlyOdd`/`OnlyEven`/`ConsecIdent`, the correct answer is
/// `None` precisely when no other question (or pair) matches. We allow both
/// the matching and non-matching cases so players see "no match" outcomes,
/// but the natural rate of non-match cases is high enough at small n that
/// without damping it would dominate. `try_place` consults this.
fn is_only_no_match(qt: &QuestionType, qi: usize, sol: &[Answer; MAX_N], n: usize) -> bool {
    match *qt {
        QuestionType::OnlySame => !(0..n).any(|j| j != qi && sol[j] == sol[qi]),
        QuestionType::OnlyOdd { answer } => !(0..n).any(|i| (i + 1) % 2 == 1 && sol[i] == answer),
        QuestionType::OnlyEven { answer } => !(0..n).any(|i| (i + 1) % 2 == 0 && sol[i] == answer),
        QuestionType::ConsecIdent => !(0..n.saturating_sub(1)).any(|i| sol[i] == sol[i + 1]),
        _ => false,
    }
}

/// Rejection probability for the "no match" outcome, tuned so the resulting
/// correct%/distractor% ratio for None lands near 1.0. Values derived
/// empirically from the type-stats analysis at 10k puzzles/level.
fn no_match_reject_prob(qt: &QuestionType) -> f64 {
    match qt {
        QuestionType::OnlySame => 0.96,
        QuestionType::ConsecIdent => 0.94,
        QuestionType::OnlyOdd { .. } | QuestionType::OnlyEven { .. } => 0.85,
        _ => 0.0,
    }
}

pub(crate) fn random_type_params(
    kind: QuestionTypeKind,
    qi: usize,
    n: usize,
    option_count: usize,
    solution: &[Answer; MAX_N],
    assigned: u16,
    rng: &mut Rng,
) -> Option<QuestionType> {
    match kind {
        QuestionTypeKind::CountAnswer => Some(QuestionType::CountAnswer {
            answer: rng.pick_letter(option_count),
        }),
        QuestionTypeKind::CountAnswerBefore => {
            // Need before_index with at least oc distinct count values (0..=before_index).
            if n < option_count {
                return None;
            }
            Some(QuestionType::CountAnswerBefore {
                answer: rng.pick_letter(option_count),
                before_index: rng.int(option_count as i32 - 1, n as i32 - 1) as u8,
            })
        }
        QuestionTypeKind::CountAnswerAfter => {
            // Need after_index with at least oc distinct count values (0..=n-1-after_index).
            if n < option_count {
                return None;
            }
            Some(QuestionType::CountAnswerAfter {
                answer: rng.pick_letter(option_count),
                after_index: rng.int(0, n as i32 - option_count as i32) as u8,
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
        QuestionTypeKind::ClosestAfter => {
            // Need after_index with at least oc distinct option values
            // (positions after_index+1..n, plus null).
            if n < option_count {
                return None;
            }
            Some(QuestionType::ClosestAfter {
                after_index: rng.int(0, n as i32 - option_count as i32) as u8,
                answer: rng.pick_letter(option_count),
            })
        }
        QuestionTypeKind::ClosestBefore => {
            // Need before_index with at least oc distinct option values
            // (positions 0..before_index, plus null).
            if n < option_count {
                return None;
            }
            Some(QuestionType::ClosestBefore {
                before_index: rng.int(option_count as i32 - 1, n as i32 - 1) as u8,
                answer: rng.pick_letter(option_count),
            })
        }
        QuestionTypeKind::FirstWith => Some(QuestionType::FirstWith {
            answer: rng.pick_letter(option_count),
        }),
        QuestionTypeKind::LastWith => Some(QuestionType::LastWith {
            answer: rng.pick_letter(option_count),
        }),
        QuestionTypeKind::PrevSame => {
            // Need oc distinct option values; pool size is qi + 1 (positions [0, qi) + null).
            if qi + 1 < option_count {
                return None;
            }
            Some(QuestionType::PrevSame)
        }
        QuestionTypeKind::NextSame => {
            // Need oc distinct option values; pool size is n - qi (positions (qi, n) + null).
            if n - qi < option_count {
                return None;
            }
            Some(QuestionType::NextSame)
        }
        QuestionTypeKind::OnlySame => Some(QuestionType::OnlySame),
        QuestionTypeKind::SameAs => {
            // "none" (no other question shares this answer) is a valid answer, so there is no
            // structural requirement. Capacity: with "none" as a value, oc distinct options
            // need n >= oc (none case: oc-1 index distractors from the n-1 other questions).
            if n < option_count {
                return None;
            }
            Some(QuestionType::SameAs)
        }
        QuestionTypeKind::ConsecIdent => Some(QuestionType::ConsecIdent),
        QuestionTypeKind::OnlyOdd | QuestionTypeKind::OnlyEven => {
            let answer = rng.pick_letter(option_count);
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
            let ref_letter = rng.pick_letter(option_count);
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
        QuestionTypeKind::TrueStmt => {
            if option_count < 5 {
                return None;
            }
            Some(QuestionType::TrueStmt)
        }
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
            // Structural: another question must share ref's answer.
            // Capacity: need at least oc-1 questions whose answer differs from ref (distractors).
            let mut has_match = false;
            let mut distractor_count = 0usize;
            for j in 0..n {
                if j == qi {
                    continue;
                }
                if solution[j] == solution[ref_qi] {
                    if j != ref_qi {
                        has_match = true;
                    }
                } else {
                    distractor_count += 1;
                }
            }
            if !has_match || distractor_count < option_count - 1 {
                return None;
            }
            Some(QuestionType::SameAsWhich {
                question_index: ref_qi as u8,
            })
        }
    }
}
