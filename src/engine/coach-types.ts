/**
 * Types for the L1 in-play coach: the ambient teaching layer that replaces the
 * old auto-solve tutorial. `ArrowReferent` mirrors Rust's `render::ArrowReferent`
 * (via the wasm `referents()` call) — what a hint arrow points at for one
 * question, resolved from the question type alone.
 */

/** A column clip: rows above (`side < 0`) or below (`side > 0`) question `qi`. */
export interface ArrowBoundary {
  qi: number;
  side: number;
}

/**
 * What an arrow points at for a question. Wire shape from Rust (tagged `kind`);
 * see `rust/src/render.rs::ArrowReferent`. `null` for kinds L1 never uses.
 */
export type ArrowReferent =
  | { kind: "column"; oi: number; boundary?: ArrowBoundary }
  | { kind: "question"; qi: number }
  | { kind: "scan"; qi: number; dir: number; oi: number }
  | { kind: "sameRun"; dir: number }
  | { kind: "candidates"; qis: number[] }
  | { kind: "tally" };

/**
 * How the coach draws attention. `point` says "look here" at one or more
 * questions (or, with `oi`, a specific option cell in each); `connector` shows a
 * question's referent — "this refers to that".
 */
export type ArrowSpec =
  | { mode: "point"; qis: number[]; oi?: number }
  | { mode: "connector"; qi: number; referent: ArrowReferent };

/**
 * One thing the coach is saying right now: a calm line plus an optional arrow.
 * `tone` lets a mistake note read a touch more alert than an idle nudge. An
 * optional `lead` renders on its own line above `text` (e.g. "Here's one…").
 */
export interface CoachMessage {
  text: string;
  lead?: string;
  arrow: ArrowSpec | null;
  tone: "calm" | "alert";
  /**
   * Consecutive messages sharing a non-empty `arrowKey` keep the same arrow
   * elements, so the arrow transitions (moves) between them instead of
   * redrawing — used to morph a mistake flag into its sharpened escalation.
   */
  arrowKey?: string;
}
