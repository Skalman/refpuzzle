import type { SavedState } from "./store.ts";
import { encodeHistory, decodeHistory } from "./store.ts";

export function getShareUrl(dateStr: string, level: number, state: SavedState): string {
  const encoded = encodeHistory(state);
  return `${window.location.origin}/day/${dateStr}?l=${level}#${encoded}`;
}

export function decodeShareHash(hash: string, n: number): SavedState | null {
  if (!hash) return null;
  return decodeHistory(hash, n);
}

export function getPuzzleUrl(dateStr: string, level: number): string {
  return `${window.location.origin}/day/${dateStr}?l=${level}`;
}
