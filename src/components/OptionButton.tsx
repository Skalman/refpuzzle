import type { OptionMark } from "../engine/types.ts";
import { LETTERS } from "../engine/types.ts";
import { IconCheck, IconX } from "./Icons.tsx";

interface Props {
  index: number;
  label: string;
  mark: OptionMark;
  implied?: boolean;
  disabled?: boolean;
  onClick: () => void;
}

export function OptionButton({ index, label, mark, implied, disabled, onClick }: Props) {
  const letter = LETTERS[index];
  const title = `${letter}: ${label}`;

  const showCross = mark === "incorrect" || implied;
  const showIcon = mark === "correct" || showCross;

  return (
    <button
      class={`option-btn ${mark} ${implied ? "implied" : ""}`}
      onClick={disabled ? undefined : onClick}
      disabled={disabled}
      title={title}
      aria-label={title}
    >
      <span class="option-indicator">
        {showIcon
          ? (mark === "correct"
              ? <IconCheck size="1.4em" strokeWidth={4} class="icon-correct" />
              : <IconX size="1.4em" strokeWidth={4} class="icon-incorrect" />)
          : <span class="option-indicator-spacer" />}
      </span>
      <span class="option-text"><span class="option-letter">{letter}.</span> {label}</span>
    </button>
  );
}
