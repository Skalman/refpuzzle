import type {
  Puzzle,
  QuestionDef,
  QuestionTypeDef,
  OptionDef,
  Claim,
  AnswerLetter,
} from "../engine/types.ts";
import { LETTERS } from "../engine/types.ts";

const START_DATE = "2026-04-19";
const YEAR_RAW = new Map<string, Record<string, Record<string, CompactPuzzle>> | null>();
const DAY_CACHE = new Map<string, Record<string, Puzzle>>();

interface CompactQuestionTypeDef {
  t: string;
  a?: number;
  q?: number;
}
interface CompactClaim {
  t: string;
  a?: number;
  q?: number;
  v: number;
}
interface CompactQuestion {
  o?: (number | null)[];
  t: CompactQuestionTypeDef;
  c?: (CompactClaim | null)[];
}
interface CompactPuzzle {
  q: CompactQuestion[];
}

export function parseCompactYear(
  data: Record<string, Record<string, CompactPuzzle>>,
): Record<string, Record<string, Puzzle>> {
  const result: Record<string, Record<string, Puzzle>> = {};
  for (const [mmdd, levels] of Object.entries(data)) {
    result[mmdd] = {};
    for (const [lvl, compact] of Object.entries(levels)) {
      const questions = compact.q.map<QuestionDef>((cq) => {
        const questionType = expandQuestion(cq.t);
        const options: OptionDef[] = cq.c
          ? cq.c.map((cc) => ({ value: null, claim: expandClaim(cc!) }))
          : (cq.o ?? [null, null, null, null, null]).map((v) => ({ value: v }));
        return { options, questionType };
      });
      result[mmdd][lvl] = {
        id: "",
        title: "",
        difficulty: lvl,
        questions,
      };
    }
  }
  return result;
}

function L(i: number | undefined): AnswerLetter {
  return LETTERS[i ?? 0];
}

function expandQuestion(q: CompactQuestionTypeDef): QuestionTypeDef {
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  const type = q.t as QuestionTypeDef["type"];
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
    case "Unique":
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
      return { type, questionIndex: q.q! };
    default: {
      (type) satisfies never;
      // oxlint-disable-next-line typescript/restrict-template-expressions
      throw new Error(`Unknown question type: ${type}`);
    }
  }
}

function expandClaim(c: CompactClaim): Claim {
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  const rule = expandQuestion(c) as QuestionTypeDef & { type: Claim["type"] };
  return { ...rule, value: c.v };
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
  if (typeof window !== "undefined" && new URLSearchParams(window.location.search).has("debug")) {
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
  if (!raw?.[mmdd]) return null;
  const day = parseCompactYear({ [mmdd]: raw[mmdd] })[mmdd];
  DAY_CACHE.set(key, day);
  return day;
}

export function puzzleId(dateStr: string, level: number): string {
  return `daily-${dateStr}-L${level}`;
}
