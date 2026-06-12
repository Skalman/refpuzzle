use crate::types::Answer;

pub struct Rng {
    s: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        Rng { s: seed }
    }

    pub fn next_u32(&mut self) -> u32 {
        self.s = self.s.wrapping_add(0x6d2b79f5);
        let mut t = self.s;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        t ^ (t >> 14)
    }

    pub fn next_f64(&mut self) -> f64 {
        self.next_u32() as f64 / 4294967296.0
    }

    pub fn int(&mut self, min: i32, max: i32) -> i32 {
        min + (self.next_f64() * (max - min + 1) as f64) as i32
    }

    pub fn pick<T: Copy>(&mut self, arr: &[T]) -> T {
        arr[self.int(0, arr.len() as i32 - 1) as usize]
    }

    pub fn pick_kv<T: Copy>(&mut self, arr: &[T]) -> (usize, T) {
        let k = self.int(0, arr.len() as i32 - 1) as usize;
        (k, arr[k])
    }

    /// Uniform random `Answer` from the first `oc` letters
    /// (the puzzle's active option range, `LETTERS[..oc]`).
    pub fn pick_letter(&mut self, oc: usize) -> Answer {
        self.pick_letter_from(0, oc)
    }

    /// Uniform random `Answer` from `LETTERS[start..end]` (half-open).
    pub fn pick_letter_from(&mut self, start: usize, end: usize) -> Answer {
        debug_assert!(start < end && end <= 5);
        Answer::from(self.int(start as i32, end as i32 - 1) as u8)
    }

    pub fn state(&self) -> u32 {
        self.s
    }

    pub fn shuffle<T>(&mut self, arr: &mut [T]) {
        for i in (1..arr.len()).rev() {
            let j = self.int(0, i as i32) as usize;
            arr.swap(i, j);
        }
    }
}
