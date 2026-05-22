import type { QuestionType } from "./types.ts";

export function formatTypeTag(qt: QuestionType): string {
  switch (qt.type) {
    case "CountAnswer":
    case "FirstWith":
    case "LastWith":
    case "OnlyOdd":
    case "OnlyEven":
      return `${qt.type}(${qt.answer})`;
    case "CountAnswerBefore":
      return `CountAnswerBefore(${qt.answer},q=${String(qt.beforeIndex)})`;
    case "CountAnswerAfter":
      return `CountAnswerAfter(${qt.answer},q=${String(qt.afterIndex)})`;
    case "ClosestAfter":
      return `ClosestAfter(${qt.answer},q=${String(qt.afterIndex)})`;
    case "ClosestBefore":
      return `ClosestBefore(${qt.answer},q=${String(qt.beforeIndex)})`;
    case "AnswerOf":
    case "LetterDist":
    case "SameAsWhich":
      return `${qt.type}(q=${String(qt.questionIndex)})`;
    case "EqualCount":
      return `EqualCount(${qt.answer})`;
    default:
      return qt.type;
  }
}
