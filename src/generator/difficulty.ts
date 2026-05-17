import type { QuestionType } from "../engine/types.ts";

export interface DifficultyProfile {
  level: 1 | 2 | 3 | 4 | 5;
  name: string;
  questionCount: number;
  allowedTypes: QuestionType["type"][];
}

export const profiles: DifficultyProfile[] = [
  {
    level: 1,
    name: "Beginner",
    questionCount: 4,
    allowedTypes: ["CountAnswer", "AnswerOf", "AnswerIsSelf", "FirstWith", "LastWith"],
  },
  {
    level: 2,
    name: "Easy",
    questionCount: 5,
    allowedTypes: [
      "CountAnswer",
      "AnswerOf",
      "AnswerIsSelf",
      "ClosestAfter",
      "ClosestBefore",
      "FirstWith",
      "LastWith",

      "NextSame",
      "PrevSame",
    ],
  },
  {
    level: 3,
    name: "Medium",
    questionCount: 8,
    allowedTypes: [
      "CountAnswer",
      "AnswerOf",
      "AnswerIsSelf",
      "ClosestAfter",
      "ClosestBefore",
      "FirstWith",
      "LastWith",

      "NextSame",
      "PrevSame",
      "LeastCommon",
      "MostCommon",
      "CountAnswerBefore",
      "CountAnswerAfter",
      "CountVowel",
      "CountConsonant",
      "NoOtherHasAnswer",
      "OnlySame",
    ],
  },
  {
    level: 4,
    name: "Hard",
    questionCount: 10,
    allowedTypes: [
      "CountAnswer",
      "AnswerOf",
      "AnswerIsSelf",
      "ClosestAfter",
      "ClosestBefore",
      "FirstWith",
      "LastWith",

      "NextSame",
      "PrevSame",
      "LeastCommon",
      "MostCommon",
      "MostCommonCount",
      "CountAnswerBefore",
      "CountAnswerAfter",
      "CountVowel",
      "CountConsonant",
      "NoOtherHasAnswer",
      "OnlySame",
      "LetterDist",
      "EqualCount",
      "ConsecIdent",
      "OnlyOdd",
      "OnlyEven",
    ],
  },
  {
    level: 5,
    name: "Expert",
    questionCount: 12,
    allowedTypes: [
      "CountAnswer",
      "AnswerOf",
      "AnswerIsSelf",
      "ClosestAfter",
      "ClosestBefore",
      "FirstWith",
      "LastWith",

      "NextSame",
      "PrevSame",
      "LeastCommon",
      "MostCommon",
      "MostCommonCount",
      "CountAnswerBefore",
      "CountAnswerAfter",
      "CountVowel",
      "CountConsonant",
      "NoOtherHasAnswer",
      "OnlySame",
      "LetterDist",
      "EqualCount",
      "ConsecIdent",
      "OnlyOdd",
      "OnlyEven",
      "TrueStmt",
    ],
  },
];
