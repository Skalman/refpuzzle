import type { Marks } from "../engine/types.ts";
import { LETTERS, L2I } from "../engine/types.ts";

export function encodeState(markSets: Marks[]): string {
  return markSets
    .map((marks) => {
      let seg = "";
      for (let i = 0; i < 5; i++) {
        if (marks[i] === "correct") seg += LETTERS[i];
        else if (marks[i] === "incorrect") seg += LETTERS[i].toLowerCase();
      }
      return seg || "_";
    })
    .join(".");
}

export function decodeState(encoded: string): Marks[] | null {
  if (!encoded) return null;
  const segments = encoded.split(".");
  if (segments.length === 0) return null;

  const result: Marks[] = [];
  for (const seg of segments) {
    const marks: Marks = ["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"];
    if (seg !== "_") {
      for (const ch of seg) {
        const idx = L2I[ch.toUpperCase()];
        if (idx == null) return null;
        marks[idx] = ch === ch.toUpperCase() ? "correct" : "incorrect";
      }
    }
    result.push(marks);
  }
  return result;
}

export function getShareUrl(puzzleId: string, markSets: Marks[]): string {
  const state = encodeState(markSets);
  return `${window.location.origin}/puzzle/${puzzleId}#${state}`;
}
