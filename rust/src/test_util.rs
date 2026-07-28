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

/// Whether `fp` has a fatal form error — the engine's precondition (see the
/// `check_answer` module doc). A fuzz builder that assembles rows itself, without
/// `construct::random_type_params`' pool-size gating, has to skip these.
pub(crate) fn form_invalid(fp: &crate::types::FlatPuzzle) -> bool {
    crate::check_form::check_form(fp)
        .iter()
        .any(|e| e.severity == crate::check_form::Severity::Error)
}
