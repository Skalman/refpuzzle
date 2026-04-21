import type { Puzzle } from "../../engine/types.ts";

export const level5: Puzzle[] = [
{
	id: "level-5-1",
	title: "Expert #1",
	difficulty: 5,
	questions: [
		{
			text: "Which statement is the only true statement?",
			options: [
			{
				label: "How many questions have a vowel as the answer? 1",
				claim: {"type":"count_vowel_answers_equals","value":1},
			},
			{
				label: "How many questions before #10 have answer B? 5",
				claim: {"type":"count_answer_before_equals","answer":"B","beforeIndex":9,"value":5},
			},
			{
				label: "How many questions before #8 have answer A? 1",
				claim: {"type":"count_answer_before_equals","answer":"A","beforeIndex":7,"value":1},
			},
			{
				label: "How many questions before #10 have answer E? 2",
				claim: {"type":"count_answer_before_equals","answer":"E","beforeIndex":9,"value":2},
			},
			{
				label: "How many questions have a consonant as the answer? 8",
				claim: {"type":"count_consonant_answers_equals","value":8},
			},
			],
			rule: {"type":"only_true_statement"},
		},
		{
			text: "What is the answer to question #11?",
			options: [
			{ label: "E" },
			{ label: "D" },
			{ label: "A" },
			{ label: "C" },
			{ label: "B" },
			],
			rule: {"type":"answer_of_question","questionIndex":10},
		},
		{
			text: "Which is the last question with answer A?",
			options: [
			{ label: "3" },
			{ label: "8" },
			{ label: "1" },
			{ label: "5" },
			{ label: "7" },
			],
			rule: {"type":"last_with_answer","answer":"A"},
		},
		{
			text: "Which is the next question that has the same answer as this one?",
			options: [
			{ label: "4" },
			{ label: "None" },
			{ label: "10" },
			{ label: "7" },
			{ label: "8" },
			],
			rule: {"type":"next_same_answer"},
		},
		{
			text: "What is the answer to question #10?",
			options: [
			{ label: "A" },
			{ label: "B" },
			{ label: "D" },
			{ label: "E" },
			{ label: "C" },
			],
			rule: {"type":"answer_of_question","questionIndex":9},
		},
		{
			text: "How many letters away is the answer to this question from the answer to question #9?",
			options: [
			{ label: "0" },
			{ label: "3" },
			{ label: "4" },
			{ label: "1" },
			{ label: "2" },
			],
			rule: {"type":"letter_distance","otherQuestionIndex":8},
		},
		{
			text: "What is the answer to question #1?",
			options: [
			{ label: "A" },
			{ label: "E" },
			{ label: "D" },
			{ label: "C" },
			{ label: "B" },
			],
			rule: {"type":"answer_of_question","questionIndex":0},
		},
		{
			text: "Which is the last question with answer C?",
			options: [
			{ label: "8" },
			{ label: "1" },
			{ label: "12" },
			{ label: "4" },
			{ label: "3" },
			],
			rule: {"type":"last_with_answer","answer":"C"},
		},
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
			text: "How many questions have answer E?",
			options: [
			{ label: "7" },
			{ label: "1" },
			{ label: "9" },
			{ label: "5" },
			{ label: "10" },
			],
			rule: {"type":"count_answer","answer":"E"},
		},
		{
			text: "Which is the previous question that has the same answer as this one?",
			options: [
			{ label: "10" },
			{ label: "8" },
			{ label: "3" },
			{ label: "7" },
			{ label: "5" },
			],
			rule: {"type":"previous_same_answer"},
		},
		{
			text: "Which is the closest question before #6 that has answer D?",
			options: [
			{ label: "3" },
			{ label: "4" },
			{ label: "None" },
			{ label: "5" },
			{ label: "1" },
			],
			rule: {"type":"closest_before","beforeIndex":5,"answer":"D"},
		},
	],
},
];

export const level5Solutions = [
	["C","B","A","D","B","B","D","B","E","B","D","B"],
] as const;
