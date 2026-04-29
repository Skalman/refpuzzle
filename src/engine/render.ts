import type { ValidationRule, Claim } from "./types.ts";
import { LETTERS } from "./types.ts";

export function renderQuestionText(rule: ValidationRule): string {
  switch (rule.type) {
    case "count_answer":
      return `How many questions have answer ${rule.answer}?`;
    case "count_answer_before":
      return `How many questions before #${rule.beforeIndex + 1} have answer ${rule.answer}?`;
    case "count_answer_after":
      return `How many questions after #${rule.afterIndex + 1} have answer ${rule.answer}?`;
    case "count_vowel_answers":
      return "How many questions have a vowel as the answer?";
    case "count_consonant_answers":
      return "How many questions have a consonant as the answer?";
    case "most_common_count":
      return "How many times does the most common answer occur?";
    case "closest_after":
      return `Which is the closest question after #${rule.afterIndex + 1} that has answer ${rule.answer}?`;
    case "closest_before":
      return `Which is the closest question before #${rule.beforeIndex + 1} that has answer ${rule.answer}?`;
    case "first_with_answer":
      return `Which is the first question with answer ${rule.answer}?`;
    case "last_with_answer":
      return `Which is the last question with answer ${rule.answer}?`;
    case "previous_same_answer":
      return "Which is the previous question that has the same answer as this one?";
    case "next_same_answer":
      return "Which is the next question that has the same answer as this one?";
    case "only_same_answer":
      return "Which is the only other question with the same answer as this one?";
    case "same_answer_as":
      return "Which question has the same answer as this one?";
    case "only_odd_with_answer":
      return `Which is the only odd-numbered question with answer ${rule.answer}?`;
    case "consecutive_identical":
      return "Which are the only two consecutive questions with identical answers?";
    case "answer_of_question":
      return `What is the answer to question #${rule.questionIndex + 1}?`;
    case "least_common_answer":
      return "Which is the least common answer?";
    case "most_common_answer":
      return "Which is the most common answer?";
    case "unique_answer":
      return "Which answer is not the answer to any other question?";
    case "equal_count_as":
      return `The number of questions with answer ${rule.answer} equals the number of questions with answer?`;
    case "answer_is_self":
      return "What is the answer to this question?";
    case "letter_distance":
      return `How many letters away is the answer to this question from the answer to question #${rule.questionIndex + 1}?`;
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
  "consecutive_identical",
]);

export function renderOptionLabel(
  rule: ValidationRule,
  value: number | null,
  _qi: number,
): string {
  if (rule.type === "only_true_statement") return "";

  if (isLetterRule(rule.type)) return value != null ? LETTERS[value] : "?";

  if (rule.type === "consecutive_identical") {
    return value != null ? `${value + 1}-${value + 2}` : "None";
  }

  if (POSITIONAL_RULES.has(rule.type)) {
    return value != null ? String(value + 1) : "None";
  }

  return value != null ? String(value) : "None";
}

export function renderClaimLabel(claim: Claim): string {
  return (
    renderQuestionText(claim) + " " + renderOptionLabel(claim, claim.value, -1)
  );
}

const LETTER_RULES = new Set([
  "answer_of_question",
  "least_common_answer",
  "most_common_answer",
  "unique_answer",
  "equal_count_as",
  "answer_is_self",
]);

function isLetterRule(type: string): boolean {
  return LETTER_RULES.has(type);
}
