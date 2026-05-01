import type { QuestionDef, Marks } from "../engine/types.ts";
import { LETTERS } from "../engine/types.ts";
import type { Validity } from "../engine/validate.ts";
import { renderQuestionText, renderOptionLabel, renderClaimLabel } from "../engine/render.ts";
import { OptionButton } from "./OptionButton.tsx";

interface Props {
  index: number;
  question: QuestionDef;
  marks: Marks;
  validity: Validity;
  disabled?: boolean;
  focusedOption?: number | null;
  defaultFocus?: boolean;
  onOptionClick: (optionIndex: number) => void;
}

const LONG_THRESHOLD = 12;

export function QuestionRow({
  index,
  question,
  marks,
  validity,
  disabled,
  focusedOption,
  defaultFocus,
  onOptionClick,
}: Props) {
  const rule = question.questionType;
  const labels = question.options.map((opt, oi) => {
    if ("claim" in opt) return renderClaimLabel(opt.claim);
    return renderOptionLabel(rule, opt.value, oi);
  });
  const isLong = labels.some((l) => l.length > LONG_THRESHOLD);
  const hasCorrect = marks.indexOf("correct") >= 0;

  return (
    <div class="question-row" data-qi={index}>
      <div class={`validity-bar ${validity}`} />
      <div class="question-header">
        <span class="question-num">{index + 1}.</span>
        <span class="question-text">{renderQuestionText(rule)}</span>
      </div>
      <div class={`question-options ${isLong ? "options-vertical" : ""}`}>
        {question.options.map((_opt, oi) => (
          <OptionButton
            key={LETTERS[oi]}
            index={oi}
            questionIndex={index}
            label={labels[oi]}
            mark={marks[oi]}
            implied={hasCorrect && marks[oi] === "unmarked"}
            disabled={disabled || (hasCorrect && marks[oi] !== "correct")}
            focused={focusedOption === oi || (defaultFocus && oi === 0)}
            onClick={() => onOptionClick(oi)}
          />
        ))}
      </div>
    </div>
  );
}
