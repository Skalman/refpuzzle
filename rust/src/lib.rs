#![allow(
    clippy::needless_range_loop,
    clippy::len_without_is_empty,
    clippy::new_without_default,
    clippy::should_implement_trait
)]

pub mod build;
pub mod check_answer;
pub mod check_form;
pub mod construct;
pub mod deduce;
pub mod difficulty;
pub mod format;
pub mod lookahead;
pub mod rng;
pub mod serialize;
pub mod solve_brute;
pub mod solve_deduce;
pub mod types;
