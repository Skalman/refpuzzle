type OptionMark = "unmarked" | "incorrect" | "correct";

interface QuestionState {
	marks: [OptionMark, OptionMark, OptionMark, OptionMark, OptionMark];
}

interface SavedState {
	questions: QuestionState[];
	completed: boolean;
}

const PREFIX = "selfrefquiz:puzzle:";

export function loadState(puzzleId: string): SavedState | null {
	try {
		const raw = localStorage.getItem(PREFIX + puzzleId);
		if (!raw) return null;
		const parsed: unknown = JSON.parse(raw);
		if (parsed && typeof parsed === "object" && "questions" in parsed) {
			return parsed as SavedState; // oxlint-disable-line typescript/no-unsafe-type-assertion
		}
		return null;
	} catch {
		return null;
	}
}

export function saveState(puzzleId: string, state: SavedState) {
	try {
		localStorage.setItem(PREFIX + puzzleId, JSON.stringify(state));
	} catch {
		// storage full or unavailable
	}
}

export function clearState(puzzleId: string) {
	try {
		localStorage.removeItem(PREFIX + puzzleId);
	} catch {
		// ignore
	}
}
