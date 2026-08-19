use crate::construct;
use crate::recipes;
use crate::rng::Rng;
use crate::solve_deduce::{NoSteps, run_engine};
use crate::stats::Stats;
use crate::types::{Answer, FlatPuzzle, MAX_N, OptionValue, QuestionType, QuestionTypeKind};
use std::collections::{BTreeMap, BTreeSet};

const LETTER_LABELS: [&str; 5] = ["A", "B", "C", "D", "E"];

fn is_letter_valued(kind: QuestionTypeKind) -> bool {
    use QuestionTypeKind::*;
    matches!(
        kind,
        AnswerOf | LeastCommon | MostCommon | NoOtherHasAnswer | AnswerIsSelf | EqualCount
    )
}

/// Candidate-list-vs-key metrics for the types that scope "same answer" to their
/// listed options (`OnlySameAmong`, `OnlySameAsAmong`): how many questions holding the
/// matched letter are *listed* as candidates and how many sit outside the list.
/// Nothing else here relates a question's option row to the answer key.
#[derive(Default)]
struct SharerStats {
    /// listed-sharer count → rows with that count
    listed: BTreeMap<usize, u32>,
    /// rows whose answer is a numeric option, and their summed unlisted sharers
    numeric_rows: u32,
    numeric_unlisted: u32,
    /// rows whose answer is the "none" option, and their summed unlisted sharers
    none_rows: u32,
    none_unlisted: u32,
}

impl SharerStats {
    fn rows(&self) -> u32 {
        self.numeric_rows + self.none_rows
    }
}

#[derive(Default)]
struct TypeStats {
    /// instances_per_puzzle[k] = # puzzles where this type appeared exactly k times
    instances_per_puzzle: BTreeMap<usize, u32>,
    /// value at the correct option position, across all instances
    correct_values: BTreeMap<OptionValue, u32>,
    /// values at non-correct option positions, across all instances
    distractor_values: BTreeMap<OptionValue, u32>,
    /// values per option position (A=0..E=4)
    position_values: [BTreeMap<OptionValue, u32>; 5],
    /// Populated only for the scoped-sameness kinds; `rows() == 0` elsewhere.
    sharers: SharerStats,
}

struct LevelData {
    level: u8,
    n: usize,
    oc: usize,
    successes: u32,
    total_calls: u32,
    per_type: BTreeMap<QuestionTypeKind, TypeStats>,
    /// Telemetry across all skeleton generations: total skeletons + fallback
    /// substitutions by phase.
    skeletons: u32,
    fallback_assign_kinds: u32,
    fallback_reserve: u32,
    fallback_backstop: u32,
}

/// `output` is a file path, or `-` for stdout.
pub fn type_stats(attempts: u32, seed: u32, output: &str) {
    let levels: Vec<LevelData> = (1..=6u8)
        .map(|l| collect_level(l, attempts, seed))
        .collect();

    let mut md = String::new();
    md.push_str(&format!(
        "# Puzzle generation statistics\n\nUp to {attempts} attempts per level. Base seed {seed}.\n\n"
    ));
    write_overview(&mut md, &levels);
    write_fallbacks(&mut md, &levels);
    write_multiplicity(&mut md, &levels);
    write_sharers(&mut md, &levels);
    write_answer_freq(&mut md, &levels);

    if output == "-" {
        print!("{md}");
    } else {
        std::fs::write(output, &md).expect("write output");
        eprintln!("wrote {output} ({} bytes)", md.len());
    }
}

