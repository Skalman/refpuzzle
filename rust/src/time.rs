//! Timing shim for the `--stats` microsecond counters. Native uses
//! `std::time::Instant`; wasm32-unknown-unknown has no clock, so the wasm build
//! is a zero-cost no-op that always reports 0.

#[cfg(not(target_arch = "wasm32"))]
pub(crate) type WasmInstant = std::time::Instant;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn wasm_now() -> WasmInstant {
    std::time::Instant::now()
}
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn us(t: WasmInstant) -> u64 {
    t.elapsed().as_micros() as u64
}

#[cfg(target_arch = "wasm32")]
#[derive(Copy, Clone)]
pub(crate) struct WasmInstant;
#[cfg(target_arch = "wasm32")]
pub(crate) fn wasm_now() -> WasmInstant {
    WasmInstant
}
#[cfg(target_arch = "wasm32")]
pub(crate) fn us(_t: WasmInstant) -> u64 {
    0
}
