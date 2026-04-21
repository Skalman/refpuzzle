import { LETTERS, L2I } from "../engine/types.ts";

type OptionMark = "unmarked" | "incorrect" | "correct";
type Marks = [OptionMark, OptionMark, OptionMark, OptionMark, OptionMark];

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

export function decodeState(s: string): Marks[] {
	return s.split(".").map((raw): Marks => {
		const seg = raw === "_" ? "" : raw;
		const marks: Marks = ["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"];
		for (const ch of seg) {
			const upper = ch.toUpperCase();
			const idx = L2I[upper] ?? -1;
			if (idx < 0) continue;
			marks[idx] = ch === upper ? "correct" : "incorrect";
		}
		return marks;
	});
}

export function getShareUrl(puzzleId: string, markSets: Marks[]): string {
	const state = encodeState(markSets);
	return `${window.location.origin}/puzzle/${puzzleId}#${state}`;
}
