//! TEMPORARY: stuck-puzzle diagnostic for rebuilding repair. Samples a fixed
//! number of v2 composes per level, runs `validate_and_repair` (engine + move #1
//! distractor repair), and reports (a) a per-level histogram of how far it got
//! and (b) example `/playground` links to puzzles repair couldn't crack, split into
//! no-progress (solved 0) and partial-progress buckets. Delete this module and
//! the `stuck` subcommand once `validate_and_repair` does real repair.

use std::io::IsTerminal;

use crate::build::{Stats, fill_options};
use crate::construct_v2::{RECIPES, Verdict, compose, validate_and_repair};
use crate::difficulty::PROFILES;
use crate::rng::Rng;
use crate::serialize::playground_link;
use crate::solve_brute::solve;

struct StuckCase {
    level: usize,
    solved: usize,
    n: usize,
    filled: usize,     // resolved cells (eliminations + solved cells)
    total: usize,      // n * oc
    solutions: usize,  // brute-force solution count (capped at 2); 1 = unique
    stuck: Vec<usize>, // unsolved question indices (0-based)
    types: Vec<String>,
    answers: Vec<char>,
    link: String,
}

/// Sample `attempts` v2 composes for each selected level (`only_level` or all
/// six), seeded for reproducibility. Print per-level solved-count + cells-filled
/// histograms, then `count` no-progress + `count` partial example links.
pub fn stuck(seed: u32, attempts: usize, count: usize, only_level: Option<usize>, origin: &str) {
    let levels: Vec<usize> = match only_level {
        Some(l) => vec![l],
        None => (1..=6).collect(),
    };
    let mut rng = Rng::new(seed);
    let mut stats = Stats::default();
    let mut dists: Vec<Vec<u32>> = Vec::new(); // dists[i][solved], solved in 0..=n
    let mut filled_dists: Vec<Vec<u32>> = Vec::new(); // filled_dists[i][cells], 0..=n*oc
    let mut no_progress: Vec<Vec<StuckCase>> = levels.iter().map(|_| Vec::new()).collect();
    let mut partial: Vec<Vec<StuckCase>> = levels.iter().map(|_| Vec::new()).collect();

    for (i, &lvl) in levels.iter().enumerate() {
        let level = lvl - 1; // 0-based index into PROFILES / RECIPES
        let p = &PROFILES[level];
        let (n, oc) = (p.question_count, p.option_count);
        let real_mask = ((1u16 << oc) - 1) as u8; // option slots that aren't phantom
        let mut dist = vec![0u32; n + 1];
        let mut filled_dist = vec![0u32; n * oc + 1];
        for _ in 0..attempts {
            let c = compose(&RECIPES[level], n, oc, &mut rng, &mut stats.v2_compose);
            let mut fp = fill_options(&c.types, &c.solution, c.n, oc, &mut rng, false);
            // `solved` = questions pinned to one answer; `filled` = resolved cells
            // (eliminated options + the known-correct cell of solved questions).
            let (solved, filled) = match validate_and_repair(
                &mut fp,
                &c.solution,
                c.n,
                RECIPES[level].lookahead_depth,
                &mut rng,
                &mut stats,
                false,
                "stuck",
            ) {
                Verdict::Accepted => (c.n, c.n * oc),
                Verdict::Stuck { solved, state } => {
                    let filled: usize = (0..c.n)
                        .map(|qi| {
                            if state.answers[qi].is_some() {
                                oc
                            } else {
                                (state.eliminated[qi] & real_mask).count_ones() as usize
                            }
                        })
                        .sum();
                    let bucket = if solved == 0 {
                        &mut no_progress[i]
                    } else {
                        &mut partial[i]
                    };
                    if bucket.len() < count {
                        bucket.push(StuckCase {
                            level: lvl,
                            solved,
                            n: c.n,
                            filled,
                            total: c.n * oc,
                            solutions: solve(&fp, None, 2).len(),
                            stuck: (0..c.n).filter(|&qi| state.answers[qi].is_none()).collect(),
                            // Read types from `fp`, not `c.types`: a kept move-#2
                            // graft rewrites a rule, so `fp` is the repaired truth.
                            types: (0..c.n)
                                .map(|qi| format!("{:?}", fp.question_types[qi]))
                                .collect(),
                            answers: (0..c.n).map(|qi| c.solution[qi].as_char()).collect(),
                            link: playground_link(origin, &fp.question_types, &fp, &state),
                        });
                    }
                    (solved, filled)
                }
            };
            dist[solved] += 1;
            filled_dist[filled] += 1;
        }
        dists.push(dist);
        filled_dists.push(filled_dist);
    }

    let color = std::io::stdout().is_terminal();
    print_histogram(&levels, &dists, color);
    print_fill(&levels, &filled_dists, color);
    print_bucket("NO-PROGRESS (engine solved 0)", &pool(&no_progress, count));
    print_bucket(
        "PARTIAL-PROGRESS (engine stalled mid-solve)",
        &pool(&partial, count),
    );
    eprintln!("\n{attempts} composes/level, seed {seed}");
}

