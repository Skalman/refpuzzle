import type { Puzzle } from "../../engine/types.ts";

export const level3: Puzzle[] = [
{
	id: "level-3-1",
	title: "Medium #1",
	difficulty: 3,
	questions: [
		{
			text: "Which is the closest question before #6 that has answer C?",
			options: [
			{ label: "2" },
			{ label: "3" },
			{ label: "None" },
			{ label: "1" },
			{ label: "4" },
			],
			rule: {"type":"closest_before","beforeIndex":5,"answer":"C"},
		},
		{
			text: "Which is the least common answer?",
			options: [
			{ label: "A" },
			{ label: "B" },
			{ label: "C" },
			{ label: "D" },
			{ label: "E" },
			],
			rule: {"type":"least_common_answer"},
		},
		{
			text: "What is the answer to question #7?",
			options: [
			{ label: "E" },
			{ label: "D" },
			{ label: "A" },
			{ label: "C" },
			{ label: "B" },
			],
			rule: {"type":"answer_of_question","questionIndex":6},
		},
		{
			text: "How many questions have answer E?",
			options: [
			{ label: "1" },
			{ label: "2" },
			{ label: "3" },
			{ label: "6" },
			{ label: "5" },
			],
			rule: {"type":"count_answer","answer":"E"},
		},
		{
			text: "Which is the closest question after #2 that has answer A?",
			options: [
			{ label: "7" },
			{ label: "5" },
			{ label: "6" },
			{ label: "8" },
			{ label: "4" },
			],
			rule: {"type":"closest_after","afterIndex":1,"answer":"A"},
		},
		{
			text: "What is the answer to question #5?",
			options: [
			{ label: "D" },
			{ label: "E" },
			{ label: "C" },
			{ label: "A" },
			{ label: "B" },
			],
			rule: {"type":"answer_of_question","questionIndex":4},
		},
		{
			text: "How many questions have answer C?",
			options: [
			{ label: "3" },
			{ label: "4" },
			{ label: "0" },
			{ label: "1" },
			{ label: "7" },
			],
			rule: {"type":"count_answer","answer":"C"},
		},
		{
			text: "Which is the last question with answer C?",
			options: [
			{ label: "8" },
			{ label: "6" },
			{ label: "3" },
			{ label: "2" },
			{ label: "7" },
			],
			rule: {"type":"last_with_answer","answer":"C"},
		},
	],
},
];

export const level3Solutions = [
	["A","C","B","A","E","B","D","D"],
] as const;