/// Print `fill::none_correct_rate`'s table body from a fresh measurement: one
/// paste-ready match arm per NONE-capable kind, in enum order, then the measured
/// NONE ratio of every structurally skewed row (`p > 1/option_count`) as the
/// input for a `KNOWN_SKEW` ceiling (measured value plus headroom).
pub fn calibration(attempts: u32, seed: u32) {
    let levels: Vec<LevelData> = (1..=6u8)
        .map(|l| collect_level(l, attempts, seed))
        .collect();

    let none_shares = |kind: QuestionTypeKind| -> Vec<f64> {
        levels
            .iter()
            .map(|ld| {
                ld.per_type.get(&kind).map_or(0.0, |entry| {
                    let total: u32 = entry.correct_values.values().sum();
                    let none = entry
                        .correct_values
                        .get(&OptionValue::NONE)
                        .copied()
                        .unwrap_or(0);
                    if total == 0 {
                        0.0
                    } else {
                        none as f64 / total as f64
                    }
                })
            })
            .collect()
    };

    println!("            //               L1    L2    L3    L4    L5    L6");
    for &kind in QuestionTypeKind::all() {
        if !kind.may_be_none() {
            continue;
        }
        let shares = none_shares(kind);
        let cells: Vec<String> = shares.iter().map(|s| format!("{s:.2}")).collect();
        println!("            {kind:?} => [{}],", cells.join(", "));
        for (i, (&share, cell)) in shares.iter().zip(&cells).enumerate() {
            if share > 0.0 && cell == "0.00" {
                eprintln!(
                    "warning: {kind:?} L{} is used but its share rounds to 0.00, which the \
                     table reads as absent — see the trap note on `none_correct_rate`",
                    i + 1
                );
            }
        }
    }

    println!();
    println!("Structurally skewed rows (NONE-correct share above 1/option_count), measured ratio:");
    for &kind in QuestionTypeKind::all() {
        if !kind.may_be_none() {
            continue;
        }
        for ld in &levels {
            let Some(entry) = ld.per_type.get(&kind) else {
                continue;
            };
            let correct_total: u32 = entry.correct_values.values().sum();
            let distractor_total: u32 = entry.distractor_values.values().sum();
            let correct_none = entry
                .correct_values
                .get(&OptionValue::NONE)
                .copied()
                .unwrap_or(0);
            let distractor_none = entry
                .distractor_values
                .get(&OptionValue::NONE)
                .copied()
                .unwrap_or(0);
            if correct_total == 0 || distractor_none == 0 {
                continue;
            }
            let p = correct_none as f64 / correct_total as f64;
            if p <= 1.0 / ld.oc as f64 {
                continue;
            }
            let ratio = p / (distractor_none as f64 / distractor_total as f64);
            println!("  {kind:?} L{}: {ratio:.2}", ld.level);
        }
    }
}

/// Generate up to `attempts` puzzles for one level and tally per-type stats.
/// Mirrors production: retry with fresh seeds until a generation succeeds, so
/// `attempts` is the target *puzzle* count, not the generate()-call count.
/// Capped at 100× calls as a backstop against an infeasible profile.
fn collect_level(level: u8, attempts: u32, seed: u32) -> LevelData {
    let recipe = &recipes::RECIPES[(level - 1) as usize];
    let mut per_type: BTreeMap<QuestionTypeKind, TypeStats> = BTreeMap::new();
    let mut successes = 0u32;
    let mut total_calls = 0u32;
    let max_calls = attempts.saturating_mul(100);
    let mut bstats = Stats::default(); // accumulates across all generate() calls, rejected included

    while successes < attempts && total_calls < max_calls {
        // Seed uses the pre-increment call index, so the sequence is stable.
        let s = seed
            .wrapping_mul(31)
            .wrapping_add(level as u32)
            .wrapping_mul(17)
            .wrapping_add(total_calls.wrapping_mul(0x9e3779b9));
        total_calls += 1;
        let mut rng = Rng::new(s);
        let result = construct::generate(recipe, &mut rng, 100, &mut bstats, "stats");
        let Some(result) = result else {
            continue;
        };
        successes += 1;
        tally_puzzle(&result, &mut per_type, recipe);
    }

    // Account for puzzles with 0 instances of each known type.
    for kind in QuestionTypeKind::all() {
        if let Some(entry) = per_type.get_mut(kind) {
            let nonzero: u32 = entry.instances_per_puzzle.values().sum();
            let zeros = successes.saturating_sub(nonzero);
            if zeros > 0 {
                entry.instances_per_puzzle.insert(0, zeros);
            }
        }
    }

    LevelData {
        level,
        n: recipe.question_count,
        oc: recipe.option_count,
        successes,
        total_calls,
        per_type,
        skeletons: bstats.skeleton.count,
        fallback_assign_kinds: bstats.skeleton.fallbacks.assign_kinds,
        fallback_reserve: bstats.skeleton.fallbacks.reserve,
        fallback_backstop: bstats.skeleton.fallbacks.backstop,
    }
}

