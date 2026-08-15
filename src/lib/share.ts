import type { SavedState } from "./store.ts";
import { encodeHistory, decodeHistory } from "./store.ts";

export function getShareUrl(dateStr: string, level: number, state: SavedState): string {
  // The history segment carries no flags, so nothing device-local can leak.
  const encoded = encodeHistory(state);
  return `${window.location.origin}/${dateStr}/${level}#${encoded}`;
}

export function decodeShareHash(hash: string, n: number): SavedState | null {
  if (!hash) return null;
  return decodeHistory(hash, n);
}

export function getPuzzleUrl(dateStr: string, level: number): string {
  return `${window.location.origin}/${dateStr}/${level}`;
}
