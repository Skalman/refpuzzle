import type { Puzzle } from "../../engine/types.ts";

export const level4: Puzzle[] = [
  {
    id: "level-4-1",
    title: "Hard #1",
    difficulty: 4,
    questions: [
      {
        text: "Which is the last question with answer D?",
        options: [{ label: "7" }, { label: "1" }, { label: "8" }, { label: "4" }, { label: "6" }],
        rule: { type: "last_with_answer", answer: "D" },
      },
      {
        text: "What is the answer to question #6?",
        options: [{ label: "B" }, { label: "E" }, { label: "D" }, { label: "C" }, { label: "A" }],
        rule: { type: "answer_of_question", questionIndex: 5 },
      },
      {
        text: "The answer to this question is the same as the answer to question?",
        options: [
          { label: "7" },
          { label: "5" },
          { label: "10" },
          { label: "8" },
          { label: "None" },
        ],
        rule: { type: "same_answer_as" },
      },
      {
        text: "What is the answer to question #8?",
        options: [{ label: "D" }, { label: "E" }, { label: "C" }, { label: "B" }, { label: "A" }],
        rule: { type: "answer_of_question", questionIndex: 7 },
      },
      {
        text: "The only two consecutive questions with identical answers are?",
        options: [
          { label: "9 and 10" },
          { label: "1 and 2" },
          { label: "5 and 6" },
          { label: "6 and 7" },
          { label: "4 and 5" },
        ],
        rule: { type: "consecutive_identical" },
      },
      {
        text: "How many questions after #1 have answer B?",
        options: [{ label: "9" }, { label: "2" }, { label: "4" }, { label: "6" }, { label: "8" }],
        rule: { type: "count_answer_after", answer: "B", afterIndex: 0 },
      },
      {
        text: "Which is the last question with answer A?",
        options: [
          { label: "4" },
          { label: "3" },
          { label: "1" },
          { label: "6" },
          { label: "None" },
        ],
        rule: { type: "last_with_answer", answer: "A" },
      },
      {
        text: "Which is the closest question before #9 that has answer E?",
        options: [{ label: "1" }, { label: "7" }, { label: "8" }, { label: "3" }, { label: "5" }],
        rule: { type: "closest_before", beforeIndex: 8, answer: "E" },
      },
      {
        text: "How many questions have answer D?",
        options: [{ label: "7" }, { label: "8" }, { label: "9" }, { label: "10" }, { label: "3" }],
        rule: { type: "count_answer", answer: "D" },
      },
      {
        text: "Which is the previous question that has the same answer as this one?",
        options: [
          { label: "None" },
          { label: "8" },
          { label: "10" },
          { label: "1" },
          { label: "3" },
        ],
        rule: { type: "previous_same_answer" },
      },
    ],
  },
];

export const level4Solutions = [["D", "D", "B", "D", "B", "C", "E", "B", "E", "B"]] as const;