/// Per-level skeleton telemetry: total skeletons (with attempts-per-accepted-
/// puzzle, i.e. the rejection ratio), and fallback substitutions per phase as a
/// per-skeleton rate. `reserve` swaps in another pool kind; `assign_kinds` and
/// `backstop` fall back to AnswerOf.
fn write_fallbacks(md: &mut String, levels: &[LevelData]) {
    md.push_str(
        "## Skeleton telemetry\n\n`skeletons` is total skeletons generated (with attempts per accepted puzzle); fallback columns are totals (with per-skeleton rate).\n\n",
    );
    let header: Vec<String> = ["Level", "skeletons", "assign_kinds", "reserve", "backstop"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let rows: Vec<Vec<String>> = levels
        .iter()
        .map(|l| {
            let per_puzzle = |c: u32| match l.successes {
                0 => c.to_string(),
                s => format!("{c} ({:.1}/pz)", c as f64 / s as f64),
            };
            let per_skeleton = |c: u32| match l.skeletons {
                0 => c.to_string(),
                skeletons => format!("{c} ({:.2}/sk)", c as f64 / skeletons as f64),
            };
            vec![
                format!("L{}", l.level),
                per_puzzle(l.skeletons),
                per_skeleton(l.fallback_assign_kinds),
                per_skeleton(l.fallback_reserve),
                per_skeleton(l.fallback_backstop),
            ]
        })
        .collect();
    render_table(md, &header, &rows);
    md.push('\n');
}

/// Fold one generated puzzle into the running per-type tallies.
fn tally_puzzle(
    result: &FlatPuzzle,
    per_type: &mut BTreeMap<QuestionTypeKind, TypeStats>,
    recipe: &recipes::LevelRecipe,
) {
    // Read the answer key with the engine the gate accepts on: `standard` at the
    // recipe depth. An accepted puzzle solves under it, so the `unreachable!` below
    // can't fire — a fired one is a gate/engine bug, so fail loud (release too).
    let solution = run_engine(
        result,
        result.initial_state,
        recipe.standard_config(),
        result.n * 15,
        &mut NoSteps,
    )
    .state
    .answers;
    let mut counts_this_puzzle: BTreeMap<QuestionTypeKind, usize> = BTreeMap::new();

    for qi in 0..result.n {
        let kind = result.question_types[qi].kind();
        *counts_this_puzzle.entry(kind).or_insert(0) += 1;

        let entry = per_type.entry(kind).or_default();
        let Some(correct) = solution[qi] else {
            unreachable!("accepted puzzle has unsolved Q{qi}");
        };
        let correct_oi = correct as usize;

        for oi in 0..result.option_count {
            let v = result.options[qi][oi];
            *entry.position_values[oi].entry(v).or_insert(0) += 1;
            if oi == correct_oi {
                *entry.correct_values.entry(v).or_insert(0) += 1;
            } else {
                *entry.distractor_values.entry(v).or_insert(0) += 1;
            }
        }

        if let Some((matched, excluded)) = scoped_sameness(result, qi, &solution) {
            tally_sharers(
                &mut entry.sharers,
                result,
                qi,
                matched,
                excluded,
                correct_oi,
                &solution,
            );
        }
    }

    for (&kind, &count) in &counts_this_puzzle {
        *per_type
            .entry(kind)
            .or_default()
            .instances_per_puzzle
            .entry(count)
            .or_insert(0) += 1;
    }
}

/// For a question that scopes "same answer" to its listed options: the matched
/// letter M, plus the question index excluded from its candidate pool (the
/// reference, for `OnlySameAsAmong`). `None` for every other kind.
fn scoped_sameness(
    fp: &FlatPuzzle,
    qi: usize,
    solution: &[Option<Answer>; MAX_N],
) -> Option<(Answer, Option<usize>)> {
    match fp.question_types[qi] {
        QuestionType::OnlySameAmong => Some((solution[qi]?, None)),
        QuestionType::OnlySameAsAmong { question_index } => {
            let ref_qi = usize::from(question_index);
            Some((solution[ref_qi]?, Some(ref_qi)))
        }
        _ => None,
    }
}

/// Count how many questions holding `matched` this row lists as candidates and
/// how many it leaves out, and record which half of the option set answered it.
fn tally_sharers(
    stats: &mut SharerStats,
    fp: &FlatPuzzle,
    qi: usize,
    matched: Answer,
    excluded: Option<usize>,
    correct_oi: usize,
    solution: &[Option<Answer>; MAX_N],
) {
    let mut listed_mask = 0u16;
    for oi in 0..fp.option_count {
        let ov = fp.options[qi][oi];
        if ov.is_num() {
            let j = usize::from(ov.value());
            if j < fp.n {
                listed_mask |= 1 << j;
            }
        }
    }

    let mut listed = 0usize;
    let mut unlisted = 0u32;
    for j in 0..fp.n {
        if j == qi || Some(j) == excluded || solution[j] != Some(matched) {
            continue;
        }
        if (listed_mask >> j) & 1 == 1 {
            listed += 1;
        } else {
            unlisted += 1;
        }
    }

    *stats.listed.entry(listed).or_insert(0) += 1;
    if fp.options[qi][correct_oi].is_none() {
        stats.none_rows += 1;
        stats.none_unlisted += unlisted;
    } else {
        stats.numeric_rows += 1;
        stats.numeric_unlisted += unlisted;
    }
}

/// Render an aligned markdown table. First column is left-aligned (labels),
/// the rest right-aligned (numbers). Every column is padded to its widest
/// cell so the raw markdown source lines up. Each row must match `header` len.
fn render_table(md: &mut String, header: &[String], rows: &[Vec<String>]) {
    let mut w: Vec<usize> = header.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            w[i] = w[i].max(cell.chars().count());
        }
    }

    let push_cells = |md: &mut String, cells: &[String]| {
        md.push('|');
        for (i, cell) in cells.iter().enumerate() {
            if i == 0 {
                md.push_str(&format!(" {:<width$} |", cell, width = w[i]));
            } else {
                md.push_str(&format!(" {:>width$} |", cell, width = w[i]));
            }
        }
        md.push('\n');
    };

    push_cells(md, header);
    md.push('|');
    for (i, &width) in w.iter().enumerate() {
        if i == 0 {
            md.push(':');
            md.push_str(&"-".repeat(width + 1));
        } else {
            md.push_str(&"-".repeat(width + 1));
            md.push(':');
        }
        md.push('|');
    }
    md.push('\n');
    for row in rows {
        push_cells(md, row);
    }
}

