import init, {
  Puzzle as WasmPuzzle,
  generatePuzzle as wasmGeneratePuzzle,
} from "../../rust/pkg/refpuzzle.js";
import type { Puzzle, QuestionType, Marks, Answer } from "../engine/types.ts";
import { L2I } from "../engine/types.ts";
import type { Validity } from "../engine/state.ts";
import {
  V_NEUTRAL,
  V_VALID,
  V_CONSISTENT,
  V_INVALID,
  V_PENDING,
  deriveState,
} from "../engine/state.ts";
import type { CompactQuestionType, CompactPuzzle } from "../puzzles/daily.ts";
import { parseCompactPuzzle } from "../puzzles/daily.ts";

let wasmReadyPromise: Promise<unknown> | null = null;
let initDone = false;

/**
 * Kicks off wasm init on first call and returns a promise that resolves
 * once the wasm module is usable. Callers should `await` it before
 * constructing a {@link PuzzleHandle} or calling {@link generatePuzzle}.
 */
export function wasmReady(): Promise<unknown> {
  wasmReadyPromise ??= init().then((v) => {
    initDone = true;
    return v;
  });
  return wasmReadyPromise;
}

export function isWasmReady(): boolean {
  return initDone;
}

// Inverse of expandQuestion in daily.ts: compact the TS QuestionType back to
// the on-disk shape that Rust's parse_puzzle accepts.
function compactQuestion(qt: QuestionType): CompactQuestionType {
  const t = qt.type;
  switch (qt.type) {
    case "CountVowel":
    case "CountConsonant":
    case "MostCommonCount":
    case "PrevSame":
    case "NextSame":
    case "OnlySame":
    case "SameAs":
    case "ConsecIdent":
    case "LeastCommon":
    case "MostCommon":
    case "NoOtherHasAnswer":
    case "AnswerIsSelf":
    case "TrueStmt":
      return { t };
    case "CountAnswer":
    case "FirstWith":
    case "LastWith":
    case "OnlyOdd":
    case "OnlyEven":
    case "EqualCount":
      return { t, a: L2I[qt.answer] };
    case "CountAnswerAfter":
    case "ClosestAfter":
      return { t, a: L2I[qt.answer], q: qt.afterIndex };
    case "CountAnswerBefore":
    case "ClosestBefore":
      return { t, a: L2I[qt.answer], q: qt.beforeIndex };
    case "AnswerOf":
    case "LetterDist":
    case "SameAsWhich":
      return { t, q: qt.questionIndex };
    default: {
      qt satisfies never;
      // oxlint-disable-next-line typescript/restrict-template-expressions
      throw new Error(`Unknown question type: ${(qt as { type: string }).type}`);
    }
  }
}

function toCompactPuzzle(p: Puzzle): CompactPuzzle {
  const q = p.questions.map((qd) => compactQuestion(qd.questionType));
  const o = p.questions.map((qd) => qd.options.map((opt) => opt.value));
  const t = p.trueStmtQuestionTypes?.map(compactQuestion);
  return t ? { q, o, t } : { q, o };
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
  free(): void;
}

export function createPuzzleHandle(p: Puzzle): PuzzleHandle {
  const json = JSON.stringify(toCompactPuzzle(p));
  const wasm = new WasmPuzzle(json);
  const wrapper: PuzzleHandle = {
    checkAllAnswers(marks, optionCount) {
      const { answers, eliminated } = deriveState(marks, optionCount);
      // Rust's Answer enum serializes as u8 (A=0..E=4); convert before sending.
      const answerIndices = answers.map((a) => (a === null ? null : L2I[a]));
      const out = wasm.checkAllAnswers({ answers: answerIndices, eliminated });
      const result: Validity[] = new Array(out.length);
      for (let i = 0; i < out.length; i++) result[i] = validityFromU8(out[i]);
      return result;
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
 * Returns a fully expanded {@link Puzzle} or null if generation failed.
 * Caller must already have awaited {@link wasmReady}.
 */
export function generatePuzzle(seed: number, level: number, id: string): Puzzle | null {
  try {
    const json = wasmGeneratePuzzle(seed, level);
    // oxlint-disable-next-line typescript/no-unsafe-type-assertion
    const compact = JSON.parse(json) as CompactPuzzle;
    const p = parseCompactPuzzle(compact);
    return { ...p, id, difficulty: String(level) };
  } catch {
    return null;
  }
}

// Re-export Answer for callers that want the union type
export type { Answer };
