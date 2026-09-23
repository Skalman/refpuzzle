import type { Marks } from "../engine/types.ts";
import { FRESH_MARKS } from "../engine/types.ts";

// The saved-state string format (v1 history tokens, the two-state ledger, and
// the frozen v0 grammar this module still decodes) is specified in
// docs/encoding.md. Shape rules that keep the parser one dispatch:
// actions start with a digit (plus `cp`), annotations are letters-then-digits,
// flags are bare stems in the ledger only.

export interface QuestionState {
  marks: Marks;
}

/**
 * A hint press at one step: the deepest level reached, and the question the
 * hint named. `qi` is null until a press names one, and never moves after.
 */
export interface HintMarker {
  level: number;
  qi: number | null;
}

/** Refused checkpoint presses at one step: how many, and the questions refused. */
export interface FailMarker {
  count: number;
  qis: number[];
}

export interface SavedState {
  questions: QuestionState[];
  completed: boolean;
  stale: boolean;
  history: QuestionState[][];
  historyIdx: number;
  hints: Map<number, HintMarker>;
  /** Refused checkpoint presses, keyed by history step. */
  fails: Map<number, FailMarker>;
}

export const PUZZLE_VERSION = 3;
/** Version of the saved-state string format; gates the boot sweep. */
const FORMAT_VERSION = 1;
const VERSION_KEY = "refpuzzle:version";
const PREFIX = "refpuzzle:puzzle:";
const LETTERS = ["A", "B", "C", "D", "E"];
const META_SEP = "|";

// ── Version bookkeeping ─────────────────────────────────────────────────────
// One querystring-shaped key holds both counters: `puzzle=3&format=1`.
// A legacy bare number is the puzzle version. The two fields have different
// writers in different phases, so all access goes through this pair.

export function getVersions(): { puzzle: number; format: number } {
  try {
    const raw = localStorage.getItem(VERSION_KEY);
    if (!raw) return { puzzle: 0, format: 0 };
    if (/^\d+$/.test(raw)) return { puzzle: Number(raw), format: 0 };
    const params = new URLSearchParams(raw);
    return {
      puzzle: Number(params.get("puzzle")) || 0,
      format: Number(params.get("format")) || 0,
    };
  } catch {
    return { puzzle: 0, format: 0 };
  }
}

export function setVersion(field: "puzzle" | "format", value: number): void {
  try {
    const versions = getVersions();
    versions[field] = value;
    localStorage.setItem(VERSION_KEY, `puzzle=${versions.puzzle}&format=${versions.format}`);
  } catch {}
}

// ── Value anatomy ───────────────────────────────────────────────────────────

function historyPart(raw: string): string {
  const i = raw.indexOf(META_SEP);
  return i >= 0 ? raw.slice(0, i) : raw;
}

function ledgerPart(raw: string): string {
  const i = raw.indexOf(META_SEP);
  return i >= 0 ? raw.slice(i + 1) : "";
}

function isV1Value(raw: string): boolean {
  const first = historyPart(raw).split(".", 1)[0];
  return /^v\d+$/.test(first);
}

/** The ledger's flags; stale is only meaningful beside solved. */
interface LedgerFlags {
  solved: boolean;
  stale: boolean;
}

/** The flags lead the ledger, so both reads are of its first tokens. */
function ledgerFlags(ledger: string): LedgerFlags {
  const tokens = ledger.split(".");
  return { solved: tokens[0] === "s", stale: tokens[1] === "st" };
}

/** The ledger's counter tokens, the leading flags dropped. */
function counterTokens(ledger: string): string[] {
  const tokens = ledger.split(".").filter((token) => token !== "");
  const flags = ledgerFlags(ledger);
  return tokens.slice((flags.solved ? 1 : 0) + (flags.stale ? 1 : 0));
}

/** Flags lead, `s` then `st`, and the counters follow in their own order. */
function composeLedger(flags: LedgerFlags, counters: string[]): string {
  const tokens: string[] = [];
  if (flags.solved) {
    tokens.push("s");
    if (flags.stale) tokens.push("st");
  }
  tokens.push(...counters);
  return tokens.join(".");
}

