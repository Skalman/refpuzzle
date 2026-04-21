// Generator-specific types (separate from engine types to keep build clean)

export interface DifficultyProfile {
	level: 1 | 2 | 3 | 4 | 5;
	name: string;
	questionCount: number;
	allowedTypes: string[];
	maxBranchPoints: number;
}
