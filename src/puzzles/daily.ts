import type { Puzzle, QuestionDef, QuestionType, Answer } from "../engine/types.ts";
import { LETTERS } from "../engine/types.ts";
import { wasmReady, generatePuzzle } from "../lib/wasm.ts";

const START_DATE = "2026-04-19";
const YEAR_RAW = new Map<string, Record<string, Record<string, CompactPuzzle>> | null>();
const DAY_CACHE = new Map<string, Record<string, Puzzle>>();

export interface CompactQuestionType {
  t: string;
  a?: number;
  q?: number;
}
export interface CompactPuzzle {
  q: CompactQuestionType[];
  o: (number | null)[][];
  t?: CompactQuestionType[];
}

export function parseCompactPuzzle(compact: CompactPuzzle): Puzzle {
  return buildPuzzle(compact, "playground");
}

export function parseCompactYear(
  data: Record<string, Record<string, CompactPuzzle>>,
): Record<string, Record<string, Puzzle>> {
  const result: Record<string, Record<string, Puzzle>> = {};
  for (const [mmdd, levels] of Object.entries(data)) {
    result[mmdd] = {};
    for (const [lvl, compact] of Object.entries(levels)) {
      result[mmdd][lvl] = buildPuzzle(compact, "", lvl);
    }
  }
  return result;
}

function buildPuzzle(compact: CompactPuzzle, id: string, difficulty: string = "1"): Puzzle {
  const optionCount = compact.o[0]?.length ?? 5;
  const questions = compact.q.map<QuestionDef>((cq, qi) => ({
    options: (compact.o[qi] ?? []).map((v) => ({ value: v })),
    questionType: expandQuestion(cq),
  }));
  const trueStmtQuestionTypes = compact.t?.map(expandQuestion);
  return { id, title: "", difficulty, questions, optionCount, trueStmtQuestionTypes };
}

function L(i: number | undefined): Answer {
  return LETTERS[i ?? 0];
}

export function expandQuestion(q: CompactQuestionType): QuestionType {
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  const type = q.t as QuestionType["type"];
  switch (type) {
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
      return { type };
    case "CountAnswer":
    case "FirstWith":
    case "LastWith":
    case "OnlyOdd":
    case "OnlyEven":
    case "EqualCount":
      return { type, answer: L(q.a) };
    case "CountAnswerAfter":
    case "ClosestAfter":
      return { type, answer: L(q.a), afterIndex: q.q! };
    case "CountAnswerBefore":
    case "ClosestBefore":
      return { type, answer: L(q.a), beforeIndex: q.q! };
    case "AnswerOf":
    case "LetterDist":
    case "SameAsWhich":
      return { type, questionIndex: q.q! };
    default: {
      (type) satisfies never;
      // oxlint-disable-next-line typescript/restrict-template-expressions
      throw new Error(`Unknown question type: ${type}`);
    }
  }
}

export function todayDateStr(): string {
  const now = new Date();
  const mm = String(now.getMonth() + 1).padStart(2, "0");
  const dd = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${mm}-${dd}`;
}

export function dayNumber(dateStr: string): number {
  const start = new Date(START_DATE + "T00:00:00");
  const date = new Date(dateStr + "T00:00:00");
  return Math.floor((date.getTime() - start.getTime()) / 86400000) + 1;
}

export function isValidDate(dateStr: string): boolean {
  if (dayNumber(dateStr) < 1) return false;
  if (
    typeof window !== "undefined" &&
    (new URLSearchParams(window.location.search).has("debug") ||
      sessionStorage.getItem("debug") === "1")
  ) {
    return true;
  }
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const date = new Date(dateStr + "T00:00:00");
  return date <= today;
}

export function dateStrFromOffset(daysAgo: number): string {
  const d = new Date();
  d.setDate(d.getDate() - daysAgo);
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  const dd = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${mm}-${dd}`;
}

async function fetchYearRaw(
  year: string,
): Promise<Record<string, Record<string, CompactPuzzle>> | null> {
  if (YEAR_RAW.has(year)) return YEAR_RAW.get(year)!;
  try {
    const resp = await fetch(`/puzzles/daily/${year}.json`);
    if (!resp.ok) {
      YEAR_RAW.set(year, null);
      return null;
    }
    const data = await resp.json();
    YEAR_RAW.set(year, data);
    return data;
  } catch {
    YEAR_RAW.set(year, null);
    return null;
  }
}

export async function fetchDaily(dateStr: string): Promise<Record<string, Puzzle> | null> {
  const key = dateStr;
  if (DAY_CACHE.has(key)) return DAY_CACHE.get(key)!;
  const year = dateStr.slice(0, 4);
  const mmdd = dateStr.slice(5, 7) + dateStr.slice(8, 10);
  const raw = await fetchYearRaw(year);
  if (raw?.[mmdd]) {
    const day = parseCompactYear({ [mmdd]: raw[mmdd] })[mmdd];
    DAY_CACHE.set(key, day);
    return day;
  }
  // Fallback: gen on the fly. Triggered when year.json is missing entirely or
  // the date isn't in it (e.g. browsing a future year we haven't pre-generated).
  const generated = await generateDay(dateStr);
  if (generated) DAY_CACHE.set(key, generated);
  return generated;
}

async function generateDay(dateStr: string): Promise<Record<string, Puzzle> | null> {
  await wasmReady();
  const y = Number(dateStr.slice(0, 4));
  const m = Number(dateStr.slice(5, 7));
  const d = Number(dateStr.slice(8, 10));
  const dateKey = y * 10000 + m * 100 + d;
  const day: Record<string, Puzzle> = {};
  for (let level = 1; level <= 6; level++) {
    const seed = (Math.imul(dateKey, 31) + level) >>> 0;
    const p = generatePuzzle(seed, level, puzzleId(dateStr, level));
    if (!p) return null;
    day[String(level)] = p;
  }
  return day;
}

export function puzzleId(dateStr: string, level: number): string {
  return `/${dateStr}/${level}`;
}
