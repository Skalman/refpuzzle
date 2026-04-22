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

export async function sharePuzzleLink(url: string, title: string): Promise<boolean> {
  if (typeof navigator !== "undefined" && navigator.share) {
    try {
      await navigator.share({ title, url });
      return true;
    } catch {
      // User cancelled or API failed
    }
  }

  if (typeof navigator !== "undefined" && navigator.clipboard) {
    try {
      await navigator.clipboard.writeText(url);
      return true;
    } catch {
      return false;
    }
  }

  return false;
}
