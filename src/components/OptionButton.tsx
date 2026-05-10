import type { OptionMark } from "../engine/types.ts";
import { LETTERS } from "../engine/types.ts";
import { IconCheck, IconX } from "./Icons.tsx";
import { useOptionHighlight } from "./TutorialHighlight.ts";

interface Props {
  index: number;
  questionIndex: number;
  label: string;
  mark: OptionMark;
  implied?: boolean;
  disabled?: boolean;
  focused?: boolean;
  onClick: () => void;
}

export function OptionButton({
  index,
  questionIndex,
  label,
  mark,
  implied,
  disabled,
  focused,
  onClick,
}: Props) {
  const tutorialHighlight = useOptionHighlight(questionIndex, index);
  const letter = LETTERS[index];
  const title = `${letter}: ${label}`;

  const showCross = mark === "incorrect" || implied;
  const showIcon = mark === "correct" || showCross;

  return (
    <button
      class={`option-btn ${mark} ${implied ? "implied" : ""}${tutorialHighlight ? " tutorial-option-highlight" : ""}`}
      onClick={disabled ? undefined : onClick}
      disabled={disabled}
      title={title}
      aria-label={title}
      tabIndex={focused ? 0 : -1}
      data-qi={questionIndex}
      data-oi={index}
    >
      <span class="option-indicator">
        {showIcon ? (
          mark === "correct" ? (
            <IconCheck size="1.4em" strokeWidth={4} class="icon-correct" />
          ) : (
            <IconX size="1.4em" strokeWidth={4} class="icon-incorrect" />
          )
        ) : (
          <span class="option-indicator-spacer" />
        )}
      </span>
      <span class="option-text">
        <span class="option-letter">{letter}.</span> {label}
      </span>
    </button>
  );
}
