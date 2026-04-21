import type { Marks } from "../engine/types.ts";
import { LETTERS } from "../engine/types.ts";

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

export function getShareUrl(puzzleId: string, markSets: Marks[]): string {
  const state = encodeState(markSets);
  return `${window.location.origin}/puzzle/${puzzleId}#${state}`;
}
