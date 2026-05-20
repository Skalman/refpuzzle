import type { CompactPuzzle } from "../puzzles/daily.ts";
import type { Marks } from "../engine/types.ts";
import { FRESH_MARKS } from "../engine/types.ts";
import type { QuestionState, SavedState } from "./store.ts";
import { cloneStates, encodeHistory } from "./store.ts";

// Build a SavedState whose history is the sequence of single-mark changes
// that leads to `marks`. encodeHistory's diffAction only emits one mark per
// step, so the history must be granular.
export function savedStateFromMarks(marks: Marks[]): SavedState {
  const current: QuestionState[] = marks.map(() => ({ marks: [...FRESH_MARKS] as Marks }));
  const history: QuestionState[][] = [cloneStates(current)];
  for (let qi = 0; qi < marks.length; qi++) {
    for (let oi = 0; oi < 5; oi++) {
      if (marks[qi][oi] !== "unmarked") {
        current[qi].marks[oi] = marks[qi][oi];
        history.push(cloneStates(current));
      }
    }
  }
  return {
    questions: history[history.length - 1],
    completed: false,
    stale: false,
    history,
    historyIdx: history.length - 1,
    hints: new Map(),
  };
}

// Hash format: p=<url-safe-base64(deflate(json))>[&h=<encoded-play-state>]

async function deflate(str: string): Promise<Uint8Array> {
  const cs = new CompressionStream("deflate-raw");
  const writer = cs.writable.getWriter();
  await writer.write(new TextEncoder().encode(str));
  await writer.close();
  return new Uint8Array(await new Response(cs.readable).arrayBuffer());
}

async function inflate(bytes: Uint8Array): Promise<string> {
  const ds = new DecompressionStream("deflate-raw");
  const decompressed = new Blob([new Uint8Array(bytes)]).stream().pipeThrough(ds);
  return new TextDecoder().decode(await new Response(decompressed).arrayBuffer());
}

function toBase64Url(bytes: Uint8Array): string {
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=/g, "");
}

function fromBase64Url(str: string): Uint8Array {
  const padded = str
    .replace(/-/g, "+")
    .replace(/_/g, "/")
    .padEnd(str.length + ((4 - (str.length % 4)) % 4), "=");
  const binary = atob(padded);
  return Uint8Array.from({ length: binary.length }, (_, i) => binary.charCodeAt(i));
}

export async function encodePlaygroundHash(
  compact: CompactPuzzle,
  state?: SavedState,
): Promise<string> {
  const p = toBase64Url(await deflate(JSON.stringify(compact)));
  const h = state ? encodeHistory(state) : null;
  return h ? `p=${p}&h=${h}` : `p=${p}`;
}

export async function decodePlaygroundHash(
  hash: string,
): Promise<{ compact: CompactPuzzle; stateHash: string | null } | null> {
  const params = new URLSearchParams(hash);
  const puzzlePart = params.get("p");
  const stateHash = params.get("h");
  if (!puzzlePart) return null;
  try {
    const json = await inflate(fromBase64Url(puzzlePart));
    // oxlint-disable-next-line typescript/no-unsafe-type-assertion
    const compact = JSON.parse(json) as CompactPuzzle;
    if (!compact?.q || !Array.isArray(compact.q)) return null;
    return { compact, stateHash };
  } catch {
    return null;
  }
}
