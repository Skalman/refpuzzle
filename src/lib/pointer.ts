/** The primary pointer's granularity — `coarse` (touch/finger) or `fine` (mouse). */
export type PointerKind = "coarse" | "fine";

/**
 * The primary pointer, so UI copy can say "tap" (coarse) vs "click" (fine).
 * Keys off the primary pointer, so a touch laptop with a trackpad reads `fine`.
 */
export function pointerKind(): PointerKind {
  return matchMedia("(pointer: coarse)").matches ? "coarse" : "fine";
}
