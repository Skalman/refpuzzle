import type { QuestionDef, Marks } from "../engine/types.ts";
import type { Validity } from "../engine/validate.ts";
import { OptionButton } from "./OptionButton.tsx";

interface Props {
  index: number;
  question: QuestionDef;
  marks: Marks;
  validity: Validity;
  onOptionClick: (optionIndex: number) => void;
}

const LONG_THRESHOLD = 12;

export function QuestionRow({ index, question, marks, validity, onOptionClick }: Props) {
  const isLong = question.options.some((o) => o.label.length > LONG_THRESHOLD);
  const hasCorrect = marks.indexOf("correct") >= 0;

  return (
    <div class="question-row">
      <div class={`validity-bar ${validity}`} />
      <div class="question-header">
        <span class="question-num">{index + 1}.</span>
        <span class="question-text">{question.text}</span>
      </div>
      <div class={`question-options ${isLong ? "options-vertical" : ""}`}>
        {question.options.map((opt, oi) => (
          <OptionButton
            key={opt.label}
            index={oi}
            label={opt.label}
            mark={marks[oi]}
            implied={hasCorrect && marks[oi] === "unmarked"}
            disabled={hasCorrect && marks[oi] !== "correct"}
            onClick={() => onOptionClick(oi)}
          />
        ))}
      </div>
    </div>
  );
}
