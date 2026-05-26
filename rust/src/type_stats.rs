use crate::build::fill_options;
use crate::construct::flat_construct;
use crate::rng::Rng;
use crate::solve_brute::solve;
use crate::types::QuestionTypeKind;

const SIZES: &[(usize, usize)] = &[(3, 3), (4, 5), (5, 5), (8, 5), (10, 5), (12, 5)];

pub fn type_stats(attempts: u32, seed: u32) {
    let kinds = QuestionTypeKind::all_flat();
    let n_kinds = kinds.len();

    let mut rng = Rng::new(seed);

    for &(n, oc) in SIZES {
        // histogram[ki][count] = number of successful puzzles where kind ki appeared `count` times
        let mut histogram = vec![vec![0u32; n + 1]; n_kinds];
        let mut successes = 0u32;

        for _ in 0..attempts {
            let Some((types, solution)) = flat_construct(n, oc, &mut rng) else {
                continue;
            };
            let Some(fp) = fill_options(&types, &solution, n, oc, &mut rng, false) else {
                continue;
            };
            if solve(&fp, None, 2).len() != 1 {
                continue;
            }
            successes += 1;

            let mut counts = [0usize; 32];
            for qt in &types[..n] {
                counts[qt.kind() as u8 as usize] += 1;
            }
            for (i, kind) in kinds.iter().enumerate() {
                let c = counts[*kind as u8 as usize].min(n);
                histogram[i][c] += 1;
            }
        }

        let yield_pct = 100.0 * successes as f64 / attempts as f64;
        eprintln!(
            "=== n={n} oc={oc} ({attempts} attempts, {successes} successes, {yield_pct:.1}%) ==="
        );

        let mut indexed: Vec<(QuestionTypeKind, usize)> = kinds.iter().copied().zip(0..).collect();
        indexed.sort_by_key(|(k, _)| format!("{k:?}"));

        for (kind, i) in &indexed {
            if histogram[*i].iter().all(|&c| c == 0) {
                continue;
            }
            let cols: Vec<String> = (0..=n)
                .map(|c| format!("{c}x = {}", histogram[*i][c]))
                .collect();
            eprintln!("{kind:?}: {}", cols.join("; "));
        }
        eprintln!();
    }
}
