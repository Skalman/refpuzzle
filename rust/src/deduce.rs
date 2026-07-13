use arrayvec::ArrayVec;

use crate::types::*;

macro_rules! deduce_rules {
    ($($variant:ident),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[allow(dead_code)]
        pub enum DeduceRule {
            All,
            $($variant),+
        }

        #[allow(dead_code)]
        pub const ALL_DEDUCE_RULES: &[DeduceRule] = &[
            $(DeduceRule::$variant),+
        ];

        #[allow(dead_code)]
        impl DeduceRule {
            pub fn from_str(s: &str) -> Option<DeduceRule> {
                match s {
                    "All" => Some(DeduceRule::All),
                    $(stringify!($variant) => Some(DeduceRule::$variant),)+
                    _ => None,
                }
            }

            pub fn to_str(self) -> &'static str {
                match self {
                    DeduceRule::All => "All",
                    $(DeduceRule::$variant => stringify!($variant)),+
                }
            }
        }
    }
}

deduce_rules! {
    CountSaturated,
    CountMustMatchForce,
    CountMustMatchElim,
    OnlyOptionLeft,
    AnswerOfForward,
    AnswerOfReverse,
    SameAsReverse,
    PrevNextOnlySameReverse,
    LetterDistForward,
    LetterDistReverseForce,
    LetterDistReverseElim,
    CountAllAnswered,
    MostCommonCountElim,
    PositionalRangeAnswered,
    PositionalRangeUnanswered,
    VowelCrossElim,
    ConsonantCrossElim,
    CountExceeded,
    CountImpossible,
    AnswerOfTargetRuledOut,
    LetterDistImpossible,
    LetterDistWrong,
    LetterDistNoMatch,
    FirstClosestAfterOutOfRange,
    FirstClosestAfterWrongAnswer,
    FirstClosestAfterRuledOut,
    FirstClosestAfterEarlierMatch,
    FirstClosestAfterSelfRef,
    FirstClosestAfterNoneMatch,
    LastClosestBeforeOutOfRange,
    LastClosestBeforeWrongAnswer,
    LastClosestBeforeRuledOut,
    LastClosestBeforeLaterMatch,
    LastClosestBeforeSelfRef,
    LastClosestBeforeNoneMatch,
    OnlyOddEvenWrongParity,
    OnlyOddEvenWrongAnswer,
    OnlyOddEvenRuledOut,
    OnlyOddEvenNoneMatch,
    ConsecIdentOutOfRange,
    ConsecIdentSelfRef,
    ConsecIdentNoCommon,
    ConsecIdentNonePair,
    EqualCountSelfRef,
    PrevSameNotBefore,
    PrevSameRuledOut,
    PrevSameCloser,
    NextSameNotAfter,
    NextSameRuledOut,
    NextSameCloser,
    OnlySameSelfRef,
    OnlySameRuledOut,
    UniqueAlreadyUsed,
    LeastCommonElim,
    LeastCommonForce,
    LeastCommonCountFloor,
    TrueStatementForward,
    OnlyOddEvenRangeElim,
    MostCommonElim,
    MostCommonForce,
    MostCommonCountCeil,
    ConsecIdentReverse,
    TrueStatementSelfRef,
    TrueStatementClaimInvalid,
    TrueStatementClaimValid,
    TrueStatementClaimKnownTrue,
    TrueStatementMatchForce,
    TrueStatementMatchElim,
    ConsecIdentForwardForce,
    ConsecIdentForwardElim,
    ConsecIdentForwardBothForce,
    EqualCountRangeElim,
    OnlySameOtherMatch,
    PrevSameNoneMatch,
    NextSameNoneMatch,
    OnlySameNoneMatch,
    OnlySameNoneForward,
    SameAsNegative,
    SameAsWhichForward,
    SameAsWhichReverse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeduceAction {
    Force { qi: usize, answer: Answer },
    Eliminate { qi: usize, oi: usize },
    EliminateMulti { question_mask: u16, option_mask: u8 },
}

#[derive(Clone, Copy, Debug)]
pub struct DeduceResult {
    pub action: DeduceAction,
    #[allow(dead_code)] // used by tests
    pub rule: DeduceRule,
}

/// The question where `action`'s conclusion conflicts with `state`, or `None` if
/// consistent: a `Force` onto a cell already answered otherwise *or* whose target
/// option is eliminated, or an `Eliminate`/`EliminateMulti` striking a cell's
/// current answer. Shared by `run_engine`'s deduce loop (real state) and
/// `lookahead`'s hypothesis probe — in the latter, forcing an already-eliminated
/// option is the primary way a hypothesis gets refuted. A sound engine on a
/// well-posed puzzle never contradicts the real state; `run_engine` surfaces it so
/// generation fails loud on an unsound rule and `check` reports the culprit.
pub(crate) fn contradiction_question(action: &DeduceAction, state: &State) -> Option<usize> {
    match *action {
        DeduceAction::Force { qi, answer } => {
            let conflicts = (state.answers[qi].is_some() && state.answers[qi] != Some(answer))
                || (state.eliminated[qi] >> answer.idx()) & 1 == 1;
            conflicts.then_some(qi)
        }
        DeduceAction::Eliminate { qi, oi } => {
            (state.answers[qi] == Some(Answer::from(oi as u8))).then_some(qi)
        }
        DeduceAction::EliminateMulti {
            question_mask,
            option_mask,
        } => {
            let mut qm = question_mask;
            while qm != 0 {
                let i = qm.trailing_zeros() as usize;
                qm &= qm - 1;
                if let Some(a) = state.answers[i]
                    && (option_mask >> a.idx()) & 1 == 1
                {
                    return Some(i);
                }
            }
            None
        }
    }
}

/// Apply a `DeduceAction` to `state`: `Force` collapses the cell to the answer,
/// `Eliminate`/`EliminateMulti` set the eliminated bits. Shared by `run_engine`
/// and `lookahead`.
pub(crate) fn apply_action(action: &DeduceAction, state: &mut State) {
    match *action {
        DeduceAction::Force { qi, answer } => {
            state.eliminated[qi] = ALL_OPTIONS_MASK ^ (1 << answer.idx());
            state.answers[qi] = Some(answer);
        }
        DeduceAction::Eliminate { qi, oi } => {
            state.eliminated[qi] |= 1 << oi;
        }
        DeduceAction::EliminateMulti {
            question_mask,
            option_mask,
        } => {
            let mut qm = question_mask;
            while qm != 0 {
                let i = qm.trailing_zeros() as usize;
                qm &= qm - 1;
                state.eliminated[i] |= option_mask;
            }
        }
    }
}

// ── Helpers ──

#[inline(always)]
fn is_elim(eliminated: &[u8; MAX_N], qi: usize, oi: usize) -> bool {
    (eliminated[qi] >> oi) & 1 == 1
}

#[inline(always)]
fn remaining_count(eliminated: u8) -> u32 {
    (!eliminated & ALL_OPTIONS_MASK).count_ones()
}

#[derive(Clone, Copy)]
struct CountResult {
    count: u8,
    guaranteed: u8,
    possible: u8,
}

impl CountResult {
    fn min(&self) -> u8 {
        self.count + self.guaranteed
    }
    fn max(&self) -> u8 {
        self.count + self.guaranteed + self.possible
    }
}

/// Return Some(i) if exactly one i in the range satisfies the predicate; None otherwise.
/// Used by rules that fire only when a single candidate remains.
fn exactly_one(
    range: impl IntoIterator<Item = usize>,
    mut f: impl FnMut(usize) -> bool,
) -> Option<usize> {
    let mut found = None;
    for i in range {
        if f(i) {
            if found.is_some() {
                return None;
            }
            found = Some(i);
        }
    }
    found
}

/// Compute-once, read-many cache. The closure is called on the first `get()`
/// and the result memoised for subsequent calls. Useful when a value is
/// derived from the input state and would otherwise be recomputed redundantly
/// across match arms — but cheap to skip entirely when no arm needs it.
struct Lazy<T, F> {
    cache: Option<T>,
    init: F,
}

impl<T: Copy, F: Fn() -> T> Lazy<T, F> {
    fn new(init: F) -> Self {
        Self { cache: None, init }
    }

    fn get(&mut self) -> T {
        if let Some(v) = self.cache {
            return v;
        }
        let v = (self.init)();
        self.cache = Some(v);
        v
    }
}

/// Whole-puzzle per-letter cell accounting. Pure function of answers +
/// eliminations: `filled[i]` = questions answered with letter i; `fillable[i]`
/// = unanswered questions where letter i is not yet eliminated. The cell-based
/// count of letter i therefore lies in `[filled[i], filled[i] + fillable[i]]`.
///
/// These are *cell* facts. Any rule that hypothesises "what if this one cell
/// were letter X" — the extremum `±1`, OnlySame's per-cell subtract — must read
/// these, never the abstract `CountBounds`: an external bound may already
/// account for the very cell being adjusted, so adding to it would double-count.
#[derive(Clone, Copy)]
struct LetterCells {
    filled: [u8; 5],
    fillable: [u8; 5],
}

impl LetterCells {
    /// Cell-based upper bound on count(i): placed + still-possible slots.
    #[inline(always)]
    fn cell_max(&self, i: usize) -> u8 {
        self.filled[i] + self.fillable[i]
    }
}

fn compute_letter_cells(
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    n: usize,
) -> LetterCells {
    let mut filled = [0u8; 5];
    let mut fillable = [0u8; 5];
    for j in 0..n {
        if let Some(a) = answers[j] {
            filled[a.idx()] += 1;
        } else {
            for li in 0..5usize {
                if !is_elim(eliminated, j, li) {
                    fillable[li] += 1;
                }
            }
        }
    }
    LetterCells { filled, fillable }
}

/// Whole-puzzle per-letter bounds derived purely from sibling Count questions
/// (`CountAnswer` / `CountAnswerBefore` / `CountAnswerAfter`), independent of
/// answered cells. `floor[i]` is a lower bound on the total count of letter i;
/// `ceil[i]` an upper bound.
///
/// Floors come from all three count kinds — a sub-range floor still
/// lower-bounds the total. Ceilings come from full-range `CountAnswer` only: a
/// `Before`/`After` ceiling bounds a sub-range and says nothing about the rest
/// of the puzzle. Consumers combine these with `LetterCells` via `lower`/`upper`
/// — never feed them into a per-cell `±1`, which would double-count.
#[derive(Clone, Copy)]
struct CountBounds {
    floor: [u8; 5],
    ceil: [u8; 5],
}

impl CountBounds {
    /// Combined lower bound on count(i): the tighter of placed cells and the
    /// Count-question floor.
    #[inline(always)]
    fn lower(&self, cells: &LetterCells, i: usize) -> u8 {
        cells.filled[i].max(self.floor[i])
    }

    /// Combined upper bound on count(i): the tighter of the cell ceiling and
    /// the Count-question ceiling.
    #[inline(always)]
    fn upper(&self, cells: &LetterCells, i: usize) -> u8 {
        cells.cell_max(i).min(self.ceil[i])
    }
}

fn compute_count_bounds(
    fp: &FlatPuzzle,
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    n: usize,
) -> CountBounds {
    let oc = fp.option_count;
    let mut floor = [0u8; 5];
    let mut ceil = [n as u8; 5];
    for k in 0..n {
        let (li, full_range) = match fp.question_types[k] {
            QuestionType::CountAnswer { answer } => (answer.idx(), true),
            QuestionType::CountAnswerBefore { answer, .. }
            | QuestionType::CountAnswerAfter { answer, .. } => (answer.idx(), false),
            _ => continue,
        };
        // Range of surviving option values: if k is answered only that option
        // survives; else every non-eliminated numeric option.
        let mut lo = u8::MAX;
        let mut hi = 0u8;
        for oi in 0..oc {
            if let Some(a) = answers[k] {
                if oi != a.idx() {
                    continue;
                }
            } else if is_elim(eliminated, k, oi) {
                continue;
            }
            let v = fp.options[k][oi];
            if v.is_num() {
                lo = lo.min(v.value());
                hi = hi.max(v.value());
            }
        }
        if lo == u8::MAX {
            continue; // no surviving numeric option
        }
        floor[li] = floor[li].max(lo);
        if full_range {
            ceil[li] = ceil[li].min(hi);
        }
    }
    CountBounds { floor, ceil }
}

pub type DeduceResults = ArrayVec<DeduceResult, 80>;

#[derive(Clone, Copy)]
enum RuleFilter {
    All,
    #[cfg(test)]
    Only(DeduceRule),
    #[cfg(test)]
    Except(DeduceRule),
}

impl RuleFilter {
    #[inline(always)]
    fn matches(self, #[cfg_attr(not(test), allow(unused_variables))] r: DeduceRule) -> bool {
        match self {
            RuleFilter::All => true,
            #[cfg(test)]
            RuleFilter::Only(o) => r == o,
            #[cfg(test)]
            RuleFilter::Except(e) => r != e,
        }
    }
}

const VOWEL_MASK: u8 = 0b10001;
const CONSONANT_MASK: u8 = 0b01110;

#[inline(always)]
fn mask_contains(mask: u8, oi: usize) -> bool {
    (mask >> oi) & 1 != 0
}

/// Compute (count, guaranteed, possible) for a mask-selected predicate over [from, to).
fn count_matching_mask(
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    mask: u8,
    from: usize,
    to: usize,
) -> CountResult {
    let non_mask = !mask & ALL_OPTIONS_MASK;
    let mut count: u8 = 0;
    let mut guaranteed: u8 = 0;
    let mut possible: u8 = 0;
    for i in from..to {
        if let Some(a) = answers[i] {
            if mask_contains(mask, a.idx()) {
                count += 1;
            }
        } else {
            let remaining_bits = !eliminated[i] & ALL_OPTIONS_MASK;
            let matching = remaining_bits & mask;
            if matching == 0 {
                continue;
            }
            if remaining_bits & non_mask == 0 {
                guaranteed += 1;
            } else {
                possible += 1;
            }
        }
    }
    CountResult {
        count,
        guaranteed,
        possible,
    }
}

/// Per-qi count-family dispatch (CountAnswer/Before/After/Vowel/Consonant).
/// Handles the answered case (CountSaturated / CountMustMatch{Force,Elim}) and
/// the unanswered case (CountAllAnswered + per-option CountExceeded/Impossible).
fn apply_count(
    fp: &FlatPuzzle,
    state: &State,
    mut push: impl FnMut(DeduceRule, DeduceAction),
    qi: usize,
    mask: u8,
    from: usize,
    to: usize,
    include_slow: bool,
) {
    let answers = &state.answers;
    let eliminated = &state.eliminated;
    let cr = count_matching_mask(answers, eliminated, mask, from, to);

    if let Some(a) = answers[qi] {
        // Answered count qi: CountSaturated / CountMustMatch{Force,Elim}.
        let ov = fp.options[qi][a.idx()];
        if !ov.is_num() {
            return;
        }
        let ov = ov.value();

        if cr.min() == ov && cr.possible > 0 {
            for j in from..to {
                if answers[j].is_none() {
                    let remaining_bits = !eliminated[j] & ALL_OPTIONS_MASK;
                    if remaining_bits & (!mask & ALL_OPTIONS_MASK) == 0 {
                        continue;
                    }
                    let option_mask = remaining_bits & mask;
                    if option_mask != 0 {
                        push(
                            DeduceRule::CountSaturated,
                            DeduceAction::EliminateMulti {
                                question_mask: 1 << j,
                                option_mask,
                            },
                        );
                    }
                }
            }
        }

        if cr.max() == ov && cr.possible > 0 {
            if cr.possible == 1 {
                for j in from..to {
                    if answers[j].is_none()
                        && eliminated[j] & mask != mask
                        && let Some(oi) = exactly_one(0..5, |oi| {
                            !is_elim(eliminated, j, oi) && mask_contains(mask, oi)
                        })
                    {
                        push(
                            DeduceRule::CountMustMatchForce,
                            DeduceAction::Force {
                                qi: j,
                                answer: Answer::from(oi as u8),
                            },
                        );
                    }
                }
            }

            for j in from..to {
                if answers[j].is_none() && eliminated[j] & mask != mask {
                    let option_mask = !eliminated[j] & !mask & ALL_OPTIONS_MASK;
                    if option_mask != 0 {
                        push(
                            DeduceRule::CountMustMatchElim,
                            DeduceAction::EliminateMulti {
                                question_mask: 1 << j,
                                option_mask,
                            },
                        );
                    }
                }
            }
        }
    } else {
        // Unanswered count qi: CountAllAnswered + per-option CountExceeded/Impossible.
        if include_slow && cr.possible == 0 {
            let target_val = OptionValue::num(cr.min());
            if let Some(oi) = exactly_one(0..fp.option_count, |oi| {
                !is_elim(eliminated, qi, oi) && fp.options[qi][oi] == target_val
            }) {
                push(
                    DeduceRule::CountAllAnswered,
                    DeduceAction::Force {
                        qi,
                        answer: Answer::from(oi as u8),
                    },
                );
            }
        }

        let mut exceeded_mask = 0u8;
        let mut impossible_mask = 0u8;
        for oi in 0..5usize {
            if is_elim(eliminated, qi, oi) {
                continue;
            }
            let ov = fp.options[qi][oi];
            if !ov.is_num() {
                // NONE/UNUSED on a count option is meaningless: any
                // claim that count == null is impossible.
                if ov.is_none() {
                    exceeded_mask |= 1 << oi;
                }
                continue;
            }
            let ov = ov.value();
            if cr.min() > ov {
                exceeded_mask |= 1 << oi;
            } else if cr.max() < ov {
                impossible_mask |= 1 << oi;
            }
        }
        if exceeded_mask != 0 {
            push(
                DeduceRule::CountExceeded,
                DeduceAction::EliminateMulti {
                    question_mask: 1 << qi,
                    option_mask: exceeded_mask,
                },
            );
        }
        if impossible_mask != 0 {
            push(
                DeduceRule::CountImpossible,
                DeduceAction::EliminateMulti {
                    question_mask: 1 << qi,
                    option_mask: impossible_mask,
                },
            );
        }
    }
}

/// Per-qi OnlyOdd/OnlyEven dispatch (qi must be unanswered). `parity` is
/// 1 for OnlyOdd (1-indexed odd = 0-indexed even positions), 0 for OnlyEven.
fn apply_only_odd_even(
    fp: &FlatPuzzle,
    state: &State,
    mut push: impl FnMut(DeduceRule, DeduceAction),
    qi: usize,
    answer: Answer,
    parity: usize,
    include_slow: bool,
) {
    let n = fp.n;
    let answers = &state.answers;
    let eliminated = &state.eliminated;

    for oi in 0..5usize {
        if is_elim(eliminated, qi, oi) {
            continue;
        }
        let ov = fp.options[qi][oi];
        if ov.is_num() {
            let pos = usize::from(ov.value());
            if (pos + 1) % 2 != parity {
                push(
                    DeduceRule::OnlyOddEvenWrongParity,
                    DeduceAction::Eliminate { qi, oi },
                );
            }
            if (pos + 1) % 2 == parity && pos < n {
                if let Some(pa) = answers[pos]
                    && pa != answer
                {
                    push(
                        DeduceRule::OnlyOddEvenWrongAnswer,
                        DeduceAction::Eliminate { qi, oi },
                    );
                }
                if answers[pos].is_none() && is_elim(eliminated, pos, answer.idx()) {
                    push(
                        DeduceRule::OnlyOddEvenRuledOut,
                        DeduceAction::Eliminate { qi, oi },
                    );
                }
            }
        } else if ov.is_none()
            && (0..n).any(|i| (i + 1) % 2 == parity && answers[i] == Some(answer))
        {
            push(
                DeduceRule::OnlyOddEvenNoneMatch,
                DeduceAction::Eliminate { qi, oi },
            );
        }
    }

    // OnlyOddEvenRangeElim: positions with the right parity
    // that aren't reachable from this OnlyOdd/Even's remaining options
    // can't hold `answer`.
    if include_slow {
        let answer_oi = answer.idx();
        let mut claimed = 0u16;
        for oi in 0..5usize {
            if is_elim(eliminated, qi, oi) {
                continue;
            }
            let ov = fp.options[qi][oi];
            if ov.is_num() {
                let v = usize::from(ov.value());
                if v < n {
                    claimed |= 1 << v;
                }
            }
        }
        let mut q_mask = 0u16;
        for j in 0..n {
            if j == qi {
                continue;
            }
            if (j + 1) % 2 != parity {
                continue;
            }
            if answers[j].is_some() {
                continue;
            }
            if (claimed >> j) & 1 == 1 {
                continue;
            }
            if !is_elim(eliminated, j, answer_oi) {
                q_mask |= 1 << j;
            }
        }
        if q_mask != 0 {
            push(
                DeduceRule::OnlyOddEvenRangeElim,
                DeduceAction::EliminateMulti {
                    question_mask: q_mask,
                    option_mask: 1 << answer_oi,
                },
            );
        }
    }
}

/// Forward positional dispatch (FirstWith / ClosestAfter).
/// `scan_start` = 0 for FirstWith, `after_index + 1` for ClosestAfter.
fn apply_positional_forward(
    fp: &FlatPuzzle,
    state: &State,
    mut push: impl FnMut(DeduceRule, DeduceAction),
    qi: usize,
    answer: Answer,
    scan_start: usize,
) {
    let n = fp.n;
    let answers = &state.answers;
    let eliminated = &state.eliminated;
    let ans = answers[qi];

    if let Some(a) = ans {
        // PositionalRangeAnswered: positions before the claimed target can't have `answer`.
        let ov = fp.options[qi][a.idx()];
        if ov.is_num() {
            let v = usize::from(ov.value());
            let letter_oi = answer.idx();
            let mut q_mask = 0u16;
            for j in scan_start..v {
                if answers[j].is_some() {
                    continue;
                }
                if !is_elim(eliminated, j, letter_oi) {
                    q_mask |= 1 << j;
                }
            }
            if q_mask != 0 {
                push(
                    DeduceRule::PositionalRangeAnswered,
                    DeduceAction::EliminateMulti {
                        question_mask: q_mask,
                        option_mask: 1 << letter_oi,
                    },
                );
            }
        }
    } else {
        // Per-option elim.
        for oi in 0..5usize {
            if is_elim(eliminated, qi, oi) {
                continue;
            }
            let ov = fp.options[qi][oi];
            if ov.is_num() {
                let pos = usize::from(ov.value());
                if pos < scan_start || pos >= n {
                    push(
                        DeduceRule::FirstClosestAfterOutOfRange,
                        DeduceAction::Eliminate { qi, oi },
                    );
                }
                if pos >= scan_start && pos < n {
                    if let Some(pa) = answers[pos]
                        && pa != answer
                    {
                        push(
                            DeduceRule::FirstClosestAfterWrongAnswer,
                            DeduceAction::Eliminate { qi, oi },
                        );
                    }
                    if answers[pos].is_none() && is_elim(eliminated, pos, answer.idx()) {
                        push(
                            DeduceRule::FirstClosestAfterRuledOut,
                            DeduceAction::Eliminate { qi, oi },
                        );
                    }
                    if (scan_start..pos).any(|j| answers[j] == Some(answer)) {
                        push(
                            DeduceRule::FirstClosestAfterEarlierMatch,
                            DeduceAction::Eliminate { qi, oi },
                        );
                    }
                    if oi == answer.idx() && qi >= scan_start && qi < pos {
                        push(
                            DeduceRule::FirstClosestAfterSelfRef,
                            DeduceAction::Eliminate { qi, oi },
                        );
                    }
                }
            } else if ov.is_none() && (scan_start..n).any(|j| answers[j] == Some(answer)) {
                push(
                    DeduceRule::FirstClosestAfterNoneMatch,
                    DeduceAction::Eliminate { qi, oi },
                );
            }
        }
        // PositionalRangeUnanswered: positions before the minimum remaining claim can't have `answer`.
        let mut min_pos = n;
        for oi in 0..5usize {
            if is_elim(eliminated, qi, oi) {
                continue;
            }
            let ov = fp.options[qi][oi];
            if ov.is_num() {
                let v = usize::from(ov.value());
                if v < min_pos {
                    min_pos = v;
                }
            }
        }
        let letter_oi = answer.idx();
        let mut q_mask = 0u16;
        for j in scan_start..min_pos {
            if answers[j].is_some() {
                continue;
            }
            if !is_elim(eliminated, j, letter_oi) {
                q_mask |= 1 << j;
            }
        }
        if q_mask != 0 {
            push(
                DeduceRule::PositionalRangeUnanswered,
                DeduceAction::EliminateMulti {
                    question_mask: q_mask,
                    option_mask: 1 << letter_oi,
                },
            );
        }
    }
}

/// Claim↔question link. A TrueStmt claim whose proposition is *global* is the same
/// proposition as a real question of type `link_type` answered to `link_val`, so
/// the claim cell `(qi, oi)` and that question's option `(k, ok)` relate. `link_val`
/// is the value `k` must hold for the claim true: the claim's own value for a
/// same-kind link, or `n − V` for the vowel/consonant complement.
///
/// Two of the four directions are sound regardless of uniqueness and always run:
/// a *false* claim can't be the selected one (`k` ruled out of `ok` ⇒ eliminate the
/// claim), and the *selected* claim must be true (`qi` answered `oi` ⇒ force `k=ok`).
/// The other two assume the TrueStmt's true claim is *unique* — true ⇒ selected
/// (`k=ok` ⇒ force the claim), and not-selected ⇒ false (claim eliminated ⇒ rule
/// `ok` out of `k`). Those are gated behind `assume_unique`: sound for play/check,
/// but NOT during generation, where that very uniqueness is what's being validated
/// (assuming it would "solve" genuinely ambiguous TrueStmts). `#[inline]` so the
/// per-arm calls fold back in (no call overhead, source stays DRY).
#[inline]
fn link_claim_question(
    fp: &FlatPuzzle,
    state: &State,
    push: &mut impl FnMut(DeduceRule, DeduceAction),
    qi: usize,
    oi: usize,
    n: usize,
    link_type: QuestionType,
    link_val: OptionValue,
    assume_unique: bool,
) {
    let answers = &state.answers;
    let eliminated = &state.eliminated;
    let self_elim = is_elim(eliminated, qi, oi);

    let Some(k) = (0..n).find(|&k| k != qi && fp.question_types[k] == link_type) else {
        return; // no question carries this proposition — nothing to link against
    };
    // `k`'s option holding the linked value. None → `k` can never take it, and a
    // valid puzzle always lists a question's true value, so the claim is false (and
    // a false claim can't be selected — sound regardless of uniqueness).
    let Some(ok) = (0..fp.option_count).find(|&ok| fp.options[k][ok] == link_val) else {
        if !self_elim {
            push(
                DeduceRule::TrueStatementMatchElim,
                DeduceAction::Eliminate { qi, oi },
            );
        }
        return;
    };
    let self_answered = answers[qi] == Some(Answer::from(oi as u8));
    let k_answered_ok = answers[k] == Some(Answer::from(ok as u8));
    let k_ruled_out = answers[k].is_some() || is_elim(eliminated, k, ok);

    // question → claim
    if k_answered_ok {
        // true claim ⇒ selected — only under uniqueness (another claim could also
        // be true and selected instead).
        if assume_unique && answers[qi].is_none() && !self_elim {
            push(
                DeduceRule::TrueStatementMatchForce,
                DeduceAction::Force {
                    qi,
                    answer: Answer::from(oi as u8),
                },
            );
        }
    } else if k_ruled_out && !self_elim {
        // false claim can't be the selected one — sound.
        push(
            DeduceRule::TrueStatementMatchElim,
            DeduceAction::Eliminate { qi, oi },
        );
    }

    // claim → question
    if answers[k].is_none() && !is_elim(eliminated, k, ok) {
        if self_answered {
            // selected claim must be true — sound.
            push(
                DeduceRule::TrueStatementMatchForce,
                DeduceAction::Force {
                    qi: k,
                    answer: Answer::from(ok as u8),
                },
            );
        } else if self_elim && assume_unique {
            // not-selected ⇒ false — only under uniqueness.
            push(
                DeduceRule::TrueStatementMatchElim,
                DeduceAction::Eliminate { qi: k, oi: ok },
            );
        }
    }
}

/// TrueStmt dispatch, keyed on each claim's `question_type`. Per claim: a uniform
/// `ClaimInvalid` (cell-semantic falsity), then a type match for the SelfRef
/// contradiction and the same-proposition link(s) — same-kind for any *global*
/// claim (host-relative types mean something different at the TrueStmt than at a
/// like-typed question, so they don't link), plus the `n − V` complement for
/// vowel/consonant counts. After the per-claim pass: `Forward` from the selected
/// claim (qi answered), else the aggregate `ClaimValid` / (unique-only)
/// `ClaimKnownTrue` (qi unanswered).
fn apply_true_stmt(
    fp: &FlatPuzzle,
    state: &State,
    mut push: impl FnMut(DeduceRule, DeduceAction),
    qi: usize,
    n: usize,
    assume_unique: bool,
) {
    let answers = &state.answers;
    let eliminated = &state.eliminated;
    let ans = answers[qi];

    for oi in 0..fp.option_count {
        let Some(claim) = fp.claim_at(qi, oi) else {
            unreachable!("Claim at ({qi},{oi}) should exist")
        };
        let open = !is_elim(eliminated, qi, oi);
        let v = claim.value;

        // Cell-semantic falsity — uniform across all claim types.
        if open
            && crate::check_answer::check_claim(fp, *state, OptionPos { qi, oi }, claim)
                == crate::check_answer::Validity::Invalid
        {
            push(
                DeduceRule::TrueStatementClaimInvalid,
                DeduceAction::Eliminate { qi, oi },
            );
        }

        // Type-specific: SelfRef contradiction (positional/AnswerOf referencing qi)
        // and the same-proposition link(s). Non-global types don't link (`_`).
        match claim.question_type {
            QuestionType::FirstWith { answer } | QuestionType::LastWith { answer } => {
                if open
                    && v.is_num()
                    && usize::from(v.value()) == qi
                    && answer != Answer::from(oi as u8)
                {
                    push(
                        DeduceRule::TrueStatementSelfRef,
                        DeduceAction::Eliminate { qi, oi },
                    );
                }
                link_claim_question(
                    fp,
                    state,
                    &mut push,
                    qi,
                    oi,
                    n,
                    claim.question_type,
                    v,
                    assume_unique,
                );
            }
            QuestionType::AnswerOf { question_index } => {
                if open
                    && question_index as usize == qi
                    && v.is_num()
                    && v.value() <= 4
                    && Answer::from(v.value()) != Answer::from(oi as u8)
                {
                    push(
                        DeduceRule::TrueStatementSelfRef,
                        DeduceAction::Eliminate { qi, oi },
                    );
                }
                link_claim_question(
                    fp,
                    state,
                    &mut push,
                    qi,
                    oi,
                    n,
                    claim.question_type,
                    v,
                    assume_unique,
                );
            }
            // Vowel/consonant counts also link to the opposite kind at `n − V`.
            QuestionType::CountVowel | QuestionType::CountConsonant => {
                link_claim_question(
                    fp,
                    state,
                    &mut push,
                    qi,
                    oi,
                    n,
                    claim.question_type,
                    v,
                    assume_unique,
                );
                if v.is_num() && v.value() <= n as u8 {
                    let opposite = if matches!(claim.question_type, QuestionType::CountVowel) {
                        QuestionType::CountConsonant
                    } else {
                        QuestionType::CountVowel
                    };
                    let comp = OptionValue::num(n as u8 - v.value());
                    link_claim_question(
                        fp,
                        state,
                        &mut push,
                        qi,
                        oi,
                        n,
                        opposite,
                        comp,
                        assume_unique,
                    );
                }
            }
            QuestionType::CountAnswer { .. }
            | QuestionType::CountAnswerBefore { .. }
            | QuestionType::CountAnswerAfter { .. }
            | QuestionType::MostCommonCount
            | QuestionType::ClosestAfter { .. }
            | QuestionType::ClosestBefore { .. }
            | QuestionType::ConsecIdent
            | QuestionType::LeastCommon
            | QuestionType::MostCommon => {
                link_claim_question(
                    fp,
                    state,
                    &mut push,
                    qi,
                    oi,
                    n,
                    claim.question_type,
                    v,
                    assume_unique,
                );
            }
            _ => {}
        }
    }

    if let Some(a) = ans {
        // Forward (qi answered): a true-statement's claim, if compatible with
        // current state, forces the referenced target.
        if let Some(claim) = fp.claim_at(qi, a.idx()) {
            match claim.question_type {
                QuestionType::FirstWith { answer } | QuestionType::LastWith { answer }
                    if claim.value.is_num() =>
                {
                    let tqi = usize::from(claim.value.value());
                    if tqi < n && answers[tqi].is_none() && !is_elim(eliminated, tqi, answer.idx())
                    {
                        push(
                            DeduceRule::TrueStatementForward,
                            DeduceAction::Force { qi: tqi, answer },
                        );
                    }
                }
                QuestionType::AnswerOf { question_index } => {
                    let tqi = question_index as usize;
                    if claim.value.is_num()
                        && claim.value.value() <= 4
                        && tqi < n
                        && answers[tqi].is_none()
                    {
                        let letter = Answer::from(claim.value.value());
                        if !is_elim(eliminated, tqi, letter.idx()) {
                            push(
                                DeduceRule::TrueStatementForward,
                                DeduceAction::Force {
                                    qi: tqi,
                                    answer: letter,
                                },
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    } else {
        // ClaimValid + ClaimKnownTrue (qi unanswered).
        if let Some(oi) = exactly_one(0..5, |oi| {
            if is_elim(eliminated, qi, oi) {
                return false;
            }
            let Some(claim) = fp.claim_at(qi, oi) else {
                return false;
            };
            let mut hyp = *state;
            hyp.answers[qi] = Some(Answer::from(oi as u8));
            hyp.eliminated[qi] = ALL_OPTIONS_MASK ^ (1 << oi);
            crate::check_answer::check_claim(fp, hyp, OptionPos { qi, oi }, claim)
                != crate::check_answer::Validity::Invalid
        }) {
            push(
                DeduceRule::TrueStatementClaimValid,
                DeduceAction::Force {
                    qi,
                    answer: Answer::from(oi as u8),
                },
            );
        }

        // Uniqueness-assuming: gated so it never fires during generation.
        if assume_unique
            && let Some(oi) = exactly_one(0..5, |oi| {
                if is_elim(eliminated, qi, oi) {
                    return false;
                }
                let Some(claim) = fp.claim_at(qi, oi) else {
                    return false;
                };
                crate::check_answer::check_claim(fp, *state, OptionPos { qi, oi }, claim)
                    == crate::check_answer::Validity::Valid
            })
        {
            push(
                DeduceRule::TrueStatementClaimKnownTrue,
                DeduceAction::Force {
                    qi,
                    answer: Answer::from(oi as u8),
                },
            );
        }
    }
}

/// Backward positional dispatch (LastWith / ClosestBefore).
/// `scan_end` = n for LastWith, `before_index` for ClosestBefore.
fn apply_positional_backward(
    fp: &FlatPuzzle,
    state: &State,
    mut push: impl FnMut(DeduceRule, DeduceAction),
    qi: usize,
    answer: Answer,
    scan_end: usize,
) {
    let answers = &state.answers;
    let eliminated = &state.eliminated;
    let ans = answers[qi];

    if let Some(a) = ans {
        // PositionalRangeAnswered: positions after the claimed target can't have `answer`.
        let ov = fp.options[qi][a.idx()];
        if ov.is_num() {
            let v = usize::from(ov.value());
            let letter_oi = answer.idx();
            let mut q_mask = 0u16;
            for j in (v + 1)..scan_end {
                if answers[j].is_some() {
                    continue;
                }
                if !is_elim(eliminated, j, letter_oi) {
                    q_mask |= 1 << j;
                }
            }
            if q_mask != 0 {
                push(
                    DeduceRule::PositionalRangeAnswered,
                    DeduceAction::EliminateMulti {
                        question_mask: q_mask,
                        option_mask: 1 << letter_oi,
                    },
                );
            }
        }
    } else {
        // Per-option elim.
        for oi in 0..5usize {
            if is_elim(eliminated, qi, oi) {
                continue;
            }
            let ov = fp.options[qi][oi];
            if ov.is_num() {
                let pos = usize::from(ov.value());
                if pos >= scan_end {
                    push(
                        DeduceRule::LastClosestBeforeOutOfRange,
                        DeduceAction::Eliminate { qi, oi },
                    );
                }
                if pos < scan_end {
                    if let Some(pa) = answers[pos]
                        && pa != answer
                    {
                        push(
                            DeduceRule::LastClosestBeforeWrongAnswer,
                            DeduceAction::Eliminate { qi, oi },
                        );
                    }
                    if answers[pos].is_none() && is_elim(eliminated, pos, answer.idx()) {
                        push(
                            DeduceRule::LastClosestBeforeRuledOut,
                            DeduceAction::Eliminate { qi, oi },
                        );
                    }
                    if ((pos + 1)..scan_end).any(|j| answers[j] == Some(answer)) {
                        push(
                            DeduceRule::LastClosestBeforeLaterMatch,
                            DeduceAction::Eliminate { qi, oi },
                        );
                    }
                    if oi == answer.idx() && qi > pos && qi < scan_end {
                        push(
                            DeduceRule::LastClosestBeforeSelfRef,
                            DeduceAction::Eliminate { qi, oi },
                        );
                    }
                }
            } else if ov.is_none() && (0..scan_end).any(|j| answers[j] == Some(answer)) {
                push(
                    DeduceRule::LastClosestBeforeNoneMatch,
                    DeduceAction::Eliminate { qi, oi },
                );
            }
        }
        // PositionalRangeUnanswered: positions after the maximum remaining claim can't have `answer`.
        let mut max_pos: Option<usize> = None;
        for oi in 0..5usize {
            if is_elim(eliminated, qi, oi) {
                continue;
            }
            let ov = fp.options[qi][oi];
            if ov.is_num() {
                let v = usize::from(ov.value());
                if max_pos.is_none_or(|m| v > m) {
                    max_pos = Some(v);
                }
            }
        }
        let letter_oi = answer.idx();
        let mut q_mask = 0u16;
        let scan_start = max_pos.map_or(0, |p| p + 1);
        for j in scan_start..scan_end {
            if answers[j].is_some() {
                continue;
            }
            if !is_elim(eliminated, j, letter_oi) {
                q_mask |= 1 << j;
            }
        }
        if q_mask != 0 {
            push(
                DeduceRule::PositionalRangeUnanswered,
                DeduceAction::EliminateMulti {
                    question_mask: q_mask,
                    option_mask: 1 << letter_oi,
                },
            );
        }
    }
}

/// Rules shared by `SameAs` and `OnlySame` arms: reverse force, NoneForward
/// (answered qi), and the common per-option elims (NoneMatch / SelfRef /
/// RuledOut) for unanswered qi. Only the reverse force is renamed per arm via
/// `reverse_rule` (SameAsReverse vs PrevNextOnlySameReverse); the others keep
/// their `OnlySame*` names for both arms (so a SameAs trace shows e.g.
/// `OnlySameNoneForward`). Renaming them would also touch the explain prose
/// keyed on those names and the deduce test fixtures.
fn apply_same_shared(
    fp: &FlatPuzzle,
    state: &State,
    mut push: impl FnMut(DeduceRule, DeduceAction),
    qi: usize,
    reverse_rule: DeduceRule,
    include_slow: bool,
) {
    let n = fp.n;
    let answers = &state.answers;
    let eliminated = &state.eliminated;
    let ans = answers[qi];

    if let Some(a) = ans {
        let ov = fp.options[qi][a.idx()];
        // Reverse: qi answered with an index → force that target qi to qi's letter.
        if ov.is_num() {
            let target_qi = usize::from(ov.value());
            if target_qi < n && answers[target_qi].is_none() {
                push(
                    reverse_rule,
                    DeduceAction::Force {
                        qi: target_qi,
                        answer: a,
                    },
                );
            }
        }

        // OnlySameNoneForward: an answered None means qi's answer is unique,
        // so no other question can have that letter. Sound; not gated on
        // assume_unique.
        if include_slow && ov.is_none() {
            for j in 0..n {
                if j == qi {
                    continue;
                }
                if answers[j].is_none() && !is_elim(eliminated, j, a.idx()) {
                    push(
                        DeduceRule::OnlySameNoneForward,
                        DeduceAction::Eliminate { qi: j, oi: a.idx() },
                    );
                }
            }
        }
    } else {
        // Per-option elim (qi unanswered): rules shared by both arms.
        for oi in 0..5usize {
            if is_elim(eliminated, qi, oi) {
                continue;
            }
            let ov = fp.options[qi][oi];
            if ov.is_none() {
                if (0..n).any(|j| j != qi && answers[j] == Some(Answer::from(oi as u8))) {
                    push(
                        DeduceRule::OnlySameNoneMatch,
                        DeduceAction::Eliminate { qi, oi },
                    );
                }
            } else if ov.is_num() {
                let pos = usize::from(ov.value());
                if pos == qi {
                    push(
                        DeduceRule::OnlySameSelfRef,
                        DeduceAction::Eliminate { qi, oi },
                    );
                }
                if pos < n && is_elim(eliminated, pos, oi) {
                    push(
                        DeduceRule::OnlySameRuledOut,
                        DeduceAction::Eliminate { qi, oi },
                    );
                }
            }
        }
    }
}

/// PrevSame / NextSame dispatch. Reverse force (when answered) into the
/// referenced position, PositionalRangeAnswered over the open interval between
/// qi and the target, plus per-option elims for unanswered qi.
///
/// `range` is the candidate range (`0..qi` for PrevSame, `(qi+1)..n` for
/// NextSame). `between(x)` is the open interval between qi and x — i.e.
/// `(x+1)..qi` for PrevSame, `(qi+1)..x` for NextSame. Direction lives in the
/// closure, baked in by monomorphization; no runtime branch.
fn apply_prev_or_next_same(
    fp: &FlatPuzzle,
    state: &State,
    mut push: impl FnMut(DeduceRule, DeduceAction),
    qi: usize,
    range: std::ops::Range<usize>,
    between: impl Fn(usize) -> std::ops::Range<usize>,
    rules: (DeduceRule, DeduceRule, DeduceRule, DeduceRule),
) {
    let n = fp.n;
    let answers = &state.answers;
    let eliminated = &state.eliminated;
    let (none_rule, out_rule, ruled_out_rule, closer_rule) = rules;

    if let Some(a) = answers[qi] {
        let ov = fp.options[qi][a.idx()];
        if ov.is_num() {
            let target_qi = usize::from(ov.value());
            if target_qi < n && answers[target_qi].is_none() {
                push(
                    DeduceRule::PrevNextOnlySameReverse,
                    DeduceAction::Force {
                        qi: target_qi,
                        answer: a,
                    },
                );
            }
            // PositionalRangeAnswered: positions strictly between qi and target
            // can't hold qi's letter.
            let letter_oi = a.idx();
            let mut q_mask = 0u16;
            for j in between(target_qi) {
                if answers[j].is_some() {
                    continue;
                }
                if !is_elim(eliminated, j, letter_oi) {
                    q_mask |= 1 << j;
                }
            }
            if q_mask != 0 {
                push(
                    DeduceRule::PositionalRangeAnswered,
                    DeduceAction::EliminateMulti {
                        question_mask: q_mask,
                        option_mask: 1 << letter_oi,
                    },
                );
            }
        }
    } else {
        // Per-option elim (qi unanswered).
        for oi in 0..5usize {
            if is_elim(eliminated, qi, oi) {
                continue;
            }
            let ov = fp.options[qi][oi];
            if ov.is_none() {
                if range
                    .clone()
                    .any(|j| answers[j] == Some(Answer::from(oi as u8)))
                {
                    push(none_rule, DeduceAction::Eliminate { qi, oi });
                }
            } else if ov.is_num() {
                let pos = usize::from(ov.value());
                if !range.contains(&pos) {
                    push(out_rule, DeduceAction::Eliminate { qi, oi });
                }
                if range.contains(&pos) {
                    if is_elim(eliminated, pos, oi) {
                        push(ruled_out_rule, DeduceAction::Eliminate { qi, oi });
                    }
                    if between(pos).any(|j| answers[j] == Some(Answer::from(oi as u8))) {
                        push(closer_rule, DeduceAction::Eliminate { qi, oi });
                    }
                }
            }
        }
    }
}

/// LeastCommon / MostCommon dispatch. Per-option, check whether `oi`'s claimed
/// count "could be" the extremum (others' max within reach) and "must be" the
/// extremum (others' min strictly past). Emit Elim for ¬could, Force when
/// exactly one option could-be AND it must-be.
///
/// The Least/Most asymmetry reduces to swapping the (subject, other) pair:
/// for Least, `oi`'s claimed letter is the "other" that other letters bound;
/// for Most, it's the "subject" being bounded by others' min.
fn apply_extremum_count<const IS_LEAST: bool>(
    fp: &FlatPuzzle,
    state: &State,
    mut push: impl FnMut(DeduceRule, DeduceAction),
    qi: usize,
    cells: &LetterCells,
    elim_rule: DeduceRule,
    force_rule: DeduceRule,
) {
    let eliminated = &state.eliminated;
    let oc = fp.option_count;

    // qi is unanswered; remove its contribution to the cell ceiling so adj_*
    // doesn't double-count when we test "if qi were `oi`".
    let min_count = cells.filled;
    let mut max_count = cells.filled;
    for li in 0..oc {
        max_count[li] += cells.fillable[li];
        if !is_elim(eliminated, qi, li) {
            max_count[li] -= 1;
        }
    }

    // Single pass: emit Elims AND track which options could-be / must-be
    // the extremum, as bitmasks.
    let mut can_mask = 0u8;
    let mut must_mask = 0u8;
    for oi in 0..oc {
        if is_elim(eliminated, qi, oi) {
            continue;
        }
        let ov = fp.options[qi][oi];
        // Skip NONE/UNUSED sentinels and any out-of-range letter claim.
        if !ov.is_num() || usize::from(ov.value()) >= oc {
            continue;
        }
        let claimed = usize::from(ov.value());

        let mut adj_min = min_count;
        let mut adj_max = max_count;
        adj_min[oi] += 1;
        adj_max[oi] += 1;

        // `pair(li)` returns (a, b) such that the comparison reduces to
        // `adj_max[a] ≥ adj_min[b]` (for could-be) and `adj_min[a] > adj_max[b]`
        // (for must-be) regardless of Least vs Most. `IS_LEAST` is a const
        // generic so the compiler monomorphizes the branch away at each call site.
        let pair = |li: usize| -> (usize, usize) {
            if IS_LEAST {
                (li, claimed)
            } else {
                (claimed, li)
            }
        };

        let can_be_extreme = (0..oc).all(|li| {
            if li == claimed {
                return true;
            }
            let (a, b) = pair(li);
            adj_max[a] >= adj_min[b]
        });
        let must_be_extreme = (0..oc).all(|li| {
            if li == claimed {
                return true;
            }
            let (a, b) = pair(li);
            adj_min[a] > adj_max[b]
        });

        if can_be_extreme {
            can_mask |= 1 << oi;
        }
        if must_be_extreme {
            must_mask |= 1 << oi;
        }
        if !can_be_extreme {
            push(elim_rule, DeduceAction::Eliminate { qi, oi });
        }
    }

    // Force when exactly one option could-be the extremum AND it must-be.
    if can_mask.count_ones() == 1 {
        let oi = can_mask.trailing_zeros() as usize;
        if must_mask & (1 << oi) != 0 {
            push(
                force_rule,
                DeduceAction::Force {
                    qi,
                    answer: Answer::from(oi as u8),
                },
            );
        }
    }
}

/// Vowel + Consonant = n cross-elim. Fires once per deduce call from the
/// canonical CountVowel arm. The two CountVowel/CountConsonant sides are
/// structurally symmetric; the local closures capture that.
fn apply_vowel_consonant_cross_elim(
    fp: &FlatPuzzle,
    state: &State,
    mut push: impl FnMut(DeduceRule, DeduceAction),
    vq: usize,
    cq: usize,
    n: usize,
) {
    let eliminated = &state.eliminated;

    // NONE counts as "valid" (still a candidate option) but can't partner —
    // leaving it in `valid` without a partner is what triggers the cross-elim.
    let valid_mask = |q: usize| -> u8 {
        let mut mask = 0u8;
        for oi in 0..5 {
            if !is_elim(eliminated, q, oi) && !fp.options[q][oi].is_unused() {
                mask |= 1 << oi;
            }
        }
        mask
    };
    let vowel_valid = valid_mask(vq);
    let consonant_valid = valid_mask(cq);

    // 5×5 cross-product: find (voi, coi) pairs whose option values sum to n.
    // Iterate only set bits of `*_valid` via trailing_zeros + clear-lowest-bit.
    let nn = n as u8;
    let mut vowel_has_partner = 0u8;
    let mut consonant_has_partner = 0u8;
    let mut v_iter = vowel_valid;
    while v_iter != 0 {
        let voi = v_iter.trailing_zeros() as usize;
        v_iter &= v_iter - 1;
        let vs = fp.options[vq][voi];
        if !vs.is_num() {
            continue;
        }
        let v = vs.value();
        let mut c_iter = consonant_valid;
        while c_iter != 0 {
            let coi = c_iter.trailing_zeros() as usize;
            c_iter &= c_iter - 1;
            let cs = fp.options[cq][coi];
            if !cs.is_num() {
                continue;
            }
            if v + cs.value() == nn {
                vowel_has_partner |= 1 << voi;
                consonant_has_partner |= 1 << coi;
            }
        }
    }

    // Emit eliminations for valid options that found no partner.
    let mut emit_unpaired = |q: usize, valid: u8, has_partner: u8, rule: DeduceRule| {
        let mut unpaired = valid & !has_partner;
        while unpaired != 0 {
            let oi = unpaired.trailing_zeros() as usize;
            unpaired &= unpaired - 1;
            push(rule, DeduceAction::Eliminate { qi: q, oi });
        }
    };
    emit_unpaired(
        vq,
        vowel_valid,
        vowel_has_partner,
        DeduceRule::VowelCrossElim,
    );
    emit_unpaired(
        cq,
        consonant_valid,
        consonant_has_partner,
        DeduceRule::ConsonantCrossElim,
    );
}

// ── Main functions ──

/// Sound-only deduction. Safe to use during generation: every conclusion is
/// true in any valid extension of the current state, regardless of whether the
/// puzzle has a unique solution.
pub fn deduce(fp: &FlatPuzzle, state: &State) -> DeduceResults {
    deduce_impl(fp, state, RuleFilter::All, true, false, None)
}

/// Single-question probe: the new deductions `qi`'s own rules produce against
/// the current state, in one pass (same rule set as [`deduce`], scoped to qi).
///
/// Intended as a cheap repair gate — `O(qi's rules)` instead of `O(all rules)`.
/// It can miss an edit whose payoff lands on a *different* question (a global
/// rule elsewhere that reads qi's options), but that only costs a skipped
/// repair, never soundness: the accepting path still runs the full engine +
/// brute-force uniqueness check. Used as repair's per-question gate (see
/// `construct::repair`).
pub fn deduce_question(fp: &FlatPuzzle, state: &State, qi: usize) -> DeduceResults {
    deduce_impl(fp, state, RuleFilter::All, true, false, Some(qi))
}

/// Deduction that may apply uniqueness-assuming rules (e.g. "TrueStmt has
/// exactly one true claim, so the known-true one must be it"). Only sound
/// when the puzzle is known to have a unique solution — use for play, check,
/// or tests; NOT during generation.
pub fn deduce_assuming_unique(fp: &FlatPuzzle, state: &State) -> DeduceResults {
    deduce_impl(fp, state, RuleFilter::All, true, true, None)
}

/// Fast-path variant of `deduce`: skips expensive non-fast rules. Sound-only
/// (does NOT apply uniqueness-assuming rules); used by lookahead's
/// hypothesis-testing where the hypothesis may be inconsistent.
pub fn deduce_fast(fp: &FlatPuzzle, state: &State) -> DeduceResults {
    deduce_impl(fp, state, RuleFilter::All, false, false, None)
}

#[cfg(test)]
pub fn deduce_with_rule(fp: &FlatPuzzle, state: &State, rule: DeduceRule) -> DeduceResults {
    deduce_impl(fp, state, RuleFilter::Only(rule), true, true, None)
}

#[cfg(test)]
pub fn deduce_with_rule_except(
    fp: &FlatPuzzle,
    state: &State,
    exclude: DeduceRule,
) -> DeduceResults {
    deduce_impl(fp, state, RuleFilter::Except(exclude), true, true, None)
}

/// Shared implementation behind `deduce` / `deduce_assuming_unique` / `deduce_fast`
/// and the test variants: scans each question (or just `question_scope`) and emits
/// the deductions its rules license. `filter`, `include_slow`, and `assume_unique`
/// are the knobs the public wrappers pin to fixed presets.
///
/// Inlined per caller on native so the arg constants fold and dead match arms get
/// DCE'd; left outlined on wasm, where each specialization would bloat the download.
#[cfg_attr(not(target_arch = "wasm32"), inline(always))]
fn deduce_impl(
    fp: &FlatPuzzle,
    state: &State,
    filter: RuleFilter,
    include_slow: bool,
    assume_unique: bool,
    // `Some(qi)` restricts the per-qi dispatch to a single question — a scoped
    // probe for repair. `None` (every play/solve caller) is a compile-time
    // constant under the per-caller inlining, so the skip below folds away and
    // the full path is byte-for-byte unchanged.
    question_scope: Option<usize>,
) -> DeduceResults {
    let n = fp.n;
    let answers = &state.answers;
    let eliminated = &state.eliminated;
    let mut results = DeduceResults::new();
    let mut push = |rule: DeduceRule, action: DeduceAction| {
        if filter.matches(rule) {
            results.push(DeduceResult { action, rule });
        }
    };

    // Canonical CountVowel/CountConsonant pair (last unanswered of each type).
    // Used by the CountVowel arm below for vowel+consonant = n cross-elim, which
    // fires exactly once per deduce call regardless of how many of each type
    // exist in the puzzle.
    let mut vowel_qi: Option<usize> = None;
    let mut consonant_qi: Option<usize> = None;
    for qi in 0..n {
        match fp.question_types[qi] {
            QuestionType::CountVowel if answers[qi].is_none() => vowel_qi = Some(qi),
            QuestionType::CountConsonant if answers[qi].is_none() => consonant_qi = Some(qi),
            _ => {}
        }
    }

    let mut letter_cells = Lazy::new(|| compute_letter_cells(answers, eliminated, n));
    let mut count_bounds = Lazy::new(|| compute_count_bounds(fp, answers, eliminated, n));

    // ── Per-qi dispatch ──
    // For each qi, dispatch on its type. Each arm owns all rules whose source
    // is qi (regardless of qi's answered state). The type-agnostic
    // OnlyOptionLeft fires at the end of each iteration.
    for qi in 0..n {
        if question_scope.is_some_and(|only| only != qi) {
            continue;
        }
        let t = &fp.question_types[qi];
        let ans = answers[qi];

        match *t {
            QuestionType::CountAnswer { answer } => {
                apply_count(
                    fp,
                    state,
                    &mut push,
                    qi,
                    1 << answer.idx(),
                    0,
                    n,
                    include_slow,
                );
            }
            QuestionType::CountAnswerBefore {
                answer,
                before_index,
            } => {
                apply_count(
                    fp,
                    state,
                    &mut push,
                    qi,
                    1 << answer.idx(),
                    0,
                    before_index as usize,
                    include_slow,
                );
            }
            QuestionType::CountAnswerAfter {
                answer,
                after_index,
            } => {
                apply_count(
                    fp,
                    state,
                    &mut push,
                    qi,
                    1 << answer.idx(),
                    after_index as usize + 1,
                    n,
                    include_slow,
                );
            }
            QuestionType::CountConsonant => {
                apply_count(fp, state, &mut push, qi, CONSONANT_MASK, 0, n, include_slow);
            }
            QuestionType::MostCommonCount if ans.is_none() => {
                let mut max_known: u8 = 0;
                let mut max_possible: u8 = 0;
                for li in 0..fp.option_count {
                    let cr = count_matching_mask(answers, eliminated, 1 << li, 0, n);
                    if cr.min() > max_known {
                        max_known = cr.min();
                    }
                    if cr.max() > max_possible {
                        max_possible = cr.max();
                    }
                }
                for oi in 0..5usize {
                    if is_elim(eliminated, qi, oi) {
                        continue;
                    }
                    let ov = fp.options[qi][oi];
                    if !ov.is_num() {
                        continue;
                    }
                    let ov = ov.value();
                    if ov < max_known || ov > max_possible {
                        push(
                            DeduceRule::MostCommonCountElim,
                            DeduceAction::Eliminate { qi, oi },
                        );
                    }
                }
            }
            QuestionType::CountVowel => {
                apply_count(fp, state, &mut push, qi, VOWEL_MASK, 0, n, include_slow);
                if include_slow
                    && Some(qi) == vowel_qi
                    && let Some(cq) = consonant_qi
                {
                    apply_vowel_consonant_cross_elim(fp, state, &mut push, qi, cq, n);
                }
            }
            QuestionType::AnswerOf { question_index } => {
                let target_qi = question_index as usize;
                if let Some(a) = ans {
                    // Reverse: qi answered → force the target qi.
                    let ov = fp.options[qi][a.idx()];
                    if ov.is_num()
                        && ov.value() <= 4
                        && target_qi < n
                        && answers[target_qi].is_none()
                    {
                        push(
                            DeduceRule::AnswerOfReverse,
                            DeduceAction::Force {
                                qi: target_qi,
                                answer: Answer::from(ov.value()),
                            },
                        );
                    }
                } else {
                    // Forward + per-option elim (qi unanswered).
                    let target_ans = answers[target_qi];
                    if let Some(target) = target_ans {
                        let mut best: Option<usize> = None;
                        for oi in 0..fp.option_count {
                            let ov = fp.options[qi][oi];
                            if ov.is_num() && ov.value() == target as u8 {
                                if !is_elim(eliminated, qi, oi) {
                                    best = Some(oi);
                                    break;
                                }
                                if best.is_none() {
                                    best = Some(oi);
                                }
                            }
                        }
                        if let Some(oi) = best {
                            push(
                                DeduceRule::AnswerOfForward,
                                DeduceAction::Force {
                                    qi,
                                    answer: Answer::from(oi as u8),
                                },
                            );
                        }
                    }
                    for oi in 0..5usize {
                        if is_elim(eliminated, qi, oi) {
                            continue;
                        }
                        let ov = fp.options[qi][oi];
                        if !ov.is_num() {
                            continue;
                        }
                        let ov = ov.value();
                        if ov <= 4 {
                            if let Some(target) = target_ans {
                                if target as u8 != ov {
                                    push(
                                        DeduceRule::AnswerOfTargetRuledOut,
                                        DeduceAction::Eliminate { qi, oi },
                                    );
                                }
                            } else if is_elim(eliminated, target_qi, ov as usize) {
                                push(
                                    DeduceRule::AnswerOfTargetRuledOut,
                                    DeduceAction::Eliminate { qi, oi },
                                );
                            }
                        }
                    }
                }
            }
            QuestionType::LetterDist { question_index } => {
                let target_qi = question_index as usize;
                if let Some(a) = ans {
                    // Reverse (src answered): narrow target's options to those at the claimed distance.
                    if target_qi < n && target_qi != qi && answers[target_qi].is_none() {
                        let ov = fp.options[qi][a.idx()];
                        if !ov.is_unused() {
                            // NONE distance is unsatisfiable: every non-eliminated option
                            // ends up in elim_mask (the `actual == ov.value()` check is
                            // skipped when the source's distance value is null).
                            let mut elim_mask = 0u8;
                            let mut valid_count = 0u8;
                            let mut valid_oi = 0usize;
                            for oi in 0..5usize {
                                if is_elim(eliminated, target_qi, oi) {
                                    continue;
                                }
                                let actual = (oi as u8).abs_diff(a as u8);
                                if ov.is_num() && actual == ov.value() {
                                    valid_count += 1;
                                    valid_oi = oi;
                                } else {
                                    elim_mask |= 1 << oi;
                                }
                            }
                            if valid_count == 1 && elim_mask != 0 {
                                push(
                                    DeduceRule::LetterDistReverseForce,
                                    DeduceAction::Force {
                                        qi: target_qi,
                                        answer: Answer::from(valid_oi as u8),
                                    },
                                );
                            }
                            if elim_mask != 0 && valid_count != 1 {
                                push(
                                    DeduceRule::LetterDistReverseElim,
                                    DeduceAction::EliminateMulti {
                                        question_mask: 1 << target_qi,
                                        option_mask: elim_mask,
                                    },
                                );
                            }
                        }
                    }
                } else {
                    // Forward + per-option elim (qi unanswered).
                    let target_ans = answers[target_qi];
                    if let Some(other_ans) = target_ans {
                        let other_idx = other_ans as u8;
                        if let Some(oi) = exactly_one(0..5, |oi| {
                            let ov = fp.options[qi][oi];
                            !is_elim(eliminated, qi, oi)
                                && ov.is_num()
                                && (oi as u8).abs_diff(other_idx) == ov.value()
                        }) {
                            push(
                                DeduceRule::LetterDistForward,
                                DeduceAction::Force {
                                    qi,
                                    answer: Answer::from(oi as u8),
                                },
                            );
                        }
                    }
                    for oi in 0..5usize {
                        if is_elim(eliminated, qi, oi) {
                            continue;
                        }
                        let ov = fp.options[qi][oi];
                        let max_dist = oi.max(4 - oi) as u8;
                        if ov.is_num() && ov.value() > max_dist {
                            push(
                                DeduceRule::LetterDistImpossible,
                                DeduceAction::Eliminate { qi, oi },
                            );
                        }
                        if let Some(other) = target_ans {
                            // ov is NONE/UNUSED: dist (always ≥ 0) can never equal it.
                            // ov is num: literal distance comparison.
                            let dist = (oi as u8).abs_diff(other as u8);
                            let matches = ov.is_num() && dist == ov.value();
                            if !matches {
                                push(
                                    DeduceRule::LetterDistWrong,
                                    DeduceAction::Eliminate { qi, oi },
                                );
                            }
                        }
                        if ov.is_num() && target_ans.is_none() && ov.value() <= max_dist {
                            let ov = ov.value();
                            let no_match = !(0..5usize).any(|ti| {
                                !is_elim(eliminated, target_qi, ti)
                                    && (oi as u8).abs_diff(ti as u8) == ov
                            });
                            if no_match {
                                push(
                                    DeduceRule::LetterDistNoMatch,
                                    DeduceAction::Eliminate { qi, oi },
                                );
                            }
                        }
                    }

                    // Reverse (src unanswered): narrow target by what's compatible from src's remaining options.
                    if target_qi < n && target_qi != qi && target_ans.is_none() {
                        let mut elim_mask = 0u8;
                        for oi in 0..5usize {
                            if is_elim(eliminated, target_qi, oi) {
                                continue;
                            }
                            let compatible = (0..5usize).any(|si| {
                                let ov = fp.options[qi][si];
                                !is_elim(eliminated, qi, si)
                                    && ov.is_num()
                                    && (oi as u8).abs_diff(si as u8) == ov.value()
                            });
                            if !compatible {
                                elim_mask |= 1 << oi;
                            }
                        }
                        if elim_mask != 0 {
                            push(
                                DeduceRule::LetterDistReverseElim,
                                DeduceAction::EliminateMulti {
                                    question_mask: 1 << target_qi,
                                    option_mask: elim_mask,
                                },
                            );
                        }
                    }
                }
            }
            QuestionType::FirstWith { answer } => {
                apply_positional_forward(fp, state, &mut push, qi, answer, 0);
            }
            QuestionType::ClosestAfter {
                answer,
                after_index,
            } => {
                apply_positional_forward(
                    fp,
                    state,
                    &mut push,
                    qi,
                    answer,
                    after_index as usize + 1,
                );
            }
            QuestionType::LastWith { answer } => {
                apply_positional_backward(fp, state, &mut push, qi, answer, n);
            }
            QuestionType::ClosestBefore {
                answer,
                before_index,
            } => {
                apply_positional_backward(fp, state, &mut push, qi, answer, before_index as usize);
            }
            QuestionType::OnlyOdd { answer } if ans.is_none() => {
                apply_only_odd_even(fp, state, &mut push, qi, answer, 1, include_slow);
            }
            QuestionType::OnlyEven { answer } if ans.is_none() => {
                apply_only_odd_even(fp, state, &mut push, qi, answer, 0, include_slow);
            }
            QuestionType::ConsecIdent => {
                // Reverse: any qi state. Eliminate matching neighbors at positions
                // that this ConsecIdent's remaining options can't claim.
                let mut possible_pairs = 0u16;
                for oi in 0..5usize {
                    if is_elim(eliminated, qi, oi) {
                        continue;
                    }
                    let ov = fp.options[qi][oi];
                    if !ov.is_num() {
                        continue;
                    }
                    let pos = usize::from(ov.value());
                    if pos + 1 < n {
                        possible_pairs |= 1 << pos;
                    }
                }
                // Iterate only impossible-pair positions (bits cleared in
                // `possible_pairs`), masked to the valid j range `0..n-1`.
                let mut impossible =
                    !possible_pairs & ((1u16 << n.saturating_sub(1)).saturating_sub(1));
                while impossible != 0 {
                    let j = impossible.trailing_zeros() as usize;
                    impossible &= impossible - 1;
                    let aj = answers[j];
                    let aj1 = answers[j + 1];
                    if let Some(ja) = aj
                        && aj1.is_none()
                        && !is_elim(eliminated, j + 1, ja.idx())
                    {
                        push(
                            DeduceRule::ConsecIdentReverse,
                            DeduceAction::Eliminate {
                                qi: j + 1,
                                oi: ja.idx(),
                            },
                        );
                    }
                    if let Some(jb) = aj1
                        && aj.is_none()
                        && !is_elim(eliminated, j, jb.idx())
                    {
                        push(
                            DeduceRule::ConsecIdentReverse,
                            DeduceAction::Eliminate {
                                qi: j,
                                oi: jb.idx(),
                            },
                        );
                    }
                }

                if let Some(a) = ans {
                    // Forward force/elim/both (qi answered).
                    if include_slow {
                        let ov = fp.options[qi][a.idx()];
                        if ov.is_num() && usize::from(ov.value()) + 1 < n {
                            let p = usize::from(ov.value());
                            let poss_a = !eliminated[p] & ALL_OPTIONS_MASK;
                            let poss_b = !eliminated[p + 1] & ALL_OPTIONS_MASK;
                            let ans_a = answers[p];
                            let ans_b = answers[p + 1];

                            if let Some(letter) = ans_a
                                && ans_b.is_none()
                                && !is_elim(eliminated, p + 1, letter.idx())
                            {
                                push(
                                    DeduceRule::ConsecIdentForwardForce,
                                    DeduceAction::Force {
                                        qi: p + 1,
                                        answer: letter,
                                    },
                                );
                            }
                            if let Some(letter) = ans_b
                                && ans_a.is_none()
                                && !is_elim(eliminated, p, letter.idx())
                            {
                                push(
                                    DeduceRule::ConsecIdentForwardForce,
                                    DeduceAction::Force {
                                        qi: p,
                                        answer: letter,
                                    },
                                );
                            }

                            // Options at p that are remaining for p but impossible at p+1
                            // (and vice versa) can't be in a consec-identical pair → eliminate.
                            if ans_a.is_none() {
                                let mut to_elim = poss_a & !poss_b & ALL_OPTIONS_MASK;
                                while to_elim != 0 {
                                    let oi = to_elim.trailing_zeros() as usize;
                                    to_elim &= to_elim - 1;
                                    push(
                                        DeduceRule::ConsecIdentForwardElim,
                                        DeduceAction::Eliminate { qi: p, oi },
                                    );
                                }
                            }
                            if ans_b.is_none() {
                                let mut to_elim = poss_b & !poss_a & ALL_OPTIONS_MASK;
                                while to_elim != 0 {
                                    let oi = to_elim.trailing_zeros() as usize;
                                    to_elim &= to_elim - 1;
                                    push(
                                        DeduceRule::ConsecIdentForwardElim,
                                        DeduceAction::Eliminate { qi: p + 1, oi },
                                    );
                                }
                            }

                            if ans_a.is_none() && ans_b.is_none() {
                                let common = poss_a & poss_b;
                                if common.count_ones() == 1 {
                                    let oi = common.trailing_zeros() as usize;
                                    push(
                                        DeduceRule::ConsecIdentForwardBothForce,
                                        DeduceAction::Force {
                                            qi: p,
                                            answer: Answer::from(oi as u8),
                                        },
                                    );
                                    push(
                                        DeduceRule::ConsecIdentForwardBothForce,
                                        DeduceAction::Force {
                                            qi: p + 1,
                                            answer: Answer::from(oi as u8),
                                        },
                                    );
                                }
                            }
                        }
                    }
                } else {
                    // Per-option elim (qi unanswered): OOR, NoCommon, SelfRef, NonePair.
                    for oi in 0..5usize {
                        if is_elim(eliminated, qi, oi) {
                            continue;
                        }
                        let ov = fp.options[qi][oi];
                        if ov.is_num() {
                            let pos = usize::from(ov.value());
                            if pos + 1 >= n {
                                push(
                                    DeduceRule::ConsecIdentOutOfRange,
                                    DeduceAction::Eliminate { qi, oi },
                                );
                            } else {
                                let common = (!eliminated[pos] & ALL_OPTIONS_MASK)
                                    & (!eliminated[pos + 1] & ALL_OPTIONS_MASK);
                                if common == 0 {
                                    push(
                                        DeduceRule::ConsecIdentNoCommon,
                                        DeduceAction::Eliminate { qi, oi },
                                    );
                                } else if pos == qi || pos + 1 == qi {
                                    let partner = if pos == qi { pos + 1 } else { pos };
                                    if is_elim(eliminated, partner, oi) {
                                        push(
                                            DeduceRule::ConsecIdentSelfRef,
                                            DeduceAction::Eliminate { qi, oi },
                                        );
                                    }
                                }
                            }
                        } else if ov.is_none()
                            && (0..n.saturating_sub(1)).any(|i| {
                                matches!(
                                    (answers[i], answers[i + 1]),
                                    (Some(a), Some(b)) if a == b
                                )
                            })
                        {
                            push(
                                DeduceRule::ConsecIdentNonePair,
                                DeduceAction::Eliminate { qi, oi },
                            );
                        }
                    }
                }
            }
            QuestionType::EqualCount { answer } if ans.is_none() => {
                for oi in 0..5usize {
                    if is_elim(eliminated, qi, oi) {
                        continue;
                    }
                    let ov = fp.options[qi][oi];
                    if !ov.is_num() {
                        continue;
                    }
                    let ov = usize::from(ov.value());
                    if ov == answer.idx() {
                        push(
                            DeduceRule::EqualCountSelfRef,
                            DeduceAction::Eliminate { qi, oi },
                        );
                    }
                    let claimed = Answer::from(ov as u8);
                    if claimed != answer {
                        // Impossible iff the upper bound on one letter's count
                        // is below the lower bound on the other's. Both bounds
                        // fold in sibling Count questions, so this also catches
                        // pairs separated by a CountAnswer floor/ceiling, not
                        // just by placed/eliminated cells.
                        let cells = letter_cells.get();
                        let bounds = count_bounds.get();
                        if bounds.upper(&cells, answer.idx()) < bounds.lower(&cells, claimed.idx())
                            || bounds.upper(&cells, claimed.idx())
                                < bounds.lower(&cells, answer.idx())
                        {
                            push(
                                DeduceRule::EqualCountRangeElim,
                                DeduceAction::Eliminate { qi, oi },
                            );
                        }
                    }
                }
            }
            QuestionType::PrevSame => {
                apply_prev_or_next_same(
                    fp,
                    state,
                    &mut push,
                    qi,
                    0..qi,
                    |x| (x + 1)..qi,
                    (
                        DeduceRule::PrevSameNoneMatch,
                        DeduceRule::PrevSameNotBefore,
                        DeduceRule::PrevSameRuledOut,
                        DeduceRule::PrevSameCloser,
                    ),
                );
            }
            QuestionType::NextSame => {
                apply_prev_or_next_same(
                    fp,
                    state,
                    &mut push,
                    qi,
                    (qi + 1)..n,
                    |x| (qi + 1)..x,
                    (
                        DeduceRule::NextSameNoneMatch,
                        DeduceRule::NextSameNotAfter,
                        DeduceRule::NextSameRuledOut,
                        DeduceRule::NextSameCloser,
                    ),
                );
            }
            QuestionType::SameAsWhich { question_index } => {
                let qi_ref = question_index as usize;
                let ref_ans = answers[qi_ref];
                if let Some(a) = ans {
                    // Reverse.
                    if include_slow {
                        let ov = fp.options[qi][a.idx()];
                        if ov.is_num() {
                            let j = usize::from(ov.value());
                            if j < n {
                                let j_ans = answers[j];
                                if let Some(ra) = ref_ans
                                    && j_ans.is_none()
                                    && !is_elim(eliminated, j, ra.idx())
                                {
                                    push(
                                        DeduceRule::SameAsWhichReverse,
                                        DeduceAction::Force { qi: j, answer: ra },
                                    );
                                }
                                if let Some(ja) = j_ans
                                    && ref_ans.is_none()
                                    && !is_elim(eliminated, qi_ref, ja.idx())
                                {
                                    push(
                                        DeduceRule::SameAsWhichReverse,
                                        DeduceAction::Force {
                                            qi: qi_ref,
                                            answer: ja,
                                        },
                                    );
                                }
                            }
                        }
                    }
                } else if let Some(ra) = ref_ans {
                    // Forward per-option elim (qi unanswered, target known).
                    for oi in 0..5usize {
                        if is_elim(eliminated, qi, oi) {
                            continue;
                        }
                        let ov = fp.options[qi][oi];
                        if !ov.is_num() {
                            continue;
                        }
                        let j = usize::from(ov.value());
                        if j < n && j != qi && j != qi_ref {
                            let wrong = match answers[j] {
                                Some(ja) => ja != ra,
                                None => is_elim(eliminated, j, ra.idx()),
                            };
                            if wrong {
                                push(
                                    DeduceRule::SameAsWhichForward,
                                    DeduceAction::Eliminate { qi, oi },
                                );
                            }
                        }
                    }
                }
            }
            QuestionType::SameAs => {
                apply_same_shared(
                    fp,
                    state,
                    &mut push,
                    qi,
                    DeduceRule::SameAsReverse,
                    include_slow,
                );

                // SameAs negative: non-selected option targets cannot share qi's
                // answer. Uniqueness-assuming, answered-qi only.
                if assume_unique && let Some(a) = ans {
                    let ai = a.idx();
                    let selected_s = fp.options[qi][ai];
                    // The "none" answer's sound inference is handled in
                    // apply_same_shared (OnlySameNoneForward); this rule is for
                    // the index case.
                    if selected_s.is_num() {
                        let selected = selected_s.value();
                        let mut q_mask = 0u16;
                        for oi in 0..fp.option_count {
                            if oi == ai {
                                continue;
                            }
                            let ts = fp.options[qi][oi];
                            if !ts.is_num() {
                                continue;
                            }
                            let target = usize::from(ts.value());
                            if target >= n || target == qi {
                                continue;
                            }
                            if ts.value() != selected
                                && answers[target].is_none()
                                && !is_elim(eliminated, target, ai)
                            {
                                q_mask |= 1 << target;
                            }
                        }
                        if q_mask != 0 {
                            push(
                                DeduceRule::SameAsNegative,
                                DeduceAction::EliminateMulti {
                                    question_mask: q_mask,
                                    option_mask: 1 << ai,
                                },
                            );
                        }
                    }
                }
            }
            QuestionType::OnlySame => {
                apply_same_shared(
                    fp,
                    state,
                    &mut push,
                    qi,
                    DeduceRule::PrevNextOnlySameReverse,
                    include_slow,
                );

                // OnlySameOtherMatch: per-option elim, OnlySame only. If pos is
                // pointing at a position where some OTHER qi (not pos) already
                // has letter `oi`, then qi can't be "only same as pos" via oi.
                if ans.is_none() {
                    for oi in 0..5usize {
                        if is_elim(eliminated, qi, oi) {
                            continue;
                        }
                        let ov = fp.options[qi][oi];
                        if !ov.is_num() {
                            continue;
                        }
                        let pos = usize::from(ov.value());
                        if pos >= n || pos == qi {
                            continue;
                        }
                        // qi is unanswered, so it doesn't contribute to letter_known.
                        // Subtract pos's contribution to check for any OTHER match.
                        let letter = Answer::from(oi as u8);
                        let cells = letter_cells.get();
                        let pos_contrib = u8::from(answers[pos] == Some(letter));
                        if cells.filled[oi] > pos_contrib {
                            push(
                                DeduceRule::OnlySameOtherMatch,
                                DeduceAction::Eliminate { qi, oi },
                            );
                        }
                    }
                }
            }
            QuestionType::LeastCommon if include_slow && ans.is_none() => {
                let cells = letter_cells.get();
                apply_extremum_count::<true>(
                    fp,
                    state,
                    &mut push,
                    qi,
                    &cells,
                    DeduceRule::LeastCommonElim,
                    DeduceRule::LeastCommonForce,
                );

                // Global pigeonhole: letter D can be the unique least-common
                // letter only if count(D) <= floor((n - oc + 1) / oc); beyond
                // that the other oc-1 letters can't all be strictly larger
                // within n answers. A proven lower bound above the threshold
                // rules D out — a whole-puzzle sum argument the pairwise
                // extremum check above structurally can't make.
                let oc = fp.option_count;
                if oc >= 2 && n + 1 >= oc {
                    let bounds = count_bounds.get();
                    let max_least = ((n + 1 - oc) / oc) as u8;
                    for oi in 0..oc {
                        if is_elim(eliminated, qi, oi) {
                            continue;
                        }
                        let v = fp.options[qi][oi];
                        if !v.is_num() {
                            continue;
                        }
                        let d = v.value() as usize;
                        if d < oc && bounds.lower(&cells, d) > max_least {
                            push(
                                DeduceRule::LeastCommonCountFloor,
                                DeduceAction::Eliminate { qi, oi },
                            );
                        }
                    }
                }
            }
            QuestionType::MostCommon if include_slow && ans.is_none() => {
                let cells = letter_cells.get();
                apply_extremum_count::<false>(
                    fp,
                    state,
                    &mut push,
                    qi,
                    &cells,
                    DeduceRule::MostCommonElim,
                    DeduceRule::MostCommonForce,
                );

                // Global pigeonhole (mirror of LeastCommonCountFloor): letter D
                // can be the unique most-common letter only if count(D) >=
                // ceil((n + oc - 1) / oc); below that the other oc-1 letters
                // can't all stay strictly smaller while summing to n. A proven
                // upper bound under the threshold rules D out.
                let oc = fp.option_count;
                if oc >= 2 {
                    let bounds = count_bounds.get();
                    let min_most = ((n + 2 * oc - 2) / oc) as u8;
                    for oi in 0..oc {
                        if is_elim(eliminated, qi, oi) {
                            continue;
                        }
                        let v = fp.options[qi][oi];
                        if !v.is_num() {
                            continue;
                        }
                        let d = v.value() as usize;
                        if d < oc && bounds.upper(&cells, d) < min_most {
                            push(
                                DeduceRule::MostCommonCountCeil,
                                DeduceAction::Eliminate { qi, oi },
                            );
                        }
                    }
                }
            }
            QuestionType::TrueStmt if include_slow => {
                apply_true_stmt(fp, state, &mut push, qi, n, assume_unique);
            }

            _ => {}
        }

        // OnlyOptionLeft is type-agnostic — fires when only one option remains.
        if ans.is_none() && remaining_count(eliminated[qi]) == 1 {
            let oi = (!eliminated[qi] & ALL_OPTIONS_MASK).trailing_zeros();
            push(
                DeduceRule::OnlyOptionLeft,
                DeduceAction::Force {
                    qi,
                    answer: Answer::from(oi as u8),
                },
            );
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn slow_test_duration() -> Option<std::time::Duration> {
        if std::env::var("REFPUZZLE_FAST_TESTS").is_ok() {
            return Some(std::time::Duration::from_millis(200));
        }
        if cfg!(debug_assertions) {
            panic!("slow test — run with --release or set REFPUZZLE_FAST_TESTS=1");
        }
        Some(std::time::Duration::from_secs(5))
    }

    /// Mirrors src/lib/playground.ts encoding for cross-runner-compatible links.
    fn playground_link(puzzle: &Value, states: &[Value], n: usize) -> String {
        use base64::Engine;
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use flate2::Compression;
        use flate2::write::DeflateEncoder;
        use std::io::Write;

        let puzzle_json = serde_json::to_string(puzzle).unwrap();
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(puzzle_json.as_bytes()).unwrap();
        let p = URL_SAFE_NO_PAD.encode(encoder.finish().unwrap());

        // History: one action per non-unmarked option, iterated in (qi, oi) order
        // to match savedStateFromMarks in playground.ts.
        let mut actions: Vec<String> = Vec::new();
        for qi in 0..n {
            let s = states.get(qi).and_then(|v| v.as_str()).unwrap_or("");
            let mut correct: Option<usize> = None;
            let mut incorrect = [false; 5];
            for ch in s.chars() {
                if ch.is_ascii_uppercase() {
                    correct = Some((ch as u8 - b'A') as usize);
                } else if ch.is_ascii_lowercase() {
                    incorrect[(ch as u8 - b'a') as usize] = true;
                }
            }
            for oi in 0..5 {
                let letter = (b'A' + oi as u8) as char;
                if Some(oi) == correct {
                    actions.push(format!("{}{}", qi + 1, letter));
                } else if incorrect[oi] {
                    actions.push(format!("{}{}", qi + 1, letter.to_ascii_lowercase()));
                }
            }
        }
        let h = actions.join(".");

        let base = std::env::var("PLAYGROUND_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:5173".to_string());
        if h.is_empty() {
            format!("{base}/playground#p={p}")
        } else {
            format!("{base}/playground#p={p}&h={h}")
        }
    }

    #[test]
    fn test_shared_deduce() {
        let json_str =
            std::fs::read_to_string("../tests/deduce.json").expect("can't read tests/deduce.json");
        let suite: Value = serde_json::from_str(&json_str).unwrap();
        let tests = suite["tests"].as_array().unwrap();

        let mut passed = 0;
        let mut failed = 0;
        let mut covered_rules = std::collections::HashSet::new();
        let mut dry_failed = 0;

        for test in tests {
            if test.get("section").is_some() {
                continue;
            }
            let name = test["name"].as_str().unwrap();
            let states = test["state"].as_array().unwrap();
            let expect = test["expect"].as_str();
            let rule_filter = test.get("rule").and_then(|v| v.as_str());

            let fp = crate::serialize::parse_puzzle(&test["puzzle"]);
            let fp = match fp {
                Some(fp) => fp,
                None => {
                    failed += 1;
                    eprintln!("FAIL: {name}: parse failed");
                    continue;
                }
            };

            let n = fp.n;
            let mut answers: [Option<Answer>; MAX_N] = [None; MAX_N];
            let mut eliminated = fp.initial_state.eliminated;
            for i in 0..n {
                let s = states[i].as_str().unwrap_or("");
                for ch in s.chars() {
                    if ch.is_ascii_uppercase() {
                        let oi = (ch as u8 - b'A') as usize;
                        answers[i] = Some(Answer::from(oi as u8));
                        eliminated[i] = ALL_OPTIONS_MASK ^ (1 << oi);
                    } else if ch.is_ascii_lowercase() {
                        let oi = (ch as u8 - b'a') as usize;
                        eliminated[i] |= 1 << oi;
                    }
                }
            }

            let parsed_rule = rule_filter.and_then(DeduceRule::from_str);
            if let Some(r) = parsed_rule {
                covered_rules.insert(r.to_str());
            }

            let state = State {
                answers,
                eliminated,
            };
            let drs = match parsed_rule {
                Some(r) => deduce_with_rule(&fp, &state, r),
                None => deduce_assuming_unique(&fp, &state),
            };
            fn format_result(dr: Option<&DeduceResult>) -> String {
                match dr {
                    Some(DeduceResult {
                        action: DeduceAction::Force { qi, answer },
                        ..
                    }) => format!("{}{}", qi + 1, answer.as_char()),
                    Some(DeduceResult {
                        action: DeduceAction::Eliminate { qi, oi },
                        ..
                    }) => format!("{}{}", qi + 1, (b'a' + *oi as u8) as char),
                    Some(DeduceResult {
                        action:
                            DeduceAction::EliminateMulti {
                                question_mask,
                                option_mask,
                            },
                        ..
                    }) => format!("qm{:b}o{:05b}", question_mask, option_mask),
                    None => "null".to_string(),
                }
            }

            let got = format_result(drs.first());
            let expected = expect.unwrap_or("null");

            if got == expected {
                passed += 1;
            } else {
                failed += 1;
                eprintln!("FAIL: {name}");
                eprintln!("  expected: {expected}");
                eprintln!("  got:      {got}");
                eprintln!(
                    "  link:     {}",
                    playground_link(&test["puzzle"], states, n)
                );
            }

            // DRY check
            if let Some(r) = parsed_rule
                && !drs.is_empty()
                && got == expected
            {
                let without = deduce_with_rule_except(&fp, &state, r);
                let without_got = format_result(without.first());
                if without_got == got {
                    dry_failed += 1;
                    eprintln!("DRY: {name}");
                    eprintln!("  excluding {} still produces: {got}", r.to_str());
                }
            }
        }

        // Coverage check
        let uncovered: Vec<_> = ALL_DEDUCE_RULES
            .iter()
            .filter(|r| !covered_rules.contains(r.to_str()))
            .collect();
        for r in &uncovered {
            eprintln!("MISSING TEST COVERAGE: {}", r.to_str());
        }

        eprintln!("{passed}/{} passed", passed + failed);
        if dry_failed > 0 {
            eprintln!("{dry_failed} DRY violation(s)");
        }
        assert_eq!(failed, 0, "{failed} test(s) failed");
        assert_eq!(
            uncovered.len(),
            0,
            "{} rule(s) without tests",
            uncovered.len()
        );
        assert_eq!(dry_failed, 0, "{dry_failed} DRY violation(s)");
    }

    #[test]
    fn test_deduce_soundness_fuzz() {
        use crate::build::fill_options;
        use crate::rng::Rng;
        use crate::solve_brute::solve;

        fn random_question_type(rng: &mut Rng, qi: usize, n: usize) -> QuestionType {
            match rng.int(0, 24) {
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
                _ => QuestionType::AnswerIsSelf,
            }
        }

        let Some(duration) = slow_test_duration() else {
            return;
        };
        let mut failures = 0;
        let mut puzzles_tested = 0;
        let deadline = std::time::Instant::now() + duration;

        for seed in 0u32.. {
            if seed % 100 == 0 && std::time::Instant::now() > deadline {
                break;
            }
            let mut rng = Rng::new(seed.wrapping_mul(7919).wrapping_add(42));
            let n = rng.int(4, 8) as usize;

            let solution: [Answer; MAX_N] =
                std::array::from_fn(|i| if i < n { rng.pick_letter(5) } else { Answer::A });

            let mut question_types = [QuestionType::AnswerIsSelf; MAX_N];
            for qi in 0..n {
                question_types[qi] = random_question_type(&mut rng, qi, n);
            }

            let fp = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                fill_options(&question_types, &solution, n, 5, &mut Rng::new(seed), false)
            }));
            let Ok(fp) = fp else { continue };

            let solutions = solve(&fp, None, 2);
            if solutions.len() != 1 {
                continue;
            }
            if (0..n).any(|i| solutions[0][i] != solution[i]) {
                eprintln!("CONSTRUCTION FAIL seed={seed}:");
                eprintln!(
                    "  construction: {:?}",
                    &solution[..n]
                        .iter()
                        .map(|a| a.as_char())
                        .collect::<Vec<_>>()
                );
                eprintln!(
                    "  brute:        {:?}",
                    &solutions[0][..n]
                        .iter()
                        .map(|a| a.as_char())
                        .collect::<Vec<_>>()
                );
                for qi in 0..n {
                    eprintln!(
                        "  Q{}: {:?} opts={:?}",
                        qi + 1,
                        fp.question_types[qi],
                        &fp.options[qi]
                    );
                }
                panic!("fill_options bug: brute solution != construction solution (seed={seed})");
            }

            puzzles_tested += 1;

            for state_seed in 0..20u32 {
                let mut rng = Rng::new(seed.wrapping_mul(1000).wrapping_add(state_seed));
                let mut answers: [Option<Answer>; MAX_N] = [None; MAX_N];
                let mut eliminated = fp.initial_state.eliminated;

                for qi in 0..n {
                    let r = rng.int(0, 4);
                    if r == 0 {
                        answers[qi] = Some(solution[qi]);
                        eliminated[qi] = ALL_OPTIONS_MASK ^ (1 << solution[qi].idx());
                    } else if r <= 2 {
                        let count = rng.int(1, 3) as usize;
                        for _ in 0..count {
                            let oi = rng.int(0, 4) as usize;
                            if Answer::from(oi as u8) != solution[qi] {
                                eliminated[qi] |= 1 << oi;
                            }
                        }
                    }
                }

                let drs = deduce(
                    &fp,
                    &State {
                        answers,
                        eliminated,
                    },
                );
                for dr in &drs {
                    let bad = match dr.action {
                        DeduceAction::Force { qi, answer } => answer != solution[qi],
                        DeduceAction::Eliminate { qi, oi } => oi == solution[qi].idx(),
                        DeduceAction::EliminateMulti {
                            question_mask,
                            option_mask,
                        } => (0..n).any(|qi| {
                            (question_mask >> qi) & 1 == 1
                                && (option_mask >> solution[qi].idx()) & 1 == 1
                        }),
                    };
                    if bad {
                        failures += 1;
                        if failures <= 3 {
                            eprintln!(
                                "SOUNDNESS FAIL seed={} state_seed={} rule={}: {:?}",
                                seed,
                                state_seed,
                                dr.rule.to_str(),
                                dr.action
                            );
                            eprintln!(
                                "  solution: {:?}",
                                &solution[..n]
                                    .iter()
                                    .map(|a| a.as_char())
                                    .collect::<Vec<_>>()
                            );
                            eprintln!("  answers:  {:?}", &answers[..n]);
                            eprintln!(
                                "  elim:     {:?}",
                                &eliminated[..n]
                                    .iter()
                                    .map(|e| format!("{:05b}", e))
                                    .collect::<Vec<_>>()
                            );
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
                }
            }
        }

        eprintln!("Fuzz: {puzzles_tested} puzzles tested, {failures} soundness failures");
        assert_eq!(failures, 0, "{failures} soundness failure(s)");
    }
}
