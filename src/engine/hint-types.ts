import type { Answer } from "./types.ts";

export type DeduceAction =
  | { type: "force"; qi: number; answer: Answer }
  | { type: "eliminate"; qi: number; oi: number }
  | { type: "eliminateMulti"; questionMask: number; optionMask: number };

/**
 * One rendered hint step, produced by the Rust explain layer (via wasm) and
 * rendered by `HintStep`: a single line, a headed block of lines, or a "look at
 * these questions" pointer. For `look`, Rust sends only the 0-based question
 * indices; the frontend builds the "Try looking at …" sentence (`hint.tryLooking`).
 */
export type ExplainStep =
  | { type: "simple"; text: string }
  | { type: "complex"; header: string; lines: string[] }
  | { type: "look"; qis: number[] };

/**
 * One solving step plus its rendered explanation — the unit the hint UI renders
 * (`explain`). The step is a deduction, or a lookahead elimination.
 */
export interface SolveStep {
  action: DeduceAction;
  explain: ExplainStep[];
  /** 0-based questions to look at, for the L1 coach's arrows. */
  focusQis: number[];
}