/// Overview row ordering: (1) more levels present first, (2) present in an
/// earlier level first, (3) higher % in an earlier level first, (4) name.
/// Each row is the per-level presence% (`None` = type not allowed there).
fn cmp_overview_rows(
    a: &(QuestionTypeKind, Vec<Option<f64>>),
    b: &(QuestionTypeKind, Vec<Option<f64>>),
) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let count = |row: &[Option<f64>]| row.iter().filter(|c| c.is_some()).count();
    count(&b.1)
        .cmp(&count(&a.1))
        .then_with(|| {
            for (ca, cb) in a.1.iter().zip(b.1.iter()) {
                match (ca.is_some(), cb.is_some()) {
                    (true, false) => return Ordering::Less,
                    (false, true) => return Ordering::Greater,
                    _ => {}
                }
            }
            Ordering::Equal
        })
        .then_with(|| {
            for (ca, cb) in a.1.iter().zip(b.1.iter()) {
                match cb.unwrap_or(0.0).partial_cmp(&ca.unwrap_or(0.0)) {
                    Some(Ordering::Equal) | None => {}
                    Some(ord) => return ord,
                }
            }
            Ordering::Equal
        })
        .then_with(|| format!("{:?}", a.0).cmp(&format!("{:?}", b.0)))
}

/// Overview matrix: rows = question types, cols = levels, cell = % of that
/// level's puzzles containing ≥1 instance (blank = not allowed at that level).
/// Sorted by number of levels present (desc), then by presence in earlier levels.
fn write_overview(md: &mut String, levels: &[LevelData]) {
    let presence = |ld: &LevelData, kind: QuestionTypeKind| -> Option<f64> {
        let entry = ld.per_type.get(&kind)?;
        if entry.correct_values.is_empty() {
            return None;
        }
        let zero = entry.instances_per_puzzle.get(&0).copied().unwrap_or(0);
        Some(100.0 * (ld.successes - zero) as f64 / ld.successes as f64)
    };

    let mut overview_rows: Vec<(QuestionTypeKind, Vec<Option<f64>>)> = QuestionTypeKind::all()
        .iter()
        .map(|&k| {
            (
                k,
                levels.iter().map(|ld| presence(ld, k)).collect::<Vec<_>>(),
            )
        })
        .filter(|(_, row)| row.iter().any(Option::is_some))
        .collect();
    overview_rows.sort_by(cmp_overview_rows);

    md.push_str(
        "## Overview\n\nPercentage of each level's puzzles containing at least one \
         instance of the type. Blank = not allowed at that level. Sorted by number \
         of levels present (desc), then by presence in earlier levels.\n\n",
    );
    let header: Vec<String> = std::iter::once(String::new())
        .chain(levels.iter().map(|ld| format!("L{}", ld.level)))
        .collect();
    let rows: Vec<Vec<String>> = overview_rows
        .iter()
        .map(|(kind, row)| {
            std::iter::once(format!("{kind:?}"))
                .chain(row.iter().map(|c| match c {
                    Some(p) => format!("{p:.0}"),
                    None => String::new(),
                }))
                .collect()
        })
        .collect();
    render_table(md, &header, &rows);
    md.push('\n');
}

