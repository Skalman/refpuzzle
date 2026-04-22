import type { Marks } from "../engine/types.ts";

export interface QuestionState {
  marks: Marks;
}

export interface SavedState {
  questions: QuestionState[];
  completed: boolean;
  history: QuestionState[][];
  historyIdx: number;
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

  if (state.completed) actions.push("x");

  for (let i = 1; i < state.history.length; i++) {
    let action = diffAction(state.history[i - 1], state.history[i]);
    if (i === state.historyIdx) action = `_${action}`;
    actions.push(action);
  }

  // If current is at position 0 (no actions taken yet) or at the end
  if (state.historyIdx === 0 && state.history.length > 1) {
    actions.splice(state.completed ? 1 : 0, 0, "_");
  }
  if (state.historyIdx === state.history.length - 1 && state.history.length > 1) {
    const last = actions.length - 1;
    if (!actions[last].startsWith("_")) actions[last] = `_${actions[last]}`;
  }

  return actions.join(".");
}

export function decodeHistory(encoded: string, n: number): SavedState | null {
  const tokens = encoded.split(".");
  const completed = tokens[0] === "x";
  const actions = completed ? tokens.slice(1) : tokens;

  const history: QuestionState[][] = [];
  const current = Array.from({ length: n }, () => ({ marks: [...FRESH_MARKS] as Marks }));
  history.push(cloneStates(current));

  let historyIdx = 0;

  for (const token of actions) {
    if (token === "" || token === "_") {
      historyIdx = history.length - 1;
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
  };
}

export function loadState(puzzleId: string): SavedState | null {
  try {
    const raw = localStorage.getItem(PREFIX + puzzleId);
    if (!raw) return null;
    let maxQ = 0;
    for (const token of raw.split(".")) {
      const clean = token.replace(/^[_-]/, "");
      if (clean === "x" || clean === "cp" || clean === "") continue;
      const q = Number(clean.replace(/[a-eA-E]$/, ""));
      if (q > maxQ) maxQ = q;
    }
    return decodeHistory(raw, Math.max(maxQ, 4));
  } catch {
    return null;
  }
}

export function saveState(puzzleId: string, state: SavedState) {
  try {
    localStorage.setItem(PREFIX + puzzleId, encodeHistory(state));
  } catch {
    // storage full or unavailable
  }
}
