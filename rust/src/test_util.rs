//! Shared helpers for the test suites.

#![cfg(test)]

/// Gate for slow tests. `REFPUZZLE_FAST_TESTS` set → reduced fast run (true); an
/// optimized build without it → full run (false); an unoptimized build without
/// it → panic, since the full run would take minutes.
pub(crate) fn fast_tests() -> bool {
    let fast = std::env::var("REFPUZZLE_FAST_TESTS").is_ok();
    assert!(
        fast || !cfg!(debug_assertions),
        "slow test — run with --release or set REFPUZZLE_FAST_TESTS=1"
    );
    fast
}

/// Fuzz-loop time budget derived from [`fast_tests`].
pub(crate) fn slow_test_duration() -> std::time::Duration {
    if fast_tests() {
        std::time::Duration::from_millis(200)
    } else {
        std::time::Duration::from_secs(5)
    }
}

/// Base seed for a fuzz loop, random per run so repeated runs (CI included) explore
/// new ground instead of re-checking one prefix. `<var>=<n>` pins it to replay a catch;
/// the caller prints the value it used.
// This module is compiled into both the lib and the bin; the only caller so far is a
// bin-side test, so the lib's copy is unused.
#[allow(dead_code)]
pub(crate) fn fuzz_base_seed(var: &str) -> u32 {
    std::env::var(var)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        })
}

/// Whether `fp` has a fatal form error — the engine's precondition (see the
/// `check_answer` module doc). A fuzz builder that assembles rows itself, without
/// `fill::random_type_params`' pool-size gating, has to skip these.
pub(crate) fn form_invalid(fp: &crate::types::FlatPuzzle) -> bool {
    crate::check_form::check_form(fp)
        .iter()
        .any(|e| e.severity == crate::check_form::Severity::Error)
}