/** Whether a stored value records a solved puzzle. Assumes v1 (post-sweep). */
export function isSolvedValue(raw: string): boolean {
  return ledgerFlags(ledgerPart(raw)).solved;
}

// ── v1 history codec ────────────────────────────────────────────────────────

export function cloneStates(qs: QuestionState[]): QuestionState[] {
  return qs.map((q) => ({ marks: [...q.marks] as Marks }));
}

function blankStates(n: number): QuestionState[] {
  return Array.from({ length: n }, () => ({ marks: [...FRESH_MARKS] as Marks }));
}

/** The one cell a step changes, as a v1 action token; `cp` when nothing changed. */
function diffAction(prev: QuestionState[], next: QuestionState[]): string {
  for (let qi = 0; qi < prev.length; qi++) {
    for (let oi = 0; oi < 5; oi++) {
      if (prev[qi].marks[oi] !== next[qi].marks[oi]) {
        const q = qi + 1;
        const letter = LETTERS[oi];
        const to = next[qi].marks[oi];
        if (to === "incorrect") return `${q}${letter.toLowerCase()}`;
        if (to === "correct") return `${q}${letter}`;
        return `${q}${letter.toLowerCase()}-`;
      }
    }
  }
  return "cp";
}

/**
 * Encode the history segment (v1). The ledger is composed by `saveState`.
 * Marker questions are written 1-based, like the action tokens, each behind
 * its own `q`; a marker that named no question carries no suffix.
 */
export function encodeHistory(state: SavedState): string {
  const tokens = ["v1"];
  const annotate = (idx: number) => {
    const hint = state.hints.get(idx);
    if (hint) tokens.push(`h${hint.level}${hint.qi === null ? "" : `q${hint.qi + 1}`}`);
    const fail = state.fails.get(idx);
    if (fail?.count) {
      tokens.push(`cpx${fail.count}${fail.qis.map((qi) => `q${qi + 1}`).join("")}`);
    }
  };

  annotate(0);
  if (state.historyIdx === 0 && state.history.length > 1) tokens.push("_");
  for (let i = 1; i < state.history.length; i++) {
    tokens.push(diffAction(state.history[i - 1], state.history[i]));
    annotate(i);
    if (i === state.historyIdx && i !== state.history.length - 1) tokens.push("_");
  }
  return tokens.join(".");
}

/**
 * Decode a history segment of either version (the first token decides).
 * Only v0 carries completed/stale here; in v1 both live in the ledger, so
 * this returns them false and `loadState` fills them in.
 */
export function decodeHistory(encoded: string, n: number): SavedState | null {
  const tokens = encoded.split(".");
  return /^v\d+$/.test(tokens[0]) ? decodeV1(tokens, n) : decodeV0(tokens, n);
}

function decodeV1(tokens: string[], n: number): SavedState {
  const current = blankStates(n);
  const history = [cloneStates(current)];
  const hints = new Map<number, HintMarker>();
  const fails = new Map<number, FailMarker>();
  // -1 = no cursor token seen → cursor at the last step.
  let historyIdx = -1;

  /** The `q`-prefixed tail of a marker token, as 0-based questions. */
  const questions = (tail: string | undefined): number[] =>
    tail
      ? tail
          .split("q")
          .filter(Boolean)
          .map((digits) => Number(digits) - 1)
      : [];

  for (const token of tokens) {
    if (token === "_") {
      historyIdx = history.length - 1;
      continue;
    }
    const hint = /^h(\d+)((?:q\d+)*)$/.exec(token);
    if (hint) {
      const [qi] = questions(hint[2]);
      hints.set(history.length - 1, { level: Number(hint[1]), qi: qi ?? null });
      continue;
    }
    const fail = /^cpx(\d+)((?:q\d+)*)$/.exec(token);
    if (fail) {
      fails.set(history.length - 1, { count: Number(fail[1]), qis: questions(fail[2]) });
      continue;
    }
    if (token === "cp") {
      history.push(cloneStates(current));
      continue;
    }
    const action = /^(\d+)([a-eA-E])(-?)$/.exec(token);
    if (action) {
      const qi = Number(action[1]) - 1;
      const oi = LETTERS.indexOf(action[2].toUpperCase());
      if (qi < 0 || qi >= n || oi < 0) continue;
      if (action[3]) {
        current[qi].marks[oi] = "unmarked";
      } else if (action[2] === action[2].toUpperCase()) {
        current[qi].marks[oi] = "correct";
      } else {
        current[qi].marks[oi] = "incorrect";
      }
      history.push(cloneStates(current));
      continue;
    }
    // Header and unknown tokens (possible newer dialects): skipped.
  }

  if (historyIdx < 0 || historyIdx >= history.length) historyIdx = history.length - 1;
  return {
    questions: history[historyIdx],
    completed: false,
    stale: false,
    history,
    historyIdx,
    hints,
    fails,
  };
}

