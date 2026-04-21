import type { Puzzle } from "../../engine/types.ts";

export const level1: Puzzle[] = [
{
	id: "level-1-1",
	title: "Beginner #1",
	difficulty: 1,
	questions: [
		{
			text: "What is the answer to question #4?",
			options: [
			{ label: "E" },
			{ label: "A" },
			{ label: "C" },
			{ label: "B" },
			{ label: "D" },
			],
			rule: {"type":"answer_of_question","questionIndex":3},
		},
		{
			text: "How many questions have answer D?",
			options: [
			{ label: "2" },
			{ label: "1" },
			{ label: "0" },
			{ label: "3" },
			{ label: "4" },
			],
			rule: {"type":"count_answer","answer":"D"},
		},
		{
			text: "What is the answer to question #2?",
			options: [
			{ label: "A" },
			{ label: "B" },
			{ label: "D" },
			{ label: "C" },
			{ label: "E" },
			],
			rule: {"type":"answer_of_question","questionIndex":1},
		},
		{
			text: "How many questions have answer A?",
			options: [
			{ label: "2" },
			{ label: "0" },
			{ label: "1" },
			{ label: "3" },
			{ label: "4" },
			],
			rule: {"type":"count_answer","answer":"A"},
		},
	],
},
];

export const level1Solutions = [
	["D","B","B","B"],
] as const;
