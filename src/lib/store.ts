import type { Marks } from "../engine/types.ts";

export interface QuestionState {
  marks: Marks;
}

export interface SavedState {
  questions: QuestionState[];
  completed: boolean;
  history: QuestionState[][];
  historyIdx: number;
  hints: Map<number, number>;
}

const PREFIX = "refpuzzle:puzzle:";
const FRESH_MARKS: Marks = ["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"];
const LETTERS = ["A", "B", "C", "D", "E"];

function diffAction(prev: QuestionState[], next: QuestionState[]): string {
  for (let qi = 0; qi < prev.length; qi++) {
    for (let oi = 0; oi < 5; oi++) {
      if (prev[qi].marks[oi] !== next[qi].marks[oi]) {
        const q = qi + 1;
        const letter = LETTERS[oi];
        const to = next[qi].marks[oi];
        if (to === "incorrect") return `${q}${letter.toLowerCase()}`;
        if (to === "correct") return `${q}${letter}`;
        return `-${q}${letter.toLowerCase()}`;
      }
    }
  }
  return "cp";
}

function applyAction(action: string, qs: QuestionState[]) {
  if (action === "cp") return;
  const unmark = action.startsWith("-");
  const rest = unmark ? action.slice(1) : action;
  const letter = rest[rest.length - 1];
  const qi = Number(rest.slice(0, -1)) - 1;
  const oi = LETTERS.indexOf(letter.toUpperCase());
  if (qi < 0 || qi >= qs.length || oi < 0) return;

  if (unmark) {
    qs[qi].marks[oi] = "unmarked";
  } else if (letter === letter.toUpperCase()) {
    qs[qi].marks[oi] = "correct";
  } else {
    qs[qi].marks[oi] = "incorrect";
  }
}

function cloneStates(qs: QuestionState[]): QuestionState[] {
  return qs.map((q) => ({ marks: [...q.marks] as Marks }));
}

export function encodeHistory(state: SavedState): string {
  const actions: string[] = [];
  const atEnd = state.historyIdx === state.history.length - 1;

  for (let i = 1; i < state.history.length; i++) {
    let action = diffAction(state.history[i - 1], state.history[i]);
    if (i === state.historyIdx && !atEnd) action = `_${action}`;
    actions.push(action);
    const hintLevel = state.hints.get(i);
    if (hintLevel != null) actions.push(`h${hintLevel}`);
  }

  if (state.historyIdx === 0 && state.history.length > 1) {
    actions.splice(0, 0, "_");
  }

  if (state.completed) actions.push("x");

  return actions.join(".");
}

export function decodeHistory(encoded: string, n: number): SavedState | null {
  const tokens = encoded.split(".");
  const completed = tokens[tokens.length - 1] === "x";
  const actions = tokens.filter((t) => t !== "x");

  const history: QuestionState[][] = [];
  const current = Array.from({ length: n }, () => ({ marks: [...FRESH_MARKS] as Marks }));
  history.push(cloneStates(current));
  const hints = new Map<number, number>();

  let historyIdx = 0;

  for (const token of actions) {
    if (token === "" || token === "_") {
      historyIdx = history.length - 1;
      continue;
    }
    // Hint marker: h1, h2, h3, h4
    const hintMatch = /^h([1-4])$/.exec(token);
    if (hintMatch) {
      hints.set(history.length - 1, Number(hintMatch[1]));
      continue;
    }
    const isCurrent = token.startsWith("_");
    const action = isCurrent ? token.slice(1) : token;
    applyAction(action, current);
    history.push(cloneStates(current));
    if (isCurrent) historyIdx = history.length - 1;
  }

  // Default to end if no _ marker found
  if (historyIdx === 0 && history.length > 1) historyIdx = history.length - 1;

  return {
    questions: history[historyIdx],
    completed,
    history,
    historyIdx,
    hints,
  };
}

export interface PuzzleMeta {
  sessions: number;
  elapsedS: number;
  fromShared?: boolean;
}

const META_SEP = "|";

function stripMeta(raw: string): string {
  const i = raw.indexOf(META_SEP);
  return i >= 0 ? raw.slice(0, i) : raw;
}

function encodeMeta(meta: PuzzleMeta): string {
  return `s${meta.sessions}e${meta.elapsedS}${meta.fromShared ? "f" : ""}`;
}

function parseMeta(s: string): PuzzleMeta {
  const sm = /s(\d+)/.exec(s);
  const em = /e(\d+)/.exec(s);
  return {
    sessions: sm ? Number(sm[1]) : 0,
    elapsedS: em ? Number(em[1]) : 0,
    fromShared: s.includes("f"),
  };
}

export function hasState(puzzleId: string): { started: boolean; completed: boolean } {
  try {
    const raw = localStorage.getItem(PREFIX + puzzleId);
    if (!raw) return { started: false, completed: false };
    const history = stripMeta(raw);
    return { started: true, completed: history.endsWith(".x") || history === "x" };
  } catch {
    return { started: false, completed: false };
  }
}

export function loadState(puzzleId: string, n: number): SavedState | null {
  try {
    const raw = localStorage.getItem(PREFIX + puzzleId);
    if (!raw) return null;
    return decodeHistory(stripMeta(raw), n);
  } catch {
    return null;
  }
}

export function saveState(puzzleId: string, state: SavedState) {
  try {
    if (state.history.length <= 1 && !state.completed) {
      localStorage.removeItem(PREFIX + puzzleId);
    } else {
      const existing = localStorage.getItem(PREFIX + puzzleId);
      const i = existing?.indexOf(META_SEP) ?? -1;
      const metaSuffix = i >= 0 ? existing!.slice(i) : "";
      localStorage.setItem(PREFIX + puzzleId, encodeHistory(state) + metaSuffix);
    }
  } catch {
    // storage full or unavailable
  }
}

export function loadMeta(puzzleId: string): PuzzleMeta {
  try {
    const raw = localStorage.getItem(PREFIX + puzzleId);
    if (!raw) return { sessions: 0, elapsedS: 0 };
    const i = raw.indexOf(META_SEP);
    return i >= 0 ? parseMeta(raw.slice(i + 1)) : { sessions: 0, elapsedS: 0 };
  } catch {
    return { sessions: 0, elapsedS: 0 };
  }
}

export function saveMeta(puzzleId: string, meta: PuzzleMeta): void {
  try {
    const raw = localStorage.getItem(PREFIX + puzzleId);
    if (!raw) return;
    localStorage.setItem(PREFIX + puzzleId, stripMeta(raw) + META_SEP + encodeMeta(meta));
  } catch {}
}

export function clearMeta(puzzleId: string): void {
  try {
    const raw = localStorage.getItem(PREFIX + puzzleId);
    if (raw?.includes(META_SEP)) {
      localStorage.setItem(PREFIX + puzzleId, stripMeta(raw));
    }
  } catch {}
}