// ── v0 (frozen; decode only) ────────────────────────────────────────────────

/** Applies one v0 action; false when the token isn't one (caller skips it). */
function applyActionV0(action: string, qs: QuestionState[]): boolean {
  if (action === "cp") return true;
  const unmark = action.startsWith("-");
  const rest = unmark ? action.slice(1) : action;
  const letter = rest[rest.length - 1] ?? "";
  const qi = Number(rest.slice(0, -1)) - 1;
  const oi = LETTERS.indexOf(letter.toUpperCase());
  if (!Number.isInteger(qi) || qi < 0 || qi >= qs.length || oi < 0) return false;

  if (unmark) {
    qs[qi].marks[oi] = "unmarked";
  } else if (letter === letter.toUpperCase()) {
    qs[qi].marks[oi] = "correct";
  } else {
    qs[qi].marks[oi] = "incorrect";
  }
  return true;
}

function decodeV0(tokens: string[], n: number): SavedState {
  const stale = tokens[tokens.length - 1] === "!";
  const completed = stale ? tokens[tokens.length - 2] === "x" : tokens[tokens.length - 1] === "x";
  const actions = tokens.filter((t) => t !== "x" && t !== "!");

  const current = blankStates(n);
  const history = [cloneStates(current)];
  const hints = new Map<number, HintMarker>();

  let historyIdx = 0;

  for (const token of actions) {
    if (token === "" || token === "_") {
      historyIdx = history.length - 1;
      continue;
    }
    const hintMatch = /^h([1-4])$/.exec(token);
    if (hintMatch) {
      hints.set(history.length - 1, { level: Number(hintMatch[1]), qi: null });
      continue;
    }
    const isCurrent = token.startsWith("_");
    const action = isCurrent ? token.slice(1) : token;
    // Best-effort, like v1: an unrecognized token is skipped, not pushed as a
    // phantom step. No legitimate v0 string contains one.
    if (!applyActionV0(action, current)) continue;
    history.push(cloneStates(current));
    if (isCurrent) historyIdx = history.length - 1;
  }

  // Default to end if no _ marker found
  if (historyIdx === 0 && history.length > 1) historyIdx = history.length - 1;

  return {
    questions: history[historyIdx],
    completed,
    stale,
    history,
    historyIdx,
    hints,
    fails: new Map(),
  };
}

// ── Migration ───────────────────────────────────────────────────────────────

function migrateActionV0(token: string): string {
  return token.startsWith("-") ? `${token.slice(1)}-` : token;
}

/** v0 meta scan-parse (frozen), re-emitted as v1 live-family tokens. */
function migrateMetaV0(meta: string): string[] {
  const field = (letter: string) => {
    const m = new RegExp(`${letter}(\\d+)`).exec(meta);
    return m ? Number(m[1]) : 0;
  };
  const out: string[] = [];
  const sessions = field("s");
  const elapsed = field("e");
  if (sessions || elapsed) out.push(`se${sessions}`, `e${elapsed}`);
  const bursts = field("n");
  if (bursts) out.push(`n${bursts}`);
  const hintCount = field("h");
  if (hintCount) out.push(`h${hintCount}`);
  const checkpoints = field("c");
  if (checkpoints) out.push(`cp${checkpoints}`);
  if (meta.includes("f")) out.push("f");
  return out;
}

