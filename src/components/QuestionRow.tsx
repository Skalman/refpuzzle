import { memo } from "preact/compat";
import type { QuestionDef, QuestionType, Marks } from "../engine/types.ts";
import { LETTERS } from "../engine/types.ts";
import type { Validity } from "../engine/state.ts";
import { renderQuestionText, renderOptionLabel, renderClaimLabel } from "../engine/render.ts";
import { useQuestionHighlight } from "./TutorialHighlight.ts";
import { OptionButton } from "./OptionButton.tsx";

interface Props {
  index: number;
  question: QuestionDef;
  marks: Marks;
  validity: Validity;
  disabled?: boolean;
  focusedOption?: number | null;
  defaultFocus?: boolean;
  /**
   * Question types for the TrueStmt's per-option claims. Matching values
   * live in `question.options[oi].value`. Omitted for non-TrueStmt rows.
   */
  trueStmtQuestionTypes?: QuestionType[];
  onOptionClick: (questionIndex: number, optionIndex: number) => void;
}

const LONG_THRESHOLD = 12;

function marksEqual(a: Marks, b: Marks): boolean {
  for (let i = 0; i < 5; i++) if (a[i] !== b[i]) return false;
  return true;
}

export const QuestionRow = memo(
  function QuestionRow({
    index,
    question,
    marks,
    validity,
    disabled,
    focusedOption,
    defaultFocus,
    trueStmtQuestionTypes,
    onOptionClick,
  }: Props) {
    const { highlighted, mute } = useQuestionHighlight(index);
    const rule = question.questionType;
    const labels = question.options.map((opt, oi) => {
      if (trueStmtQuestionTypes && rule.type === "TrueStmt") {
        return renderClaimLabel({
          questionType: trueStmtQuestionTypes[oi],
          value: opt.value ?? -1,
        });
      }
      return renderOptionLabel(rule, opt.value, oi);
    });
    const isLong = labels.some((l) => l.length > LONG_THRESHOLD);
    const hasCorrect = marks.indexOf("correct") >= 0;

    return (
      <div
        class={`question-row${highlighted ? " tutorial-highlight" : ""}${mute ? " tutorial-mute-options" : ""}`}
        data-qi={index}
      >
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
              onClick={() => onOptionClick(index, oi)}
            />
          ))}
        </div>
      </div>
    );
  },
  (prev, next) =>
    prev.index === next.index &&
    prev.question === next.question &&
    marksEqual(prev.marks, next.marks) &&
    prev.validity === next.validity &&
    prev.disabled === next.disabled &&
    prev.focusedOption === next.focusedOption &&
    prev.defaultFocus === next.defaultFocus &&
    prev.onOptionClick === next.onOptionClick,
);
