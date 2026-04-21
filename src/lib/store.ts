import type { Marks } from "../engine/types.ts";

export interface QuestionState {
  marks: Marks;
}

export interface SavedState {
  questions: QuestionState[];
  completed: boolean;
}

const PREFIX = "refpuzzle:puzzle:";

const VALID_MARKS: ReadonlySet<string> = new Set(["unmarked", "incorrect", "correct"]);

function isValidMarks(v: unknown): v is Marks {
  if (!Array.isArray(v) || v.length !== 5) return false;
  for (let i = 0; i < 5; i++) {
    if (typeof v[i] !== "string" || !VALID_MARKS.has(v[i])) return false;
  }
  return true;
}

function isValidState(v: unknown): v is SavedState {
  if (!v || typeof v !== "object" || !("questions" in v)) return false;
  const obj = v as Record<string, unknown>; // oxlint-disable-line typescript/no-unsafe-type-assertion
  if (!Array.isArray(obj.questions)) return false;
  for (const q of obj.questions) {
    if (!q || typeof q !== "object" || !("marks" in q)) return false;
    if (!isValidMarks((q as Record<string, unknown>).marks)) return false; // oxlint-disable-line typescript/no-unsafe-type-assertion
  }
  return true;
}

export function loadState(puzzleId: string): SavedState | null {
  try {
    const raw = localStorage.getItem(PREFIX + puzzleId);
    if (!raw) return null;
    const parsed: unknown = JSON.parse(raw);
    if (!isValidState(parsed)) return null;
    return parsed;
  } catch {
    return null;
  }
}

export function saveState(puzzleId: string, state: SavedState) {
  try {
    localStorage.setItem(PREFIX + puzzleId, JSON.stringify(state));
  } catch {
    // storage full or unavailable
  }
}
