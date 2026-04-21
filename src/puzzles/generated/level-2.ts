import type { Puzzle } from "../../engine/types.ts";

export const level2: Puzzle[] = [
  {
    id: "level-2-1",
    title: "Easy #1",
    difficulty: 2,
    questions: [
      {
        text: "Which is the last question with answer E?",
        options: [
          { label: "5" },
          { label: "1" },
          { label: "2" },
          { label: "4" },
          { label: "None" },
        ],
        rule: { type: "last_with_answer", answer: "E" },
      },
      {
        text: "Which is the closest question after #1 that has answer B?",
        options: [
          { label: "None" },
          { label: "5" },
          { label: "4" },
          { label: "3" },
          { label: "2" },
        ],
        rule: { type: "closest_after", afterIndex: 0, answer: "B" },
      },
      {
        text: "What is the answer to question #4?",
        options: [{ label: "B" }, { label: "E" }, { label: "C" }, { label: "D" }, { label: "A" }],
        rule: { type: "answer_of_question", questionIndex: 3 },
      },
      {
        text: "How many questions have answer D?",
        options: [{ label: "4" }, { label: "1" }, { label: "0" }, { label: "3" }, { label: "2" }],
        rule: { type: "count_answer", answer: "D" },
      },
      {
        text: "How many questions have answer E?",
        options: [{ label: "4" }, { label: "5" }, { label: "1" }, { label: "0" }, { label: "3" }],
        rule: { type: "count_answer", answer: "E" },
      },
    ],
  },
];

export const level2Solutions = [["D", "D", "B", "E", "C"]] as const;
