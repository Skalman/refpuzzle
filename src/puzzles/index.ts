import type { Puzzle } from "../engine/types.ts";
import { level1 } from "./generated/level-1.ts";
import { level2 } from "./generated/level-2.ts";
import { level3 } from "./generated/level-3.ts";
import { level4 } from "./generated/level-4.ts";
import { level5 } from "./generated/level-5.ts";

export const puzzlesByLevel: Puzzle[][] = [level1, level2, level3, level4, level5];

export const allPuzzles: Puzzle[] = puzzlesByLevel.flat();
