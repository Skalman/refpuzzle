import type { Puzzle } from "../../engine/types.ts";

export const level2: Puzzle[] = [
{
	id: "level-2-1",
	title: "Easy #1",
	difficulty: 2,
	questions: [
		{
			text: "How many questions have answer E?",
			options: [
			{ label: "4" },
			{ label: "3" },
			{ label: "2" },
			{ label: "1" },
			{ label: "0" },
			],
			rule: {"type":"count_answer","answer":"E"},
		},
		{
			text: "Which is the last question with answer B?",
			options: [
			{ label: "5" },
			{ label: "3" },
			{ label: "2" },
			{ label: "1" },
			{ label: "None" },
			],
			rule: {"type":"last_with_answer","answer":"B"},
		},
		{
			text: "What is the answer to question #2?",
			options: [
			{ label: "B" },
			{ label: "E" },
			{ label: "C" },
			{ label: "D" },
			{ label: "A" },
			],
			rule: {"type":"answer_of_question","questionIndex":1},
		},
		{
			text: "Which is the last question with answer C?",
			options: [
			{ label: "4" },
			{ label: "2" },
			{ label: "3" },
			{ label: "None" },
			{ label: "1" },
			],
			rule: {"type":"last_with_answer","answer":"C"},
		},
		{
			text: "What is the answer to question #1?",
			options: [
			{ label: "B" },
			{ label: "C" },
			{ label: "A" },
			{ label: "E" },
			{ label: "D" },
			],
			rule: {"type":"answer_of_question","questionIndex":0},
		},
	],
},
];

export const level2Solutions = [
	["C","A","E","E","B"],
] as const;
