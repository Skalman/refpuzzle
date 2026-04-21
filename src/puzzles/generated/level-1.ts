import type { Puzzle } from "../../engine/types.ts";

export const level1: Puzzle[] = [
  {
    id: "level-1-1",
    title: "Beginner #1",
    difficulty: 1,
    questions: [
      {
        text: "How many questions have answer A?",
        options: [{ label: "3" }, { label: "0" }, { label: "1" }, { label: "2" }, { label: "4" }],
        rule: { type: "count_answer", answer: "A" },
      },
      {
        text: "How many questions have answer B?",
        options: [{ label: "3" }, { label: "4" }, { label: "2" }, { label: "1" }, { label: "0" }],
        rule: { type: "count_answer", answer: "B" },
      },
      {
        text: "What is the answer to question #2?",
        options: [{ label: "B" }, { label: "E" }, { label: "C" }, { label: "A" }, { label: "D" }],
        rule: { type: "answer_of_question", questionIndex: 1 },
      },
      {
        text: "What is the answer to question #1?",
        options: [{ label: "E" }, { label: "D" }, { label: "C" }, { label: "A" }, { label: "B" }],
        rule: { type: "answer_of_question", questionIndex: 0 },
      },
    ],
  },
];

export const level1Solutions = [["B", "D", "E", "E"]] as const;
