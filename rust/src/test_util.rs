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
pub(crate) fn slow_test_duration() -> Option<std::time::Duration> {
    Some(if fast_tests() {
        std::time::Duration::from_millis(200)
    } else {
        std::time::Duration::from_secs(5)
    })
}
