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
import { trackFatalError } from "./analytics.ts";

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

// Inverse of `lib.rs::validity_to_u8`; the encoding is documented on the Rust
// `Validity` enum (check_answer.rs). Keep all three in sync.
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

let fatalReported = false;

/**
 * Runs a wasm call. A Rust panic (`panic="abort"`) traps as a RuntimeError (message
 * only "unreachable" — `op`/`puzzle`/`params` are the real diagnostics); it doesn't
 * poison the instance, so surface the #fatal bug UI once + log + rethrow rather than
 * latch the engine off. A non-RuntimeError (malformed-puzzle JsError) passes through.
 */
function tryWasm<T>(ctx: { op: string; puzzle?: string; params?: unknown }, call: () => T): T {
  try {
    return call();
  } catch (e) {
    if (e instanceof WebAssembly.RuntimeError && !fatalReported) {
      fatalReported = true;
      if (import.meta.env.PROD) {
        trackFatalError(e, "wasm_panicked", { op: ctx.op, puzzle: ctx.puzzle, params: ctx.params });
      }
      window.showFatalError?.(ctx.puzzle ? `${ctx.op} @ ${ctx.puzzle}` : ctx.op, {
        title: "The puzzle engine hit a bug",
        body:
          "Something in the puzzle engine failed — a bug, not your device. It's been " +
          "logged for the developers. Reloading may clear it.",
      });
    }
    throw e;
  }
}

export function createPuzzleHandle(compact: CompactPuzzle, puzzle?: string): PuzzleHandle {
  const wasm = tryWasm({ op: "new", puzzle }, () => new WasmPuzzle(JSON.stringify(compact)));
  const wrapper: PuzzleHandle = {
    checkAllAnswers(marks, optionCount) {
      const state = answerIndicesFromMarks(marks, optionCount);
      return tryWasm({ op: "checkAllAnswers", puzzle, params: state }, () => {
        const out = wasm.checkAllAnswers(state);
        const result: Validity[] = new Array(out.length);
        for (let i = 0; i < out.length; i++) result[i] = validityFromU8(out[i]);
        return result;
      });
    },
    solve() {
      // oxlint-disable-next-line typescript/no-unsafe-type-assertion
      return tryWasm({ op: "solve", puzzle }, () => wasm.solve() as (Answer | null)[]);
    },
    nextStep(answers, eliminated) {
      const answerIndices = answers.map((a) => (a === null ? null : L2I[a]));
      const state = { answers: answerIndices, eliminated };
      return tryWasm({ op: "nextStep", puzzle, params: state }, () => {
        // oxlint-disable-next-line typescript/no-unsafe-type-assertion
        return wasm.nextStep(state) as SolveStep | null;
      });
    },
    renderBoard() {
      return tryWasm({ op: "renderBoard", puzzle }, () => {
        // oxlint-disable-next-line typescript/no-unsafe-type-assertion
        return wasm.renderBoard() as RenderedQuestion[];
      });
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
 * Generate one puzzle on the fly. `dateKey` is `year*10000 + mm*100 + dd`; `level` is
 * 1..6; `id` is the `/date/level` path. Rust derives the RNG seed from `(dateKey, level)`
 * (see `rng::daily_seed`), so this matches the baked corpus. Returns a rendered
 * {@link Puzzle}, or null if generation recoverably failed (budget exhausted, bad JSON).
 * Caller must have awaited {@link wasmReady}. A Rust panic surfaces the fatal UI via
 * {@link tryWasm}.
 */
export function generatePuzzle(dateKey: number, level: number, id: string): Puzzle | null {
  try {
    const json = tryWasm({ op: "generate", puzzle: id, params: { dateKey, level } }, () =>
      wasmGeneratePuzzle(dateKey, level),
    );
    // oxlint-disable-next-line typescript/no-unsafe-type-assertion
    const compact = JSON.parse(json) as CompactPuzzle;
    return { ...parseCompactPuzzle(compact), id };
  } catch {
    return null;
  }
}
