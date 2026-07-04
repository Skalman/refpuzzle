use crate::build::{self, GenerateResult};
use crate::construct;
use crate::difficulty::PROFILES;
use crate::rng::Rng;
use crate::solve_deduce::solve;
use crate::types::{OptionValue, QuestionTypeKind};
use std::collections::{BTreeMap, BTreeSet};

const LETTERS: [&str; 5] = ["A", "B", "C", "D", "E"];

fn is_letter_valued(kind: QuestionTypeKind) -> bool {
    use QuestionTypeKind::*;
    matches!(
        kind,
        AnswerOf | LeastCommon | MostCommon | NoOtherHasAnswer | AnswerIsSelf | EqualCount
    )
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
    write_answer_freq(&mut md, &levels);

    if output == "-" {
        print!("{md}");
    } else {
        std::fs::write(output, &md).expect("write output");
        eprintln!("wrote {output} ({} bytes)", md.len());
    }
}

/// Generate up to `attempts` puzzles for one level and tally per-type stats.
/// Mirrors production: retry with fresh seeds until a generation succeeds, so
/// `attempts` is the target *puzzle* count, not the generate()-call count.
/// Capped at 100× calls as a backstop against an infeasible profile.
fn collect_level(level: u8, attempts: u32, seed: u32) -> LevelData {
    let profile = &PROFILES[(level - 1) as usize];
    let mut per_type: BTreeMap<QuestionTypeKind, TypeStats> = BTreeMap::new();
    let mut successes = 0u32;
    let mut attempt = 0u32;
    let mut total_calls = 0u32;
    let max_calls = attempts.saturating_mul(100);
    let mut bstats = build::Stats::default(); // accumulates across accepted puzzles

    while successes < attempts && total_calls < max_calls {
        let s = seed
            .wrapping_mul(31)
            .wrapping_add(level as u32)
            .wrapping_mul(17)
            .wrapping_add(attempt.wrapping_mul(0x9e3779b9));
        attempt = attempt.wrapping_add(1);
        total_calls += 1;
        let mut rng = Rng::new(s);
        let result = construct::generate(
            &construct::RECIPES[(level - 1) as usize],
            profile.question_count,
            profile.option_count,
            &mut rng,
            100,
            &mut bstats,
            "stats",
        );
        let Some(result) = result else {
            continue;
        };
        successes += 1;
        tally_puzzle(&result, &mut per_type);
    }

    // Account for puzzles with 0 instances of each known type.
    for kind in QuestionTypeKind::all_flat() {
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
        n: profile.question_count,
        oc: profile.option_count,
        successes,
        total_calls,
        per_type,
        skeletons: bstats.v2_skeleton.count,
        fallback_assign_kinds: bstats.v2_skeleton.fallbacks.assign_kinds,
        fallback_reserve: bstats.v2_skeleton.fallbacks.reserve,
        fallback_backstop: bstats.v2_skeleton.fallbacks.backstop,
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
fn tally_puzzle(result: &GenerateResult, per_type: &mut BTreeMap<QuestionTypeKind, TypeStats>) {
    let solution = solve(&result.fp).answers;
    let mut counts_this_puzzle: BTreeMap<QuestionTypeKind, usize> = BTreeMap::new();

    for qi in 0..result.n {
        let kind = result.question_types[qi].kind();
        *counts_this_puzzle.entry(kind).or_insert(0) += 1;

        let entry = per_type.entry(kind).or_default();
        let Some(correct) = solution[qi] else {
            continue;
        };
        let correct_oi = correct as usize;

        for oi in 0..result.fp.option_count {
            let v = result.fp.options[qi][oi];
            *entry.position_values[oi].entry(v).or_insert(0) += 1;
            if oi == correct_oi {
                *entry.correct_values.entry(v).or_insert(0) += 1;
            } else {
                *entry.distractor_values.entry(v).or_insert(0) += 1;
            }
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

    let mut overview_rows: Vec<(QuestionTypeKind, Vec<Option<f64>>)> = QuestionTypeKind::all_flat()
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
        let absent: Vec<String> = QuestionTypeKind::all_flat()
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
            let label = |v: OptionValue| -> String {
                if v.is_none() {
                    "None".into()
                } else if letter && v.value() < 5 {
                    LETTERS[v.value() as usize].into()
                } else {
                    v.value().to_string()
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
                    &format!("at {}", LETTERS[i]),
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
    let mut kinds: Vec<QuestionTypeKind> = QuestionTypeKind::all_flat()
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
