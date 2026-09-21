import type { SavedState } from "./store.ts";
import { encodeHistory, decodeHistory } from "./store.ts";

export function getShareUrl(dateStr: string, level: number, state: SavedState): string {
  // The history segment carries the track and its markers — which questions
  // took a hint or a refused checkpoint — but no ledger flags or timings.
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

/** A link as it reads on screen: no scheme, no trailing slash. */
export function prettyUrl(url: string): string {
  return url.replace(/^https?:\/\//, "").replace(/\/$/, "");
}

/** Just the host a link points at, for signing a shared card. */
export function hostOf(url: string): string {
  return url.replace(/^https?:\/\//, "").replace(/\/.*$/, "");
}
