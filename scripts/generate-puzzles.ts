import { writeFileSync, mkdirSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { generateConstructive as generate } from "../src/generator/construct.ts";
import { profiles } from "../src/generator/difficulty.ts";
import { RNG } from "../src/generator/rng.ts";
import type { AnswerLetter, Puzzle } from "../src/engine/types.ts";

const __dirname = dirname(fileURLToPath(import.meta.url));
const outputDir = resolve(__dirname, "../src/puzzles/generated");

const args = process.argv.slice(2);
let targetLevel: number | null = null;
let baseSeed = 42;
let count = 1;

for (let i = 0; i < args.length; i++) {
	if (args[i] === "--level" && args[i + 1]) {
		targetLevel = Number(args[++i]);
	} else if (args[i] === "--seed" && args[i + 1]) {
		baseSeed = Number(args[++i]);
	} else if (args[i] === "--count" && args[i + 1]) {
		count = Number(args[++i]);
	}
}

const levels = targetLevel
	? profiles.filter((p) => p.level === targetLevel)
	: profiles;

if (levels.length === 0) {
	console.error(`Unknown level: ${targetLevel}`);
	process.exit(1);
}

mkdirSync(outputDir, { recursive: true });

let allOk = true;

for (const profile of levels) {
	const t0 = performance.now();
	console.log(
		`Level ${profile.level} (${profile.name}, ${profile.questionCount}Q, ${count} puzzles) ...`,
	);

	const puzzles: { puzzle: Puzzle; solution: AnswerLetter[] }[] = [];
	let seedOffset = 0;
	const maxOuter = profile.level <= 3 ? 20 : 100;

	for (let pi = 0; pi < count; pi++) {
		let result = null;
		let attempts = 0;

		while (!result && attempts < maxOuter) {
			const seed =
				baseSeed + profile.level * 10000 + seedOffset * 7919;
			seedOffset++;
			const rng = new RNG(seed);
			result = generate(profile, rng);
			attempts++;
		}

		if (!result) {
			console.error(
				`  FAILED puzzle ${pi + 1}/${count} after ${attempts} seeds`,
			);
			allOk = false;
			break;
		}

		result.puzzle.id = `level-${profile.level}-${pi + 1}`;
		result.puzzle.title = `${profile.name} #${pi + 1}`;
		puzzles.push(result);
		process.stdout.write(`  ${pi + 1}/${count}`);
	}

	const elapsed = ((performance.now() - t0) / 1000).toFixed(2);

	if (puzzles.length < count) {
		console.log(`  INCOMPLETE (${elapsed}s)`);
		continue;
	}

	const filename = `level-${profile.level}.ts`;
	writeFileSync(resolve(outputDir, filename), formatFile(puzzles, profile.level));

	console.log(
		`  OK  ${elapsed}s  → ${filename}  (${puzzles.length} puzzles)`,
	);
}

if (!allOk) process.exit(1);

function formatPuzzle(puzzle: Puzzle): string {
	const qs = puzzle.questions
		.map((q) => {
			const opts = q.options
				.map((o) => {
					if ("claim" in o) {
						return `\t\t\t{\n\t\t\t\tlabel: ${JSON.stringify(o.label)},\n\t\t\t\tclaim: ${JSON.stringify(o.claim)},\n\t\t\t}`;
					}
					return `\t\t\t{ label: ${JSON.stringify(o.label)} }`;
				})
				.join(",\n");
			return `\t\t{\n\t\t\ttext: ${JSON.stringify(q.text)},\n\t\t\toptions: [\n${opts},\n\t\t\t],\n\t\t\trule: ${JSON.stringify(q.rule)},\n\t\t}`;
		})
		.join(",\n");

	return `{
\tid: ${JSON.stringify(puzzle.id)},
\ttitle: ${JSON.stringify(puzzle.title)},
\tdifficulty: ${puzzle.difficulty},
\tquestions: [
${qs},
\t],
}`;
}

function formatFile(
	results: { puzzle: Puzzle; solution: AnswerLetter[] }[],
	level: number,
): string {
	const puzzleStrs = results.map((r) => formatPuzzle(r.puzzle)).join(",\n");
	const solutionStrs = results
		.map((r) => JSON.stringify(r.solution))
		.join(",\n\t");

	return `import type { Puzzle } from "../../engine/types.ts";

export const level${level}: Puzzle[] = [
${puzzleStrs},
];

export const level${level}Solutions = [
\t${solutionStrs},
] as const;
`;
}
