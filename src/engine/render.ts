import type { QuestionTypeDef, Claim } from "./types.ts";
import { LETTERS } from "./types.ts";

export function renderQuestionText(qt: QuestionTypeDef): string {
  switch (qt.type) {
    case "count_answer":
      return `How many questions have answer ${qt.answer}?`;
    case "count_answer_before":
      return `How many questions before #${qt.beforeIndex + 1} have answer ${qt.answer}?`;
    case "count_answer_after":
      return `How many questions after #${qt.afterIndex + 1} have answer ${qt.answer}?`;
    case "count_vowel_answers":
      return "How many questions have a vowel as the answer?";
    case "count_consonant_answers":
      return "How many questions have a consonant as the answer?";
    case "most_common_count":
      return "How many times does the most common answer occur?";
    case "closest_after":
      return `Which is the closest question after #${qt.afterIndex + 1} that has answer ${qt.answer}?`;
    case "closest_before":
      return `Which is the closest question before #${qt.beforeIndex + 1} that has answer ${qt.answer}?`;
    case "first_with_answer":
      return `Which is the first question with answer ${qt.answer}?`;
    case "last_with_answer":
      return `Which is the last question with answer ${qt.answer}?`;
    case "previous_same_answer":
      return "Which is the previous question that has the same answer as this one?";
    case "next_same_answer":
      return "Which is the next question that has the same answer as this one?";
    case "only_same_answer":
      return "Which is the only other question with the same answer as this one?";
    case "same_answer_as":
      return "Which other question has the same answer as this one?";
    case "only_odd_with_answer":
      return `Which is the only odd-numbered question with answer ${qt.answer}?`;
    case "only_even_with_answer":
      return `Which is the only even-numbered question with answer ${qt.answer}?`;
    case "consecutive_identical":
      return "Which are the only two consecutive questions with identical answers?";
    case "answer_of_question":
      return `What is the answer to question #${qt.questionIndex + 1}?`;
    case "least_common_answer":
      return "Which is the least common answer?";
    case "most_common_answer":
      return "Which is the most common answer?";
    case "unique_answer":
      return "Which answer is not the answer to any other question?";
    case "equal_count_as":
      return `Which answer appears the same number of times as ${qt.answer}?`;
    case "answer_is_self":
      return "What is the answer to this question?";
    case "letter_distance":
      return `How many letters away is the answer to this question from the answer to question #${qt.questionIndex + 1}?`;
    case "only_true_statement":
      return "Which statement is the only true statement?";
  }
  return "";
}

const POSITIONAL_RULES = new Set([
  "closest_after",
  "closest_before",
  "first_with_answer",
  "last_with_answer",
  "previous_same_answer",
  "next_same_answer",
  "only_same_answer",
  "same_answer_as",
  "only_odd_with_answer",
  "only_even_with_answer",
  "consecutive_identical",
]);

export function renderOptionLabel(qt: QuestionTypeDef, value: number | null, _qi: number): string {
  if (qt.type === "only_true_statement") return "";

  if (isLetterRule(qt.type)) return value != null ? LETTERS[value] : "?";

  if (qt.type === "consecutive_identical") {
    return value != null ? `${value + 1}-${value + 2}` : "None";
  }

  if (qt.type === "equal_count_as") {
    return value != null ? LETTERS[value] : "None";
  }

  if (POSITIONAL_RULES.has(qt.type)) {
    return value != null ? String(value + 1) : "None";
  }

  return value != null ? String(value) : "None";
}

export function renderClaimLabel(claim: Claim): string {
  return renderQuestionText(claim) + " " + renderOptionLabel(claim, claim.value, -1);
}

const LETTER_RULES = new Set([
  "answer_of_question",
  "least_common_answer",
  "most_common_answer",
  "unique_answer",
  "answer_is_self",
]);

function isLetterRule(type: string): boolean {
  return LETTER_RULES.has(type);
}
