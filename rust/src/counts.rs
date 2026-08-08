//! Count/tally primitives shared by `deduce`, `check_answer`, `explain`, and the
//! generator: the counting predicate, the mask-selected tally, per-letter cell
//! accounting, whole-puzzle count-question bounds, and the answer-key tallies
//! generation reads. Lives here — not in `deduce` or `check_answer` — so every
//! consumer can use them without a module cycle (`deduce` already depends on
//! `check_answer`).

use crate::types::*;

/// What a count question counts: one answer letter, or a letter class.
#[derive(Clone, Copy)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum Pred {
    IsAnswer(Answer),
    IsVowel,
    IsConsonant,
}

impl Pred {
    pub(crate) fn matches(self, a: Answer) -> bool {
        match self {
            Pred::IsAnswer(t) => a == t,
            Pred::IsVowel => a.is_vowel(),
            Pred::IsConsonant => !a.is_vowel(),
        }
    }
    pub(crate) fn mask(self) -> u8 {
        match self {
            Pred::IsAnswer(t) => 1u8 << t.idx(),
            Pred::IsVowel => 0b10001,
            Pred::IsConsonant => 0b01110,
        }
    }
}

/// The counting predicate for a count-type question, or `None` for any other kind.
pub(crate) fn count_pred(qt: &QuestionType) -> Option<Pred> {
    match *qt {
        QuestionType::CountAnswer { answer }
        | QuestionType::CountAnswerBefore { answer, .. }
        | QuestionType::CountAnswerAfter { answer, .. } => Some(Pred::IsAnswer(answer)),
        QuestionType::CountVowel => Some(Pred::IsVowel),
        QuestionType::CountConsonant => Some(Pred::IsConsonant),
        _ => None,
    }
}

/// The question range a count kind scans: the whole board, or the sub-range a
/// Before/After kind names.
pub(crate) fn count_range(qt: &QuestionType, n: usize) -> (usize, usize) {
    match *qt {
        QuestionType::CountAnswerBefore { before_index, .. } => (0, before_index as usize),
        QuestionType::CountAnswerAfter { after_index, .. } => (after_index as usize + 1, n),
        _ => (0, n),
    }
}

/// Coarser counterpart to [`MaskTally`]: `count` answered matches plus `remaining`
/// unanswered questions that could still match, with the locked-in ones folded in
/// rather than split out.
pub(crate) struct CountResult {
    pub(crate) count: u8,
    pub(crate) remaining: u8,
}

pub(crate) fn count_matching(
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    pred: Pred,
    from: usize,
    to: usize,
) -> CountResult {
    let mask = pred.mask();
    let mut count: u8 = 0;
    let mut remaining: u8 = 0;
    for i in from..to {
        match answers[i] {
            Some(a) if pred.matches(a) => count += 1,
            None if eliminated[i] & mask != mask => remaining += 1,
            _ => {}
        }
    }
    CountResult { count, remaining }
}

/// Mask-selected tally over a range: `count` answered matches, `guaranteed`
/// unanswered questions *locked* to a match (only masked options remain), and
/// `possible` unanswered questions that could go either way. `min` (= count +
/// guaranteed) is the floor on the match count, `max` (+ possible) the ceiling.
#[derive(Clone, Copy)]
pub(crate) struct MaskTally {
    pub(crate) count: u8,
    pub(crate) guaranteed: u8,
    pub(crate) possible: u8,
}

impl MaskTally {
    pub(crate) fn min(&self) -> u8 {
        self.count + self.guaranteed
    }
    pub(crate) fn max(&self) -> u8 {
        self.count + self.guaranteed + self.possible
    }
}

#[inline(always)]
pub(crate) fn mask_contains(mask: u8, oi: usize) -> bool {
    (mask >> oi) & 1 != 0
}

/// Compute the [`MaskTally`] for a mask-selected predicate over `[from, to)`.
pub(crate) fn count_matching_mask(
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    mask: u8,
    from: usize,
    to: usize,
) -> MaskTally {
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
    MaskTally {
        count,
        guaranteed,
        possible,
    }
}