/// `L{lvl}: {pct}% {solved}/{n}; …` for solved 0..=n — the `n/n` bucket is the
/// acceptance rate (engine solve + repair). Stuck cases report the engine's
/// progress on the post-repair puzzle.
fn print_histogram(levels: &[usize], dists: &[Vec<u32>], color: bool) {
    println!("=== QUESTIONS SOLVED (engine + repair) ===");
    for (i, dist) in dists.iter().enumerate() {
        let total: u32 = dist.iter().sum();
        let n = dist.len() - 1;
        let parts: Vec<String> = (0..=n)
            .map(|s| {
                let pct = if total == 0 {
                    0.0
                } else {
                    dist[s] as f64 / total as f64 * 100.0
                };
                bucket(pct, &format!("{s}/{n}"), color)
            })
            .collect();
        println!("L{}: {}", levels[i], parts.join("; "));
    }
}

/// A `{pct}% {label}` histogram cell, dimmed when it rounds to 0% (noise) and
/// bolded above 5% (notable). Plain when stdout isn't a terminal so redirected
/// output stays free of escape codes.
fn bucket(pct: f64, label: &str, color: bool) -> String {
    let cell = format!("{pct:.0}% {label}");
    if !color {
        cell
    } else if pct < 0.5 {
        format!("\x1b[2m{cell}\x1b[0m") // dim — noise
    } else if pct > 20.0 {
        format!("\x1b[1;33m{cell}\x1b[0m") // bold yellow — dominant spike
    } else if pct > 5.0 {
        format!("\x1b[33m{cell}\x1b[0m") // yellow — notable
    } else {
        cell
    }
}

/// Full per-level distribution of resolved cells (0..=n·oc) with the mean
/// appended. The `0/N` bucket is the inert rate; distinguishes "no entry point
/// at all" from "lots eliminated but no question finished".
fn print_fill(levels: &[usize], dists: &[Vec<u32>], color: bool) {
    println!("\n=== CELLS FILLED (engine + repair; 0/N = inert) ===");
    for (i, dist) in dists.iter().enumerate() {
        let samples: u32 = dist.iter().sum();
        let cells = dist.len() - 1;
        let parts: Vec<String> = (0..=cells)
            .map(|c| {
                let pct = if samples == 0 {
                    0.0
                } else {
                    dist[c] as f64 / samples as f64 * 100.0
                };
                bucket(pct, &format!("{c}/{cells}"), color)
            })
            .collect();
        let weighted: u64 = (0..=cells).map(|c| c as u64 * dist[c] as u64).sum();
        let avg = if samples == 0 {
            0.0
        } else {
            weighted as f64 / samples as f64
        };
        println!("L{}: {}  | avg {avg:.1}", levels[i], parts.join("; "));
    }
}

/// Round-robin across per-level lists so the pooled output stays mixed.
fn pool(by_level: &[Vec<StuckCase>], count: usize) -> Vec<&StuckCase> {
    let mut out: Vec<&StuckCase> = Vec::new();
    let mut idx = 0;
    loop {
        let before = out.len();
        for level in by_level {
            if let Some(case) = level.get(idx) {
                out.push(case);
                if out.len() >= count {
                    return out;
                }
            }
        }
        if out.len() == before {
            return out; // all lists exhausted
        }
        idx += 1;
    }
}

fn print_bucket(title: &str, cases: &[&StuckCase]) {
    println!("\n=== {title} — {} shown ===", cases.len());
    for c in cases {
        let stuck: Vec<String> = c.stuck.iter().map(|qi| format!("Q{}", qi + 1)).collect();
        let uniqueness = match c.solutions {
            1 => "unique".to_string(),
            s => format!("AMBIGUOUS ({s}+ solutions)"),
        };
        println!(
            "\nL{}  {}/{} solved  {}/{} filled  {}  stuck: {}",
            c.level,
            c.solved,
            c.n,
            c.filled,
            c.total,
            uniqueness,
            stuck.join(" ")
        );
        println!("  {}", c.link);
        for qi in 0..c.n {
            let mark = if c.stuck.contains(&qi) { '*' } else { ' ' };
            println!(
                "    {mark}Q{:<2} = {}  {}",
                qi + 1,
                c.answers[qi],
                c.types[qi]
            );
        }
    }
}
