import type { AnswerLetter, Puzzle, FlatPuzzle } from "../src/engine/types.ts";
import { LETTERS, flattenPuzzle } from "../src/engine/types.ts";
import { evaluate, evaluateClaim } from "../src/engine/evaluators.ts";
import { solve } from "../src/generator/solver.ts";
import { allPuzzles } from "../src/puzzles/index.ts";

let passed = 0;
let failed = 0;

function assert(condition: boolean, msg: string) {
	if (condition) {
		passed++;
	} else {
		failed++;
		console.error(`  FAIL: ${msg}`);
	}
}

function assertEq<T>(actual: T, expected: T, msg: string) {
	assert(
		JSON.stringify(actual) === JSON.stringify(expected),
		`${msg}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`,
	);
}

// ════════════════════════════════════════════════
// Evaluator tests
// ════════════════════════════════════════════════

function testEvaluators() {
	console.log("Evaluator tests...");

	const puzzle: Puzzle = {
		id: "test",
		title: "Test",
		difficulty: 1,
		questions: [
			{
				text: "Q1",
				options: [
					{ label: "0" },
					{ label: "1" },
					{ label: "2" },
					{ label: "3" },
					{ label: "4" },
				],
				rule: { type: "count_answer", answer: "B" },
			},
			{
				text: "Q2",
				options: [
					{ label: "A" },
					{ label: "B" },
					{ label: "C" },
					{ label: "D" },
					{ label: "E" },
				],
				rule: { type: "answer_of_question", questionIndex: 0 },
			},
			{
				text: "Q3",
				options: [
					{ label: "1" },
					{ label: "2" },
					{ label: "3" },
					{ label: "4" },
					{ label: "None" },
				],
				rule: { type: "closest_after", afterIndex: 0, answer: "C" },
			},
			{
				text: "Q4",
				options: [
					{ label: "0" },
					{ label: "1" },
					{ label: "2" },
					{ label: "3" },
					{ label: "4" },
				],
				rule: { type: "letter_distance", otherQuestionIndex: 0 },
			},
		],
	};
	const fp = flattenPuzzle(puzzle);

	// count_answer: [C, B, C, A] → count(B) = 1, option B = "1" ✓
	const answers: AnswerLetter[] = ["C", "B", "C", "A"];
	assert(
		evaluate(fp.rules[0], 0, "C", answers, fp) === false,
		"count_answer C: count(B)=1, optC='2', should be false",
	);
	assert(
		evaluate(fp.rules[0], 0, "B", answers, fp) === true,
		"count_answer B: count(B)=1, optB='1', should be true",
	);

	// answer_of_question: Q2 should match Q1's answer
	assert(
		evaluate(fp.rules[1], 1, "C", answers, fp) === true,
		"answer_of_question: Q1=C, selecting C → optC='C' matches",
	);
	assert(
		evaluate(fp.rules[1], 1, "A", answers, fp) === false,
		"answer_of_question: Q1=C, selecting A → optA='A' ≠ C",
	);

	// closest_after: closest C after Q1 (index 0) → Q3 (index 2, display 3)
	assert(
		evaluate(fp.rules[2], 2, "C", answers, fp) === true,
		"closest_after: closest C after #1 is Q3, optC='3' ✓",
	);
	assert(
		evaluate(fp.rules[2], 2, "A", answers, fp) === false,
		"closest_after: optA='1' but Q1 isn't C",
	);

	// letter_distance: Q4's selected answer vs Q1's answer (C)
	// If Q4=A: |A-C| = |0-2| = 2, optA='0' → 2≠0 ✗
	// If Q4=C: |C-C| = 0, optC='2' → 0≠2 ✗
	// If Q4=E: |E-C| = |4-2| = 2, optE='4' → 2≠4 ✗
	// If Q4=D: |D-C| = |3-2| = 1, optD='3' → 1≠3 ✗
	// If Q4=B: |B-C| = |1-2| = 1, optB='1' → 1=1 ✓
	assert(
		evaluate(fp.rules[3], 3, "B", answers, fp) === true,
		"letter_distance: |B-C| = 1, optB='1' ✓",
	);
	assert(
		evaluate(fp.rules[3], 3, "A", answers, fp) === false,
		"letter_distance: |A-C| = 2, optA='0' ✗",
	);

	// Test with partial answers (nulls)
	const partial: (AnswerLetter | null)[] = ["C", null, "C", null];
	assert(
		evaluate(fp.rules[0], 0, "A", partial, fp) === true,
		"count_answer partial: count(B)=0, optA='0' ✓",
	);

	// Test least_common_answer
	const lcPuzzle: Puzzle = {
		id: "lc",
		title: "LC",
		difficulty: 1,
		questions: [
			{
				text: "Q1",
				options: [
					{ label: "A" },
					{ label: "B" },
					{ label: "C" },
					{ label: "D" },
					{ label: "E" },
				],
				rule: { type: "least_common_answer" },
			},
			{
				text: "Q2",
				options: [{ label: "A" }, { label: "B" }, { label: "C" }, { label: "D" }, { label: "E" }],
				rule: { type: "answer_is_self" },
			},
			{
				text: "Q3",
				options: [{ label: "A" }, { label: "B" }, { label: "C" }, { label: "D" }, { label: "E" }],
				rule: { type: "answer_is_self" },
			},
		],
	};
	const lcFp = flattenPuzzle(lcPuzzle);
	const lcAnswers: AnswerLetter[] = ["A", "B", "B"];
	// A=1, B=2, C=D=E=0. Least common = C,D,E (count 0)
	assert(
		evaluate(lcFp.rules[0], 0, "C", lcAnswers, lcFp) === true,
		"least_common: C(0) is minimum, selecting C ✓",
	);
	assert(
		evaluate(lcFp.rules[0], 0, "A", lcAnswers, lcFp) === false,
		"least_common: A(1) > min(0), selecting A ✗",
	);
	assert(
		evaluate(lcFp.rules[0], 0, "B", lcAnswers, lcFp) === false,
		"least_common: B(2) > min(0), selecting B ✗",
	);

	// answer_is_self: always true
	assert(
		evaluate(lcFp.rules[1], 1, "A", lcAnswers, lcFp) === true,
		"answer_is_self: always true for A",
	);
	assert(
		evaluate(lcFp.rules[1], 1, "E", lcAnswers, lcFp) === true,
		"answer_is_self: always true for E",
	);

	// Test evaluateClaim
	const claimAnswers: AnswerLetter[] = ["A", "B", "C", "B", "A"];
	assert(
		evaluateClaim(
			{ type: "count_answer_equals", answer: "B", value: 2 },
			claimAnswers,
		) === true,
		"claim count_answer_equals B=2 ✓",
	);
	assert(
		evaluateClaim(
			{ type: "count_answer_equals", answer: "B", value: 3 },
			claimAnswers,
		) === false,
		"claim count_answer_equals B=3 ✗",
	);
	assert(
		evaluateClaim(
			{ type: "count_vowel_answers_equals", value: 2 },
			claimAnswers,
		) === true,
		"claim vowels=2 (A,A) ✓",
	);
	assert(
		evaluateClaim(
			{ type: "count_consonant_answers_equals", value: 3 },
			claimAnswers,
		) === true,
		"claim consonants=3 (B,C,B) ✓",
	);
}

