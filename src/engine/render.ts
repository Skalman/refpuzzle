import type { QuestionType, Claim } from "./types.ts";
import { LETTERS } from "./types.ts";

export function renderQuestionText(qt: QuestionType): string {
  switch (qt.type) {
    case "CountAnswer":
      return `How many questions have answer ${qt.answer}?`;
    case "CountAnswerBefore":
      return `How many questions before #${qt.beforeIndex + 1} have answer ${qt.answer}?`;
    case "CountAnswerAfter":
      return `How many questions after #${qt.afterIndex + 1} have answer ${qt.answer}?`;
    case "CountVowel":
      return "How many questions have a vowel as the answer?";
    case "CountConsonant":
      return "How many questions have a consonant as the answer?";
    case "MostCommonCount":
      return "How many times does the most common answer occur?";
    case "ClosestAfter":
      return `Which is the closest question after #${qt.afterIndex + 1} that has answer ${qt.answer}?`;
    case "ClosestBefore":
      return `Which is the closest question before #${qt.beforeIndex + 1} that has answer ${qt.answer}?`;
    case "FirstWith":
      return `Which is the first question with answer ${qt.answer}?`;
    case "LastWith":
      return `Which is the last question with answer ${qt.answer}?`;
    case "PrevSame":
      return "Which is the previous question that has the same answer as this one?";
    case "NextSame":
      return "Which is the next question that has the same answer as this one?";
    case "OnlySame":
      return "Which is the only other question with the same answer as this one?";
    case "SameAs":
      return "Which of these questions has the same answer as this one?";
    case "SameAsWhich":
      return `Which of these questions has the same answer as #${qt.questionIndex + 1}?`;
    case "OnlyOdd":
      return `Which is the only odd-numbered question with answer ${qt.answer}?`;
    case "OnlyEven":
      return `Which is the only even-numbered question with answer ${qt.answer}?`;
    case "ConsecIdent":
      return "Which are the only two consecutive questions with identical answers?";
    case "AnswerOf":
      return `What is the answer to question #${qt.questionIndex + 1}?`;
    case "LeastCommon":
      return "Which is the least common answer?";
    case "MostCommon":
      return "Which is the most common answer?";
    case "NoOtherHasAnswer":
      return "Which answer is not the answer to any other question?";
    case "EqualCount":
      return `Which answer appears the same number of times as ${qt.answer}?`;
    case "AnswerIsSelf":
      return "What is the answer to this question?";
    case "LetterDist":
      return `How many letters away is the answer to this question from the answer to question #${qt.questionIndex + 1}?`;
    case "TrueStmt":
      return "Which statement is the only true statement?";
  }
  qt satisfies never;
  return "";
}

const POSITIONAL_RULES = new Set([
  "ClosestAfter",
  "ClosestBefore",
  "FirstWith",
  "LastWith",
  "PrevSame",
  "NextSame",
  "OnlySame",
  "SameAs",
  "SameAsWhich",
  "OnlyOdd",
  "OnlyEven",
  "ConsecIdent",
]);

export function renderOptionLabel(qt: QuestionType, value: number | null, _qi: number): string {
  if (qt.type === "TrueStmt") return "";

  if (isLetterRule(qt.type)) return value != null ? LETTERS[value] : "?";

  if (qt.type === "ConsecIdent") {
    return value != null ? `${value + 1}-${value + 2}` : "None";
  }

  if (qt.type === "EqualCount") {
    return value != null ? LETTERS[value] : "None";
  }

  if (POSITIONAL_RULES.has(qt.type)) {
    return value != null ? String(value + 1) : "None";
  }

  return value != null ? String(value) : "None";
}

export function renderClaimLabel(claim: Claim): string {
  return (
    renderQuestionText(claim.questionType) +
    " " +
    renderOptionLabel(claim.questionType, claim.value, -1)
  );
}

const LETTER_RULES = new Set([
  "AnswerOf",
  "LeastCommon",
  "MostCommon",
  "NoOtherHasAnswer",
  "AnswerIsSelf",
]);

function isLetterRule(type: string): boolean {
  return LETTER_RULES.has(type);
}
