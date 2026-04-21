import type { Puzzle } from "../../engine/types.ts";

export const level3: Puzzle[] = [
  {
    id: "level-3-1",
    title: "Medium #1",
    difficulty: 3,
    questions: [
      {
        text: "Which is the least common answer?",
        options: [{ label: "A" }, { label: "B" }, { label: "C" }, { label: "D" }, { label: "E" }],
        rule: { type: "least_common_answer" },
      },
      {
        text: "Which is the last question with answer A?",
        options: [{ label: "8" }, { label: "5" }, { label: "4" }, { label: "2" }, { label: "3" }],
        rule: { type: "last_with_answer", answer: "A" },
      },
      {
        text: "What is the answer to question #4?",
        options: [{ label: "A" }, { label: "D" }, { label: "E" }, { label: "B" }, { label: "C" }],
        rule: { type: "answer_of_question", questionIndex: 3 },
      },
      {
        text: "The answer to this question is the same as the answer to question?",
        options: [{ label: "3" }, { label: "5" }, { label: "6" }, { label: "8" }, { label: "4" }],
        rule: { type: "same_answer_as" },
      },
      {
        text: "How many questions have answer D?",
        options: [{ label: "3" }, { label: "1" }, { label: "2" }, { label: "4" }, { label: "0" }],
        rule: { type: "count_answer", answer: "D" },
      },
      {
        text: "What is the answer to question #5?",
        options: [{ label: "D" }, { label: "B" }, { label: "A" }, { label: "E" }, { label: "C" }],
        rule: { type: "answer_of_question", questionIndex: 4 },
      },
      {
        text: "What is the answer to question #2?",
        options: [{ label: "A" }, { label: "E" }, { label: "B" }, { label: "D" }, { label: "C" }],
        rule: { type: "answer_of_question", questionIndex: 1 },
      },
      {
        text: "Which is the last question with answer E?",
        options: [
          { label: "None" },
          { label: "3" },
          { label: "7" },
          { label: "1" },
          { label: "6" },
        ],
        rule: { type: "last_with_answer", answer: "E" },
      },
    ],
  },
];

export const level3Solutions = [["D", "C", "A", "A", "B", "B", "E", "C"]] as const;
