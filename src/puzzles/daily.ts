import type { Puzzle } from "../engine/types.ts";

const START_DATE = "2026-04-19";
const CACHE = new Map<string, Record<string, Puzzle> | null>();

export function todayDateStr(): string {
  const now = new Date();
  const mm = String(now.getMonth() + 1).padStart(2, "0");
  const dd = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${mm}-${dd}`;
}

export function dateToPath(dateStr: string): string {
  const year = dateStr.slice(0, 4);
  const mmdd = dateStr.slice(5, 7) + dateStr.slice(8, 10);
  return `/puzzles/daily/${year}/${mmdd}.json`;
}

export function dayNumber(dateStr: string): number {
  const start = new Date(START_DATE + "T00:00:00");
  const date = new Date(dateStr + "T00:00:00");
  return Math.floor((date.getTime() - start.getTime()) / 86400000) + 1;
}

export function isValidDate(dateStr: string): boolean {
  return dayNumber(dateStr) >= 1 && dayNumber(dateStr) <= 1000;
}

export function dateStrFromOffset(daysAgo: number): string {
  const d = new Date();
  d.setDate(d.getDate() - daysAgo);
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  const dd = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${mm}-${dd}`;
}

export async function fetchDaily(dateStr: string): Promise<Record<string, Puzzle> | null> {
  if (CACHE.has(dateStr)) return CACHE.get(dateStr)!;
  try {
    const resp = await fetch(dateToPath(dateStr));
    if (!resp.ok) {
      CACHE.set(dateStr, null);
      return null;
    }
    const data: Record<string, Puzzle> = await resp.json(); // eslint-disable-line
    CACHE.set(dateStr, data);
    return data;
  } catch {
    CACHE.set(dateStr, null);
    return null;
  }
}

export function puzzleId(dateStr: string, level: number): string {
  return `daily-${dateStr}-L${level}`;
}