/// One multiplicity table per level: rows = question types, cols = N×, cell =
/// count of puzzles where the type appeared N times. Lists never-placed types.
fn write_multiplicity(md: &mut String, levels: &[LevelData]) {
    md.push_str(
        "## Multiplicity\n\nCount of puzzles where each question type appeared N times.\n\n",
    );
    for ld in levels {
        let yield_pct = 100.0 * ld.successes as f64 / ld.total_calls as f64;
        md.push_str(&format!(
            "<details>\n<summary>Level {} (n={}, options={})</summary>\n\n\
             {} puzzles ({:.1}% yield).\n\n",
            ld.level, ld.n, ld.oc, ld.successes, yield_pct
        ));

        let sorted_kinds = sorted_present_kinds(&ld.per_type);
        let max_mult = sorted_kinds
            .iter()
            .filter_map(|k| ld.per_type.get(k))
            .flat_map(|e| e.instances_per_puzzle.keys().copied())
            .max()
            .unwrap_or(0);

        let present_set: BTreeSet<QuestionTypeKind> = sorted_kinds.iter().copied().collect();
        let absent: Vec<String> = QuestionTypeKind::all()
            .iter()
            .filter(|k| !present_set.contains(k))
            .map(|k| format!("{k:?}"))
            .collect();
        if !absent.is_empty() {
            md.push_str(&format!("Never placed: {}.\n\n", absent.join(", ")));
        }

        let header: Vec<String> = std::iter::once(String::new())
            .chain((0..=max_mult).map(|c| format!("{c}×")))
            .collect();
        let rows: Vec<Vec<String>> = sorted_kinds
            .iter()
            .map(|kind| {
                let entry = &ld.per_type[kind];
                std::iter::once(format!("{kind:?}"))
                    .chain((0..=max_mult).map(|c| {
                        match entry.instances_per_puzzle.get(&c).copied().unwrap_or(0) {
                            0 => String::new(),
                            n => n.to_string(),
                        }
                    }))
                    .collect()
            })
            .collect();
        render_table(md, &header, &rows);
        md.push_str("\n</details>\n\n");
    }
}

/// Candidate sharers: for each level and each scoped-sameness type, how many of
/// the questions holding the matched letter the row actually lists. `listed N` is
/// a row count; `unlisted` columns are the mean per row of that answer shape.
///
/// The two readings this exists for: `listed` must be exactly 1 for a
/// numeric-answered row and 0 for a none-answered one (anything else is a key the
/// "only one" reading can't check), and a non-zero `unlisted/none` is what says
/// the correct value was picked from the candidate list rather than the key.
fn write_sharers(md: &mut String, levels: &[LevelData]) {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut max_listed = 1usize;
    for ld in levels {
        for entry in ld.per_type.values() {
            if entry.sharers.rows() == 0 {
                continue;
            }
            max_listed = max_listed.max(entry.sharers.listed.keys().copied().max().unwrap_or(0));
        }
    }
    for ld in levels {
        for (&kind, entry) in &ld.per_type {
            let s = &entry.sharers;
            if s.rows() == 0 {
                continue;
            }
            let mean = |total: u32, count: u32| match count {
                0 => String::new(),
                c => format!("{:.3}", total as f64 / c as f64),
            };
            rows.push(
                [
                    format!("L{} {kind:?}", ld.level),
                    s.rows().to_string(),
                    s.numeric_rows.to_string(),
                    s.none_rows.to_string(),
                ]
                .into_iter()
                .chain(
                    (0..=max_listed).map(|k| match s.listed.get(&k).copied().unwrap_or(0) {
                        0 => String::new(),
                        c => c.to_string(),
                    }),
                )
                .chain([
                    mean(s.numeric_unlisted, s.numeric_rows),
                    mean(s.none_unlisted, s.none_rows),
                ])
                .collect(),
            );
        }
    }
    if rows.is_empty() {
        return;
    }

    md.push_str(
        "## Candidate sharers\n\nFor the types that scope \"same answer\" to their \
         listed options. `rows` is question instances; `num-ans` / `none-ans` split \
         them by which half of the option set is the answer. `listed N` counts rows \
         where exactly N *listed* candidates hold the matched letter. The last two \
         columns are the mean number of questions holding it that the row does *not* \
         list.\n\n",
    );
    let header: Vec<String> = ["", "rows", "num-ans", "none-ans"]
        .iter()
        .map(|s| s.to_string())
        .chain((0..=max_listed).map(|k| format!("listed {k}")))
        .chain(["unlisted/num".to_string(), "unlisted/none".to_string()])
        .collect();
    render_table(md, &header, &rows);
    md.push('\n');
}