/**
 * Textual v0→v1 conversion of one stored value: a token-level rewrite that
 * needs no decode and no question count. v1 input passes through untouched.
 */
export function migrateValue(raw: string): string {
  if (isV1Value(raw)) return raw;
  const metaStr = ledgerPart(raw);
  let completed = false;
  let stale = false;
  const out = ["v1"];

  for (const token of historyPart(raw).split(".")) {
    if (token === "x") {
      completed = true;
    } else if (token === "!") {
      stale = true;
    } else if (token === "" || token === "_") {
      out.push("_");
    } else if (token.startsWith("_")) {
      out.push(migrateActionV0(token.slice(1)), "_");
    } else {
      out.push(migrateActionV0(token));
    }
  }

  const ledger = composeLedger({ solved: completed, stale }, migrateMetaV0(metaStr));
  return out.join(".") + (ledger ? META_SEP + ledger : "");
}

/**
 * One-shot boot sweep: convert every stored puzzle value to v1. Gated on the
 * stored format version; idempotent, so a re-run (or the heal-on-touch path
 * in `loadState`) is always safe.
 */
export function migrateLocalStorage(): void {
  try {
    if (getVersions().format >= FORMAT_VERSION) return;
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (!key?.startsWith(PREFIX)) continue;
      const raw = localStorage.getItem(key);
      if (raw && !isV1Value(raw)) localStorage.setItem(key, migrateValue(raw));
    }
    setVersion("format", FORMAT_VERSION);
  } catch {
    /* storage unavailable */
  }
}

// ── Meta: the local ledger ──────────────────────────────────────────────────
// The flags (s, st) lead and the counters follow. Counters accumulate while
// solving and stay put once solved, so the summary can read the solve back;
// `saveState` sets the flags when it writes a completed state.

export interface PuzzleMeta {
  /** Stretches of the puzzle being on screen; their durations sum to elapsedS. */
  sessions: number;
  elapsedS: number;
  /** Episodes of history navigation, not presses — see `trackHistoryBurst`. */
  historyBursts: number;
  hints: number;
  checkpoints: number;
  /** Refused checkpoint presses. Monotonic: rewinding can't launder it. */
  checkpointFails: number;
  fromShared?: boolean;
}

export function emptyMeta(): PuzzleMeta {
  return {
    sessions: 0,
    elapsedS: 0,
    historyBursts: 0,
    hints: 0,
    checkpoints: 0,
    checkpointFails: 0,
  };
}

/** The counter tokens a meta records, in the ledger's order. */
function metaTokens(meta: PuzzleMeta): string[] {
  const out = [`se${meta.sessions}`, `e${meta.elapsedS}`];
  if (meta.historyBursts) out.push(`n${meta.historyBursts}`);
  if (meta.hints) out.push(`h${meta.hints}`);
  if (meta.checkpoints) out.push(`cp${meta.checkpoints}`);
  if (meta.checkpointFails) out.push(`cpx${meta.checkpointFails}`);
  if (meta.fromShared) out.push("f");
  return out;
}

function parseMeta(ledger: string): PuzzleMeta {
  const meta = emptyMeta();
  for (const token of ledger.split(".")) {
    if (token === "f") {
      meta.fromShared = true;
      continue;
    }
    const m = /^([a-z]+?)(\d+)$/.exec(token);
    if (!m) continue;
    const value = Number(m[2]);
    if (m[1] === "se") meta.sessions = value;
    else if (m[1] === "e") meta.elapsedS = value;
    else if (m[1] === "n") meta.historyBursts = value;
    else if (m[1] === "h") meta.hints = value;
    else if (m[1] === "cp") meta.checkpoints = value;
    else if (m[1] === "cpx") meta.checkpointFails = value;
  }
  return meta;
}

