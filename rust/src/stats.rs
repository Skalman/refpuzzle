//! Generation telemetry, accumulated across `generate()` attempts and
//! reported by the CLI's `--stats` flag.

/// Where the planned question mix couldn't be honored during skeleton
/// generation. In the assign phase a leftover slot becomes AnswerOf
/// (`assign_kinds`); in the parametrize phase the planned kind is either swapped
/// for another fitting pool kind (`reserve`) or, failing that, demoted to a
/// generic AnswerOf (`backstop`). Only `assign_kinds`/`backstop` are AnswerOf
/// demotions; `reserve` is a same-tier kind swap.
#[derive(Default)]
pub struct FallbackCounts {
    /// Assign phase: a kind fit no free slot, so a leftover slot became AnswerOf.
    pub assign_kinds: u32,
    /// Parametrize phase: the planned kind didn't fit, replaced by a different
    /// fitting kind from the level's pool.
    pub reserve: u32,
    /// Parametrize phase: neither the planned kind nor any pool reserve fit, so
    /// the slot fell back to a generic AnswerOf.
    pub backstop: u32,
}

/// `generate_skeleton` telemetry, accumulated across every attempt: how many
/// skeletons were generated and the AnswerOf fallbacks they incurred. `count` is
/// the denominator for per-skeleton fallback rates.
#[derive(Default)]
pub struct SkeletonStats {
    pub count: u32,
    pub fallbacks: FallbackCounts,
}

#[derive(Default)]
pub struct Stats {
    pub attempts: u32,
    pub fail_solve: u32,
    pub fail_solve_zero_progress: u32,
    pub deduce_calls: u32,
    pub deduce_results: u32,
    pub lookahead_calls: u32,
    pub lookahead_hits: u32,
    pub lookahead_us: u64,
    // deduce/deduce_fast calls made inside lookahead's probe loops. Not folded
    // into deduce_calls (which counts only the outer hint-loop `deduce`) so the
    // two propagation paths stay distinguishable.
    pub deduce_calls_in_lookahead: u32,
    // distractor-repair telemetry: `_attempts` counts questions probed for
    // repair (each probe may try several candidate edits); `_ok` counts probes
    // whose kept edit completed the puzzle and passed both re-check backstops.
    pub distractor_attempts: u32,
    pub distractor_ok: u32,
    // Distractor repairs that reported "solved" from the resume state but were
    // rejected by an independent from-scratch re-check, so the caller regenerates.
    // The resume-from-`state` optimization can carry an elimination a global rule
    // made on a since-edited option, so the emitted puzzle is re-verified two ways:
    // `_ambiguous` = brute found ≥2 solutions (not well-posed); `_unsolvable` =
    // unique, but the hint engine can't solve it from scratch.
    pub repair_ambiguous: u32,
    pub repair_unsolvable: u32,
    pub skeleton: SkeletonStats,
}

impl Stats {
    pub fn merge(&mut self, other: &Stats) {
        self.attempts += other.attempts;
        self.fail_solve += other.fail_solve;
        self.fail_solve_zero_progress += other.fail_solve_zero_progress;
        self.deduce_calls += other.deduce_calls;
        self.deduce_results += other.deduce_results;
        self.lookahead_calls += other.lookahead_calls;
        self.lookahead_hits += other.lookahead_hits;
        self.lookahead_us += other.lookahead_us;
        self.deduce_calls_in_lookahead += other.deduce_calls_in_lookahead;
        self.distractor_attempts += other.distractor_attempts;
        self.distractor_ok += other.distractor_ok;
        self.repair_ambiguous += other.repair_ambiguous;
        self.repair_unsolvable += other.repair_unsolvable;
        self.skeleton.count += other.skeleton.count;
        self.skeleton.fallbacks.assign_kinds += other.skeleton.fallbacks.assign_kinds;
        self.skeleton.fallbacks.reserve += other.skeleton.fallbacks.reserve;
        self.skeleton.fallbacks.backstop += other.skeleton.fallbacks.backstop;
    }

    /// Fold one `run_engine` invocation's loop telemetry into the running stats.
    pub fn merge_engine(&mut self, tel: &crate::solve_deduce::EngineTelemetry) {
        self.deduce_calls += tel.deduce_calls;
        self.deduce_results += tel.deduce_results;
        self.lookahead_calls += tel.lookahead_calls;
        self.lookahead_hits += tel.lookahead_hits;
        self.lookahead_us += tel.lookahead_us;
        self.deduce_calls_in_lookahead += tel.deduce_calls_in_lookahead;
    }

    pub fn print(&self) {
        let ok = self.attempts - self.fail_solve;
        eprintln!(
            "  attempts={} ok={} solve_fail={} (zero_progress={}) | deduce: {} calls, {} results | lookahead: {} calls, {} hits, {}ms ({} deduce calls)",
            self.attempts,
            ok,
            self.fail_solve,
            self.fail_solve_zero_progress,
            self.deduce_calls,
            self.deduce_results,
            self.lookahead_calls,
            self.lookahead_hits,
            self.lookahead_us / 1000,
            self.deduce_calls_in_lookahead,
        );
        eprintln!(
            "  skeletons={} | repair distractor={}/{} rejected(ambiguous={} unsolvable={}) | fallbacks: assign_kinds={} reserve={} backstop={}",
            self.skeleton.count,
            self.distractor_ok,
            self.distractor_attempts,
            self.repair_ambiguous,
            self.repair_unsolvable,
            self.skeleton.fallbacks.assign_kinds,
            self.skeleton.fallbacks.reserve,
            self.skeleton.fallbacks.backstop,
        );
    }
}