/// Per-type answer/distractor/position tables, grouped per level. Each cell is
/// a percentage; the `ratio` row is correct% / distractor% (near 1.0 = neutral).
fn write_answer_freq(md: &mut String, levels: &[LevelData]) {
    md.push_str("## Answer frequency and positioning\n\n");
    md.push_str(
        "Per option value, percentage as correct answer, as distractor, and \
         per option-position. `ratio` is correct% / distractors% — a value \
         near 1.0 means the answer is no more or less likely than its \
         distractor frequency would suggest.\n\n",
    );
    for ld in levels {
        md.push_str(&format!(
            "<details>\n<summary>Level {} (n={}, options={})</summary>\n\n",
            ld.level, ld.n, ld.oc
        ));
        for kind in &sorted_present_kinds(&ld.per_type) {
            let entry = &ld.per_type[kind];
            md.push_str(&format!(
                "<details style=\"margin-left: 1em\">\n<summary>{kind:?}</summary>\n\n"
            ));

            let mut keys: BTreeSet<OptionValue> = BTreeSet::new();
            keys.extend(entry.correct_values.keys());
            keys.extend(entry.distractor_values.keys());
            for pv in &entry.position_values {
                keys.extend(pv.keys());
            }
            let letter = is_letter_valued(*kind);
            let label = |ov: OptionValue| -> String {
                if ov.is_none() {
                    "None".into()
                } else if letter && ov.value() < 5 {
                    LETTER_LABELS[ov.value() as usize].into()
                } else {
                    ov.value().to_string()
                }
            };

            // A row of percentages (blank where the value never occurs).
            let pct_row = |name: &str, m: &BTreeMap<OptionValue, u32>| -> Vec<String> {
                let total: u32 = m.values().sum();
                std::iter::once(name.to_string())
                    .chain(keys.iter().map(|&k| match m.get(&k).copied().unwrap_or(0) {
                        0 => String::new(),
                        n => format!("{:.1}", 100.0 * n as f64 / total as f64),
                    }))
                    .collect()
            };

            let correct_total: u32 = entry.correct_values.values().sum();
            let distractor_total: u32 = entry.distractor_values.values().sum();
            let ratio_row: Vec<String> = std::iter::once("ratio".to_string())
                .chain(keys.iter().map(|&k| {
                    let cn = entry.correct_values.get(&k).copied().unwrap_or(0);
                    let dn = entry.distractor_values.get(&k).copied().unwrap_or(0);
                    if cn == 0 && dn == 0 {
                        String::new()
                    } else if dn == 0 || distractor_total == 0 {
                        "—".to_string()
                    } else {
                        let cp = cn as f64 / correct_total as f64;
                        let dp = dn as f64 / distractor_total as f64;
                        format!("{:.2}", cp / dp)
                    }
                }))
                .collect();

            let header: Vec<String> = std::iter::once(String::new())
                .chain(keys.iter().map(|&k| label(k)))
                .collect();
            let mut rows = vec![
                pct_row("correct", &entry.correct_values),
                pct_row("distractors", &entry.distractor_values),
                ratio_row,
            ];
            for i in 0..ld.oc {
                rows.push(pct_row(
                    &format!("at {}", LETTER_LABELS[i]),
                    &entry.position_values[i],
                ));
            }
            render_table(md, &header, &rows);
            md.push_str("\n</details>\n\n");
        }
        md.push_str("</details>\n\n");
    }
}