/** The stored counters; all zero for a puzzle never played on this device. */
export function loadMeta(puzzleId: string): PuzzleMeta {
  try {
    const raw = localStorage.getItem(PREFIX + puzzleId);
    return raw ? parseMeta(ledgerPart(raw)) : emptyMeta();
  } catch {
    return emptyMeta();
  }
}

/** Replaces the counters; the flags stay as they are. */
export function saveMeta(puzzleId: string, meta: PuzzleMeta): void {
  try {
    const raw = localStorage.getItem(PREFIX + puzzleId);
    if (!raw) return;
    const ledger = composeLedger(ledgerFlags(ledgerPart(raw)), metaTokens(meta));
    localStorage.setItem(PREFIX + puzzleId, historyPart(raw) + META_SEP + ledger);
  } catch {}
}

// ── Stored state ────────────────────────────────────────────────────────────

/** What the ledger says about one puzzle, without decoding its history. */
export interface PuzzleProgress {
  started: boolean;
  completed: boolean;
  stale: boolean;
}

export function hasState(puzzleId: string): PuzzleProgress {
  try {
    const raw = localStorage.getItem(PREFIX + puzzleId);
    if (!raw) return { started: false, completed: false, stale: false };
    const flags = ledgerFlags(ledgerPart(raw));
    return { started: true, completed: flags.solved, stale: flags.stale };
  } catch {
    return { started: false, completed: false, stale: false };
  }
}

export function loadState(puzzleId: string, n: number): SavedState | null {
  try {
    let raw = localStorage.getItem(PREFIX + puzzleId);
    if (!raw) return null;
    // Heal-on-touch: a pre-deploy tab may write v0 after the sweep has run.
    if (!isV1Value(raw)) {
      raw = migrateValue(raw);
      localStorage.setItem(PREFIX + puzzleId, raw);
    }
    const state = decodeHistory(historyPart(raw), n);
    if (!state) return null;
    const flags = ledgerFlags(ledgerPart(raw));
    state.completed = flags.solved;
    state.stale = flags.stale;
    return state;
  } catch {
    return null;
  }
}

export function saveState(puzzleId: string, state: SavedState) {
  try {
    if (state.history.length <= 1 && !state.completed) {
      localStorage.removeItem(PREFIX + puzzleId);
      return;
    }
    const existing = localStorage.getItem(PREFIX + puzzleId);
    const existingLedger = existing ? ledgerPart(existing) : "";
    // Completion raises the solved flag, and the caller's `stale` settles `st`
    // — a completed save carries a fresh check_answer verdict. The counters
    // are preserved verbatim either way.
    const ledger = state.completed
      ? composeLedger({ solved: true, stale: state.stale }, counterTokens(existingLedger))
      : existingLedger;
    localStorage.setItem(
      PREFIX + puzzleId,
      encodeHistory(state) + (ledger ? META_SEP + ledger : ""),
    );
  } catch {
    // storage full or unavailable
  }
}

export function getCompletedPuzzleIds(): string[] {
  const ids: string[] = [];
  try {
    for (let i = 0; i < localStorage.length; i++) {
      const k = localStorage.key(i);
      if (!k?.startsWith(PREFIX)) continue;
      const raw = localStorage.getItem(k);
      if (raw && isSolvedValue(raw)) ids.push(k.slice(PREFIX.length));
    }
  } catch {}
  return ids;
}

/** Stale is only meaningful beside solved; the counters ride along. */
function setStale(puzzleId: string, stale: boolean): void {
  try {
    const raw = localStorage.getItem(PREFIX + puzzleId);
    if (!raw) return;
    const ledger = ledgerPart(raw);
    if (!ledgerFlags(ledger).solved) return;
    localStorage.setItem(
      PREFIX + puzzleId,
      historyPart(raw) + META_SEP + composeLedger({ solved: true, stale }, counterTokens(ledger)),
    );
  } catch {}
}

export function markStale(puzzleId: string): void {
  setStale(puzzleId, true);
}

export function unmarkStale(puzzleId: string): void {
  setStale(puzzleId, false);
}
