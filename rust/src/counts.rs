//! Count/tally primitives shared by `deduce`, `check_answer`, and `explain`: the
//! mask-selected tally, per-letter cell accounting, and whole-puzzle
//! count-question bounds. Lives here — not in `deduce` or `check_answer` — so both
//! can use it without a module cycle (`deduce` already depends on `check_answer`).

use crate::types::*;

/// Mask-selected tally over a range: `count` answered matches, `guaranteed`
/// unanswered cells *locked* to a match (only masked options remain), and
/// `possible` unanswered cells that could go either way. `min` (= count +
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
/// These are *cell* facts. Any rule that hypothesises "what if this one cell
/// were letter X" — the extremum `±1`, OnlySame's per-cell subtract — must read
/// these, never the abstract `CountBounds`: an external bound may already
/// account for the very cell being adjusted, so adding to it would double-count.
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
/// answered cells. `floor[i]` is a lower bound on the total count of letter i;
/// `ceil[i]` an upper bound.
///
/// Floors come from all three count kinds — a sub-range floor still
/// lower-bounds the total. Ceilings come from full-range `CountAnswer` only: a
/// `Before`/`After` ceiling bounds a sub-range and says nothing about the rest
/// of the puzzle. Consumers combine these with `LetterCells` via `lower`/`upper`
/// — never feed them into a per-cell `±1`, which would double-count.
#[derive(Clone, Copy)]
pub(crate) struct CountBounds {
    floor: [u8; 5],
    ceil: [u8; 5],
}

impl CountBounds {
    /// Combined lower bound on count(i): the tighter of placed cells and the
    /// Count-question floor.
    #[inline(always)]
    pub(crate) fn lower(&self, cells: &LetterCells, i: usize) -> u8 {
        cells.filled[i].max(self.floor[i])
    }

    /// Combined upper bound on count(i): the tighter of the cell ceiling and
    /// the Count-question ceiling.
    #[inline(always)]
    pub(crate) fn upper(&self, cells: &LetterCells, i: usize) -> u8 {
        cells.cell_max(i).min(self.ceil[i])
    }
}

pub(crate) fn compute_count_bounds(
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
            } else if is_eliminated(eliminated, k, oi) {
                continue;
            }
            let ov = fp.options[k][oi];
            if ov.is_num() {
                lo = lo.min(ov.value());
                hi = hi.max(ov.value());
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