fn sorted_present_kinds(per_type: &BTreeMap<QuestionTypeKind, TypeStats>) -> Vec<QuestionTypeKind> {
    let mut kinds: Vec<QuestionTypeKind> = QuestionTypeKind::all()
        .iter()
        .copied()
        .filter(|k| {
            per_type
                .get(k)
                .is_some_and(|e| !e.correct_values.is_empty())
        })
        .collect();
    kinds.sort_by_key(|k| format!("{k:?}"));
    kinds
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::fast_tests;
    use crate::types::QuestionTypeKind as Kind;

    /// Acceptable band for a value's correct-share ÷ distractor-share. The ratio converts
    /// straight to the player-facing rate:
    /// `P(value correct | value on the row) = ratio / (ratio + option_count - 1)`.
    ///
    /// So 1.0 is neutral — offered on a row, the value is correct exactly
    /// `1/option_count` of the time — and this band is 14.9%..=25.9% at `oc = 5` against
    /// a fair 20%. For scale, the pre-calibration `OnlySameAmong` L6 ratio of 6.01 was a 60% hit
    /// rate for "pick None whenever it's offered".
    ///
    /// This is the design target itself, not a loosened version of it: the sample below
    /// is much smaller than the 10k-attempt run the calibration was measured on, but the
    /// estimates still land inside, so there's no reason to give the gate extra slack.
    const BAND: (f64, f64) = (0.7, 1.4);

    /// A `(kind, level)` whose skew is structural rather than a calibration failure, so
    /// it is held to a ceiling instead of `BAND`. Its NONE-correct rate `p` exceeds
    /// `1/option_count`, and then even offering NONE on every row where it is *wrong*
    /// leaves `ratio = p·(oc-1)/(1-p) > 1` — see `fill::none_distractor_rate`. No option
    /// row can fix these; only a lower `p` can.
    struct KnownSkew {
        kind: Kind,
        level: u8,
        /// Highest ratio this row may show before the gate fails.
        ceiling: f64,
    }

    /// Ceilings are the value measured by `type-stats --attempts 10000 --seed 1` plus
    /// headroom. They come down only by lowering `p`, which means changing where the type
    /// is placed or how its row is sampled — not the option row. The other 16 rows with a
    /// NONE option are inside `BAND` and deliberately absent from this list.
    const KNOWN_SKEW: &[KnownSkew] = &[
        KnownSkew {
            kind: Kind::ClosestAfter,
            level: 1,
            ceiling: 4.90,
        },
        KnownSkew {
            kind: Kind::ClosestAfter,
            level: 3,
            ceiling: 3.60,
        },
        KnownSkew {
            kind: Kind::ClosestAfter,
            level: 4,
            ceiling: 1.90,
        },
        KnownSkew {
            kind: Kind::ClosestAfter,
            level: 5,
            ceiling: 1.60,
        },
        KnownSkew {
            kind: Kind::ClosestBefore,
            level: 1,
            ceiling: 5.15,
        },
        KnownSkew {
            kind: Kind::ClosestBefore,
            level: 3,
            ceiling: 3.70,
        },
        KnownSkew {
            kind: Kind::ClosestBefore,
            level: 4,
            ceiling: 1.90,
        },
        KnownSkew {
            kind: Kind::EqualCount,
            level: 5,
            ceiling: 4.90,
        },
        KnownSkew {
            kind: Kind::EqualCount,
            level: 6,
            ceiling: 3.85,
        },
        KnownSkew {
            kind: Kind::FirstWith,
            level: 1,
            ceiling: 3.10,
        },
        KnownSkew {
            kind: Kind::FirstWith,
            level: 2,
            ceiling: 2.15,
        },
        KnownSkew {
            kind: Kind::FirstWith,
            level: 3,
            ceiling: 3.00,
        },
        KnownSkew {
            kind: Kind::LastWith,
            level: 1,
            ceiling: 3.25,
        },
        KnownSkew {
            kind: Kind::LastWith,
            level: 2,
            ceiling: 2.20,
        },
        KnownSkew {
            kind: Kind::LastWith,
            level: 3,
            ceiling: 2.90,
        },
        KnownSkew {
            kind: Kind::NextSame,
            level: 1,
            ceiling: 2.75,
        },
        KnownSkew {
            kind: Kind::NextSame,
            level: 3,
            ceiling: 4.25,
        },
        KnownSkew {
            kind: Kind::NextSame,
            level: 4,
            ceiling: 2.65,
        },
        KnownSkew {
            kind: Kind::NextSame,
            level: 5,
            ceiling: 1.90,
        },
        KnownSkew {
            kind: Kind::OnlyEven,
            level: 5,
            ceiling: 4.45,
        },
        KnownSkew {
            kind: Kind::OnlyEven,
            level: 6,
            ceiling: 4.10,
        },
        KnownSkew {
            kind: Kind::OnlyOdd,
            level: 5,
            ceiling: 4.55,
        },
        KnownSkew {
            kind: Kind::OnlyOdd,
            level: 6,
            ceiling: 4.00,
        },
        KnownSkew {
            kind: Kind::PrevSame,
            level: 1,
            ceiling: 2.65,
        },
        KnownSkew {
            kind: Kind::PrevSame,
            level: 3,
            ceiling: 3.95,
        },
        KnownSkew {
            kind: Kind::PrevSame,
            level: 4,
            ceiling: 2.65,
        },
        KnownSkew {
            kind: Kind::PrevSame,
            level: 5,
            ceiling: 2.00,
        },
        KnownSkew {
            kind: Kind::OnlySameAmong,
            level: 1,
            ceiling: 5.75,
        },
        KnownSkew {
            kind: Kind::OnlySameAmong,
            level: 3,
            ceiling: 5.55,
        },
        KnownSkew {
            kind: Kind::OnlySameAmong,
            level: 4,
            ceiling: 4.35,
        },
        KnownSkew {
            kind: Kind::OnlySameAmong,
            level: 5,
            ceiling: 3.65,
        },
        KnownSkew {
            kind: Kind::OnlySameAmong,
            level: 6,
            ceiling: 3.30,
        },
        KnownSkew {
            kind: Kind::OnlySameAsAmong,
            level: 5,
            ceiling: 3.90,
        },
        KnownSkew {
            kind: Kind::OnlySameAsAmong,
            level: 6,
            ceiling: 3.45,
        },
    ];

    /// A kind needs this many instances at a level before its ratio is asserted on —
    /// below it the estimate is too noisy to mean anything. Skips are reported.
    const MIN_INSTANCES: u32 = 150;

    /// The None option must not be a tell, and must not vanish either. Regenerates a
    /// sample per level and checks two things per kind: its NONE ratio against `BAND` (or
    /// `KNOWN_SKEW`'s ceiling where no option row could reach the band), and that NONE
    /// still turns up on some rows at all.
    #[test]
    fn none_ratio_within_band() {
        // A statistical check can't shrink and keep its meaning: at fast-run sizes most
        // kinds never reach `MIN_INSTANCES`, so that mode is a smoke pass over the few
        // rows that do, and only the full run is the gate. Both still assert on every row
        // they measure — the difference is how many rows that is.
        let (attempts, min_rows) = if fast_tests() { (300, 3) } else { (1500, 45) };
        // The fast sample is a fifth of the full run, so its estimates carry ~sqrt(5)x
        // the noise. Stretch the band by that factor in log space: a smoke bound that
        // still catches a real skew, while only the full run asserts the design target.
        let band = if fast_tests() { (0.45, 2.15) } else { BAND };
        // A ratio inside `BAND` says NONE is weighted fairly on the rows that offer it,
        // but says nothing about how many rows those are — drive its correct-rate toward
        // zero and the ratio stays at 1 while NONE disappears from the game. This is the
        // separate canary for that. Sparsest today is `OnlySame` L6 at 13.7%, so the bar
        // sits far below anything real; it's here to catch a vanishing, not drift.
        const MIN_SHOWN: f64 = 0.03;
        let mut failures: Vec<String> = Vec::new();
        let mut skipped: Vec<String> = Vec::new();
        let mut checked = 0usize;
        let mut tightest_shown = (f64::MAX, String::new());

        for level in 1..=6u8 {
            let ld = collect_level(level, attempts, 1);
            for (&kind, entry) in &ld.per_type {
                let correct_total: u32 = entry.correct_values.values().sum();
                let distractor_total: u32 = entry.distractor_values.values().sum();
                let correct_none = entry
                    .correct_values
                    .get(&OptionValue::NONE)
                    .copied()
                    .unwrap_or(0);
                let distractor_none = entry
                    .distractor_values
                    .get(&OptionValue::NONE)
                    .copied()
                    .unwrap_or(0);
                if correct_none == 0 && distractor_none == 0 {
                    continue; // no NONE option at this level
                }
                if correct_total < MIN_INSTANCES {
                    skipped.push(format!("{kind:?} L{level} ({correct_total} instances)"));
                    continue;
                }
                let ceiling = KNOWN_SKEW
                    .iter()
                    .find(|p| p.kind == kind && p.level == level)
                    .map(|p| p.ceiling);
                checked += 1;

                // Rows offering NONE at all, whether as the answer or a distractor.
                let shown = (correct_none + distractor_none) as f64 / correct_total as f64;
                if shown < tightest_shown.0 {
                    tightest_shown = (shown, format!("{kind:?} L{level}"));
                }
                if shown < MIN_SHOWN {
                    failures.push(format!(
                        "{kind:?} L{level}: NONE is shown on only {:.1}% of rows, under the \
                         {:.0}% floor — it has all but left the option pool",
                        100.0 * shown,
                        100.0 * MIN_SHOWN,
                    ));
                }

                if distractor_none == 0 {
                    failures.push(format!(
                        "{kind:?} L{level}: NONE is never a distractor but is correct \
                         {:.1}% of the time — always a tell",
                        100.0 * correct_none as f64 / correct_total as f64
                    ));
                    continue;
                }
                let ratio = (correct_none as f64 / correct_total as f64)
                    / (distractor_none as f64 / distractor_total as f64);
                let ok = match ceiling {
                    Some(c) => ratio <= c,
                    None => ratio >= band.0 && ratio <= band.1,
                };
                if !ok {
                    let want = match ceiling {
                        Some(c) => format!("<= {c:.2} (known-skewed row)"),
                        None => format!("in {:.2}..={:.2}", band.0, band.1),
                    };
                    failures.push(format!(
                        "{kind:?} L{level}: ratio {ratio:.2}, want {want} \
                         (NONE correct {:.1}%, distractor {:.1}%)",
                        100.0 * correct_none as f64 / correct_total as f64,
                        100.0 * distractor_none as f64 / distractor_total as f64,
                    ));
                }
            }
        }

        if !skipped.is_empty() {
            eprintln!(
                "none_ratio_within_band: {} rows too small to assert on: {}",
                skipped.len(),
                skipped.join(", ")
            );
        }
        eprintln!(
            "none_ratio_within_band: {checked} rows checked; NONE shown least often on \
             {} ({:.1}% of rows)",
            tightest_shown.1,
            100.0 * tightest_shown.0,
        );
        assert!(
            checked >= min_rows,
            "only {checked} rows checked at {attempts} attempts, wanted {min_rows} — the \
             sample got too small to be a gate"
        );
        assert!(
            failures.is_empty(),
            "NONE option is skewed in {} of {checked} rows:\n  {}\n\
             Re-measure with `cargo run --release -- type-stats --calibration` and paste \
             the printed table over `fill::none_correct_rate`'s; it also prints each \
             skewed row's measured ratio, the input for a `KNOWN_SKEW` ceiling.",
            failures.len(),
            failures.join("\n  ")
        );
    }
}