// ════════════════════════════════════════════════
// Solver tests
// ════════════════════════════════════════════════

function testSolver() {
	console.log("Solver tests...");

	// Simple puzzle with known unique solution
	const simple: Puzzle = {
		id: "s",
		title: "S",
		difficulty: 1,
		questions: [
			{
				text: "Q1",
				options: [
					{ label: "A" },
					{ label: "B" },
					{ label: "C" },
					{ label: "D" },
					{ label: "E" },
				],
				rule: { type: "answer_of_question", questionIndex: 1 },
			},
			{
				text: "Q2",
				options: [
					{ label: "A" },
					{ label: "B" },
					{ label: "C" },
					{ label: "D" },
					{ label: "E" },
				],
				rule: { type: "answer_of_question", questionIndex: 0 },
			},
			{
				text: "Q3",
				options: [
					{ label: "3" },
					{ label: "0" },
					{ label: "1" },
					{ label: "2" },
					{ label: "4" },
				],
				rule: { type: "count_answer", answer: "A" },
			},
		],
	};
	// Q1=Q2 (mirror). Q3 counts A's.
	// If Q1=Q2=A: count(A)=2, opt A='3' → 2≠3 ✗
	// If Q1=Q2=C: count(A)=0, opt C='1' → 0≠1 ✗
	// If Q1=Q2=D: count(A)=0, opt D='2' → 0≠2 ✗
	// Need to check all combos...
	const solutions = solve(simple, undefined, 10);
	assert(solutions.length > 0, "simple puzzle has at least 1 solution");

	// Verify each solution is actually valid
	const fp = flattenPuzzle(simple);
	for (const sol of solutions) {
		const allValid = fp.rules.every((r, i) =>
			evaluate(r, i, sol[i], sol, fp),
		);
		assert(allValid, `solver solution [${sol.join(",")}] validates correctly`);
	}

	// Test with fixed answers
	if (solutions.length > 0) {
		const sol = solutions[0];
		const fixed: (AnswerLetter | null)[] = [sol[0], null, null];
		const constrained = solve(simple, fixed, 10);
		assert(
			constrained.length >= 1 && constrained.some((s) => s[0] === sol[0]),
			"solver with fixed answer includes the expected solution",
		);
	}
}

// ════════════════════════════════════════════════
// Naive brute-force solver (no pruning, for cross-validation)
// ════════════════════════════════════════════════

function bruteForce(puzzle: Puzzle, maxN = 8): AnswerLetter[][] {
	const n = puzzle.questions.length;
	if (n > maxN) return []; // too large for brute force
	const fp = flattenPuzzle(puzzle);
	const solutions: AnswerLetter[][] = [];
	const current: AnswerLetter[] = new Array(n).fill("A");

	function recurse(depth: number) {
		if (depth === n) {
			const valid = fp.rules.every((r, i) =>
				evaluate(r, i, current[i], current, fp),
			);
			if (valid) solutions.push([...current]);
			return;
		}
		for (const letter of LETTERS) {
			current[depth] = letter;
			recurse(depth + 1);
		}
	}

	recurse(0);
	return solutions;
}