/// Whole-puzzle per-letter cell accounting. Pure function of answers +
/// eliminations: `filled[i]` = questions answered with letter i; `fillable[i]`
/// = unanswered questions where letter i is not yet eliminated. The cell-based
/// count of letter i therefore lies in `[filled[i], filled[i] + fillable[i]]`.
///
/// These are *cell* facts. Any rule that hypothesises "what if this one question
/// were letter X" — the extremum `±1`, OnlySame's per-question subtract — must read
/// these, never the abstract `CountBounds`: an external bound may already
/// account for the very question being adjusted, so adding to it would double-count.
#[derive(Clone, Copy)]
pub(crate) struct LetterCells {
    pub(crate) filled: [u8; 5],
    pub(crate) fillable: [u8; 5],
}

impl LetterCells {
    /// Cell-based upper bound on count(i): placed + still-possible slots.
    #[inline(always)]
    pub(crate) fn cell_max(&self, i: usize) -> u8 {
        self.filled[i] + self.fillable[i]
    }
}

pub(crate) fn compute_letter_cells(
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
                if !is_eliminated(eliminated, j, li) {
                    fillable[li] += 1;
                }
            }
        }
    }
    LetterCells { filled, fillable }
}

/// Whole-puzzle per-letter bounds derived purely from sibling Count questions
/// (`CountAnswer` / `CountAnswerBefore` / `CountAnswerAfter`), independent of
/// answered questions. `floor[i]` is a lower bound on the total count of letter i;
/// `ceil[i]` an upper bound.
///
/// Both bounds come from all three count kinds, but they arrive differently. A
/// sub-range floor lower-bounds the total as it stands. A sub-range *ceiling* has to
/// be widened first: it caps only its own range, so the total is capped at that plus
/// every question outside the range. Consumers combine these with `LetterCells` via
/// `lower`/`upper` — never feed them into a per-question `±1`, which would double-count.
///
/// Each bound also records the question that set it, so `explain` can name the
/// source instead of re-deriving it with a second copy of this scan.
#[derive(Clone, Copy)]
pub(crate) struct CountBounds {
    floor: [Bound; 5],
    ceil: [Bound; 5],
}

/// One directional bound on a letter's whole-board count. `value` is the bound itself,
/// and all `lower`/`upper` read.
///
/// `source` and `own_range_value` are auxiliary, for `explain` alone: the count question
/// that tightened the bound to where it stands, and the bound that question stated over
/// the range *it* counts. Only a question that actually tightens it is credited, so the
/// named source always still binds, and a bound left unrestricted names nobody — quoting
/// a vacuous one produces "there are at least 0 questions with answer B, so B appears too
/// often to be the least common".
///
/// The two differ only for a sub-range ceiling, which caps its own range and has to be
/// widened by the questions outside it before it caps the board.
#[derive(Clone, Copy)]
struct Bound {
    value: u8,
    own_range_value: u8,
    source: Option<u8>,
}

impl Bound {
    /// A bound that rules nothing out: 0 for a floor, `n` for a ceiling.
    fn unrestricted(value: u8) -> Self {
        Bound {
            value,
            own_range_value: value,
            source: None,
        }
    }

    fn at_least(&mut self, value: u8, question_index: usize) {
        if value > self.value {
            self.value = value;
            self.own_range_value = value;
            self.source = Some(question_index as u8);
        }
    }

    /// `own_range_value` caps the source's own range; `outside` is how many questions lie
    /// beyond it, so the two sum to the whole-board cap.
    fn at_most(&mut self, own_range_value: u8, outside: u8, question_index: usize) {
        let value = own_range_value + outside;
        if value < self.value {
            self.value = value;
            self.own_range_value = own_range_value;
            self.source = Some(question_index as u8);
        }
    }

    fn attributed(&self) -> Option<(usize, u8)> {
        self.source.map(|src| (usize::from(src), self.value))
    }
}

impl CountBounds {
    /// Combined lower bound on count(i): the tighter of placed cells and the
    /// Count-question floor.
    #[inline(always)]
    pub(crate) fn lower(&self, cells: &LetterCells, i: usize) -> u8 {
        cells.filled[i].max(self.floor[i].value)
    }

