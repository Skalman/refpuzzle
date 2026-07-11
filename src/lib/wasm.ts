import init, {
  Puzzle as WasmPuzzle,
  generatePuzzle as wasmGeneratePuzzle,
} from "../../rust/pkg/refpuzzle.js";
import type { Puzzle, RenderedQuestion, Marks, Answer } from "../engine/types.ts";
import { L2I } from "../engine/types.ts";
import type { SolveStep } from "../engine/hint-types.ts";
import type { Validity } from "../engine/state.ts";
import {
  V_NEUTRAL,
  V_VALID,
  V_CONSISTENT,
  V_INVALID,
  V_PENDING,
  deriveState,
} from "../engine/state.ts";
import type { CompactPuzzle } from "../puzzles/daily.ts";
import { parseCompactPuzzle } from "../puzzles/daily.ts";

let wasmReadyPromise: Promise<unknown> | null = null;

/**
 * Kicks off wasm init on first call and returns a promise that resolves
 * once the wasm module is usable. Callers should `await` it before
 * constructing a {@link PuzzleHandle} or calling {@link generatePuzzle}.
 */
export function wasmReady(): Promise<unknown> {
  wasmReadyPromise ??= init();
  return wasmReadyPromise;
}

function validityFromU8(v: number): Validity {
  switch (v) {
    case 0:
      return V_NEUTRAL;
    case 1:
      return V_VALID;
    case 2:
      return V_CONSISTENT;
    case 3:
      return V_INVALID;
    case 4:
      return V_PENDING;
    default:
      return V_NEUTRAL;
  }
}

const handleRegistry = new FinalizationRegistry<WasmPuzzle>((wasm) => wasm.free());

export interface PuzzleHandle {
  checkAllAnswers(marks: Marks[], optionCount: number): Validity[];
  solve(): (Answer | null)[];
  /** The next solving step — a deduction, else the shortest lookahead
   *  contradiction — plus its explanation, from the derived state directly.
   *  Null when the puzzle is solved or stuck. The hint UI renders `explain`;
   *  the tutorial also applies `action` to walk the puzzle synthetically. */
  nextStep(answers: (Answer | null)[], eliminated: number[]): SolveStep | null;
  /** Rendered board text — prompt + option labels for every question. */
  renderBoard(): RenderedQuestion[];
  free(): void;
}

function answerIndicesFromMarks(marks: Marks[], optionCount: number) {
  const { answers, eliminated } = deriveState(marks, optionCount);
  // Rust's Answer enum serializes as u8 (A=0..E=4); convert before sending.
  return { answers: answers.map((a) => (a === null ? null : L2I[a])), eliminated };
}

export function createPuzzleHandle(compact: CompactPuzzle): PuzzleHandle {
  const wasm = new WasmPuzzle(JSON.stringify(compact));
  const wrapper: PuzzleHandle = {
    checkAllAnswers(marks, optionCount) {
      const state = answerIndicesFromMarks(marks, optionCount);
      const out = wasm.checkAllAnswers(state);
      const result: Validity[] = new Array(out.length);
      for (let i = 0; i < out.length; i++) result[i] = validityFromU8(out[i]);
      return result;
    },
    solve() {
      // oxlint-disable-next-line typescript/no-unsafe-type-assertion
      return wasm.solve() as (Answer | null)[];
    },
    nextStep(answers, eliminated) {
      const answerIndices = answers.map((a) => (a === null ? null : L2I[a]));
      // oxlint-disable-next-line typescript/no-unsafe-type-assertion
      return wasm.nextStep({ answers: answerIndices, eliminated }) as SolveStep | null;
    },
    renderBoard() {
      // oxlint-disable-next-line typescript/no-unsafe-type-assertion
      return wasm.renderBoard() as RenderedQuestion[];
    },
    free() {
      handleRegistry.unregister(wrapper);
      wasm.free();
    },
  };
  handleRegistry.register(wrapper, wasm, wrapper);
  return wrapper;
}

/**
 * Generate one puzzle on the fly. `seed` is any u32; `level` is 1..6.
 * Returns a rendered {@link Puzzle} (board text cached) or null if generation
 * failed. Caller must already have awaited {@link wasmReady}.
 */
export function generatePuzzle(seed: number, level: number, id: string): Puzzle | null {
  try {
    const json = wasmGeneratePuzzle(seed, level);
    // oxlint-disable-next-line typescript/no-unsafe-type-assertion
    const compact = JSON.parse(json) as CompactPuzzle;
    const p = parseCompactPuzzle(compact);
    return { ...p, id };
  } catch {
    return null;
  }
}
