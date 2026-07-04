/// Per-level board dimensions. The question mix (which kinds appear, caps,
/// lookahead depth) lives in [`crate::construct::RECIPES`]; this is only the
/// board size, shared by generation, the wasm runtime, and the stats tools.
pub struct DifficultyProfile {
    pub question_count: usize,
    pub option_count: usize,
}

const fn profile(question_count: usize, option_count: usize) -> DifficultyProfile {
    DifficultyProfile {
        question_count,
        option_count,
    }
}

/// Indexed by level - 1.
pub static PROFILES: [DifficultyProfile; 6] = [
    profile(3, 3),  // L1 (intro)
    profile(4, 5),  // L2 (beginner)
    profile(5, 5),  // L3
    profile(8, 5),  // L4
    profile(10, 5), // L5
    profile(12, 5), // L6
];