    /// Combined upper bound on count(i): the tighter of the cell ceiling and
    /// the Count-question ceiling.
    #[inline(always)]
    pub(crate) fn upper(&self, cells: &LetterCells, i: usize) -> u8 {
        cells.cell_max(i).min(self.ceil[i].value)
    }

    /// The count question imposing the floor on letter `i`, and that floor —
    /// `None` when no count question bounds the letter from below.
    pub(crate) fn floor_source(&self, i: usize) -> Option<(usize, u8)> {
        self.floor[i].attributed()
    }

    /// The count question imposing the ceiling on letter `i`, that whole-board ceiling,
    /// and the cap the question stated over the range it counts.
    pub(crate) fn ceil_source(&self, i: usize) -> Option<(usize, u8, u8)> {
        let bound = &self.ceil[i];
        bound
            .attributed()
            .map(|(question, value)| (question, value, bound.own_range_value))
    }
}

// The scan below reads every surviving option of a count question as a number. These
// kinds declare they can't be answered NONE, which is what makes `check_form` reject a
// NONE option on them — flip one of these and the scan starts reading NONE as a count.
const _: () = assert!(!QuestionTypeKind::CountAnswer.may_be_none());
const _: () = assert!(!QuestionTypeKind::CountAnswerBefore.may_be_none());
const _: () = assert!(!QuestionTypeKind::CountAnswerAfter.may_be_none());

/// Build the [`CountBounds`] in one pass: each count question's surviving option values
/// bound the letter it counts, tightest wins. A letter no count question mentions keeps
/// its unrestricted `0..=n`.
pub(crate) fn compute_count_bounds(
    fp: &FlatPuzzle,
    answers: &[Option<Answer>; MAX_N],
    eliminated: &[u8; MAX_N],
    n: usize,
) -> CountBounds {
    let oc = fp.option_count;
    let mut floor = [Bound::unrestricted(0); 5];
    let mut ceil = [Bound::unrestricted(n as u8); 5];
    for qi in 0..n {
        // Questions outside the range this one counts — they can hold the letter too, so
        // its ceiling only caps the total once widened by them. Zero for a full-range
        // `CountAnswer`, which caps the total directly.
        let qt = fp.question_types[qi];
        let (letter_index, extra_possible) = match qt {
            QuestionType::CountAnswer { answer } => (answer.idx(), 0),
            QuestionType::CountAnswerBefore {
                answer,
                before_index,
            } => (answer.idx(), (n as u8) - before_index),
            QuestionType::CountAnswerAfter {
                answer,
                after_index,
            } => (answer.idx(), after_index + 1),
            _ => continue,
        };

        let (lo, hi) = if let Some(answer) = answers[qi] {
            let value = fp.options[qi][answer.idx()].value();
            (value, value)
        } else {
            let option_values = (0..oc).filter_map(|oi| {
                if is_eliminated(eliminated, qi, oi) {
                    None
                } else {
                    Some(fp.options[qi][oi].value())
                }
            });
            // Get min and max.
            option_values.fold((u8::MAX, u8::MIN), |acc, x| (acc.0.min(x), acc.1.max(x)))
        };

        if lo == u8::MAX {
            continue;
        }
        floor[letter_index].at_least(lo, qi);
        ceil[letter_index].at_most(hi, extra_possible, qi);
    }
    CountBounds { floor, ceil }
}

// ── Answer-key tallies ──
//
// Generation-side counterparts to the state-based tallies above: the answer key is
// complete, so these are exact counts with no bounds to track.

pub(crate) fn letter_counts(sol: &[Answer; MAX_N], n: usize) -> [i32; 5] {
    let mut counts = [0i32; 5];
    for i in 0..n {
        counts[sol[i].idx()] += 1;
    }
    counts
}

pub(crate) fn count_letter(sol: &[Answer; MAX_N], letter: Answer, n: usize) -> i32 {
    let mut c = 0i32;
    for i in 0..n {
        if sol[i] == letter {
            c += 1;
        }
    }
    c
}