// ════════════════════════════════════════════════
// Generated puzzle cross-validation
// ════════════════════════════════════════════════

function testGeneratedPuzzles() {
	console.log("Generated puzzle tests...");

	const puzzles = allPuzzles.map((p) => ({
		name: p.id,
		puzzle: p,
	}));

	for (const { name, puzzle } of puzzles) {
		// Solver finds exactly 1 solution
		const solutions = solve(puzzle, undefined, 2);
		assert(
			solutions.length === 1,
			`${name}: solver finds exactly 1 solution (found ${solutions.length})`,
		);

		if (solutions.length !== 1) continue;
		const sol = solutions[0];

		// Solution validates correctly
		const fp = flattenPuzzle(puzzle);
		const allValid = fp.rules.every((r, i) =>
			evaluate(r, i, sol[i], sol, fp),
		);
		assert(allValid, `${name}: solution [${sol.join(",")}] validates`);

		// Cross-validate with brute force for small puzzles
		if (puzzle.questions.length <= 8) {
			const t0 = performance.now();
			const bruteSolutions = bruteForce(puzzle);
			const elapsed = (performance.now() - t0).toFixed(0);
			assertEq(
				bruteSolutions.length,
				1,
				`${name}: brute force finds exactly 1 solution (${elapsed}ms)`,
			);
			if (bruteSolutions.length === 1) {
				assertEq(
					bruteSolutions[0],
					sol,
					`${name}: brute force solution matches solver`,
				);
			}
		}

		// Every question has unique text
		const texts = new Set(puzzle.questions.map((q) => q.text));
		assert(
			texts.size === puzzle.questions.length,
			`${name}: all question texts are unique`,
		);

		// Every question has exactly 5 options
		for (let i = 0; i < puzzle.questions.length; i++) {
			assert(
				puzzle.questions[i].options.length === 5,
				`${name} Q${i + 1}: has 5 options`,
			);
		}

		// Options within each question are distinct
		for (let i = 0; i < puzzle.questions.length; i++) {
			const labels = puzzle.questions[i].options.map((o) => o.label);
			const unique = new Set(labels);
			assert(
				unique.size === 5,
				`${name} Q${i + 1}: all option labels are distinct (${labels.join(", ")})`,
			);
		}
	}
}

// ════════════════════════════════════════════════
// Solver edge cases
// ════════════════════════════════════════════════

function testSolverEdgeCases() {
	console.log("Solver edge cases...");

	// Puzzle with no solution
	const impossible: Puzzle = {
		id: "imp",
		title: "Imp",
		difficulty: 1,
		questions: [
			{
				text: "Q1",
				options: [
					{ label: "A" },
					{ label: "B" },
					{ label: "C" },
					{ label: "D" },
					{ label: "E" },
				],
				rule: { type: "answer_of_question", questionIndex: 1 },
			},
			{
				text: "Q2",
				options: [
					{ label: "B" },
					{ label: "A" },
					{ label: "D" },
					{ label: "E" },
					{ label: "C" },
				],
				// Q2 mirrors Q1, but options are swapped so Q1=Q2 is impossible
				// Q1=A → optA='A' → Q2 must be A → Q2=A → optA='B' → Q1 must be B → contradiction
				rule: { type: "answer_of_question", questionIndex: 0 },
			},
		],
	};
	const impSol = solve(impossible, undefined, 5);
	// Check via brute force too
	const impBrute = bruteForce(impossible);
	assertEq(impSol.length, impBrute.length, "impossible puzzle: solver agrees with brute force");

	// Puzzle with multiple solutions: two answer_is_self questions (any combo works)
	const multi: Puzzle = {
		id: "multi",
		title: "Multi",
		difficulty: 1,
		questions: [
			{
				text: "Q1",
				options: [{ label: "A" }, { label: "B" }, { label: "C" }, { label: "D" }, { label: "E" }],
				rule: { type: "answer_is_self" },
			},
			{
				text: "Q2",
				options: [{ label: "A" }, { label: "B" }, { label: "C" }, { label: "D" }, { label: "E" }],
				rule: { type: "answer_is_self" },
			},
		],
	};
	const multiSol = solve(multi, undefined, 30);
	const multiBrute = bruteForce(multi);
	assert(multiBrute.length === 25, `answer_is_self x2: brute force finds 25 solutions (5x5)`);
	assertEq(
		multiSol.length,
		25,
		"multi-solution: solver finds all 25",
	);
}

// ════════════════════════════════════════════════
// Run all
// ════════════════════════════════════════════════

testEvaluators();
testSolver();
testSolverEdgeCases();
testGeneratedPuzzles();

console.log(`\n${passed} passed, ${failed} failed`);
if (failed > 0) process.exit(1);
