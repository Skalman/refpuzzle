import type { Puzzle } from "../engine/types.ts";

const START_DATE = "2026-04-19";
const YEAR_CACHE = new Map<string, Record<string, Record<string, Puzzle>> | null>();

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
  const d = dayNumber(dateStr);
  return d >= 1 && d <= 1461;
}

export function dateStrFromOffset(daysAgo: number): string {
  const d = new Date();
  d.setDate(d.getDate() - daysAgo);
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  const dd = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${mm}-${dd}`;
}

async function fetchYear(year: string): Promise<Record<string, Record<string, Puzzle>> | null> {
  if (YEAR_CACHE.has(year)) return YEAR_CACHE.get(year)!;
  try {
    const resp = await fetch(`/puzzles/daily/${year}.json`);
    if (!resp.ok) {
      YEAR_CACHE.set(year, null);
      return null;
    }
    const data: Record<string, Record<string, Puzzle>> = await resp.json(); // eslint-disable-line
    YEAR_CACHE.set(year, data);
    return data;
  } catch {
    YEAR_CACHE.set(year, null);
    return null;
  }
}

export async function fetchDaily(dateStr: string): Promise<Record<string, Puzzle> | null> {
  const year = dateStr.slice(0, 4);
  const mmdd = dateStr.slice(5, 7) + dateStr.slice(8, 10);
  const yearData = await fetchYear(year);
  if (!yearData) return null;
  return yearData[mmdd] ?? null;
}

export function puzzleId(dateStr: string, level: number): string {
  return `daily-${dateStr}-L${level}`;
}
