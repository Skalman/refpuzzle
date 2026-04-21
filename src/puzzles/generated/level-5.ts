import type { Puzzle } from "../../engine/types.ts";

export const level5: Puzzle[] = [
{
	id: "level-5-1",
	title: "Expert #1",
	difficulty: 5,
	questions: [
		{
			text: "Which is the first question with answer E?",
			options: [
			{ label: "2" },
			{ label: "8" },
			{ label: "7" },
			{ label: "12" },
			{ label: "10" },
			],
			rule: {"type":"first_with_answer","answer":"E"},
		},
		{
			text: "What is the answer to question #10?",
			options: [
			{ label: "A" },
			{ label: "C" },
			{ label: "E" },
			{ label: "B" },
			{ label: "D" },
			],
			rule: {"type":"answer_of_question","questionIndex":9},
		},
		{
			text: "How many questions have answer A?",
			options: [
			{ label: "12" },
			{ label: "5" },
			{ label: "4" },
			{ label: "7" },
			{ label: "3" },
			],
			rule: {"type":"count_answer","answer":"A"},
		},
		{
			text: "What is the answer to question #6?",
			options: [
			{ label: "A" },
			{ label: "B" },
			{ label: "D" },
			{ label: "E" },
			{ label: "C" },
			],
			rule: {"type":"answer_of_question","questionIndex":5},
		},
		{
			text: "How many questions have answer C?",
			options: [
			{ label: "5" },
			{ label: "0" },
			{ label: "10" },
			{ label: "4" },
			{ label: "1" },
			],
			rule: {"type":"count_answer","answer":"C"},
		},
		{
			text: "How many questions after #3 have answer A?",
			options: [
			{ label: "1" },
			{ label: "7" },
			{ label: "2" },
			{ label: "5" },
			{ label: "0" },
			],
			rule: {"type":"count_answer_after","answer":"A","afterIndex":2},
		},
		{
			text: "Which statement is the only true statement?",
			options: [
			{
				label: "How many questions have answer B? 0",
				claim: {"type":"count_answer_equals","answer":"B","value":0},
			},
			{
				label: "How many questions have a vowel as the answer? 6",
				claim: {"type":"count_vowel_answers_equals","value":6},
			},
			{
				label: "How many questions have a consonant as the answer? 6",
				claim: {"type":"count_consonant_answers_equals","value":6},
			},
			{
				label: "How many questions have a vowel as the answer? 9",
				claim: {"type":"count_vowel_answers_equals","value":9},
			},
			{
				label: "How many questions before #11 have answer C? 0",
				claim: {"type":"count_answer_before_equals","answer":"C","beforeIndex":10,"value":0},
			},
			],
			rule: {"type":"only_true_statement"},
		},
		{
			text: "How many letters away is the answer to this question from the answer to question #11?",
			options: [
			{ label: "1" },
			{ label: "4" },
			{ label: "2" },
			{ label: "0" },
			{ label: "3" },
			],
			rule: {"type":"letter_distance","otherQuestionIndex":10},
		},
		{
			text: "What is the answer to question #8?",
			options: [
			{ label: "D" },
			{ label: "C" },
			{ label: "A" },
			{ label: "B" },
			{ label: "E" },
			],
			rule: {"type":"answer_of_question","questionIndex":7},
		},
		{
			text: "How many questions have a consonant as the answer?",
			options: [
			{ label: "10" },
			{ label: "0" },
			{ label: "12" },
			{ label: "4" },
			{ label: "1" },
			],
			rule: {"type":"count_consonant_answers"},
		},
		{
			text: "What is the answer to question #1?",
			options: [
			{ label: "B" },
			{ label: "E" },
			{ label: "C" },
			{ label: "A" },
			{ label: "D" },
			],
			rule: {"type":"answer_of_question","questionIndex":0},
		},
		{
			text: "The answer to this question is the same as the answer to question?",
			options: [
			{ label: "3" },
			{ label: "12" },
			{ label: "9" },
			{ label: "6" },
			{ label: "2" },
			],
			rule: {"type":"same_answer_as"},
		},
	],
},
];

export const level5Solutions = [
	["A","E","E","E","E","C","A","D","A","D","D","E"],
] as const;
