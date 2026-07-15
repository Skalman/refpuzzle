//! CLI subcommand handlers for the `refpuzzle` binary. Bin-only (never compiled
//! into the wasm library): each orchestrates the crate-root engine modules and
//! handles argument-driven I/O for one subcommand.

pub mod check;
pub mod diagnose;
pub mod type_stats;
