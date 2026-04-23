import type { OptionMark } from "../engine/types.ts";
import { LETTERS } from "../engine/types.ts";

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
  const indicator = mark === "correct" ? "✅" : showCross ? "❌" : "⬜";

  return (
    <button
      class={`option-btn ${mark} ${implied ? "implied" : ""}`}
      onClick={disabled ? undefined : onClick}
      disabled={disabled}
      title={title}
      aria-label={title}
    >
      <span class={mark === "unmarked" && !implied ? "option-indicator invisible" : "option-indicator"}>
        {indicator}
      </span>
      <span class="option-letter">{letter}.</span> {label}
    </button>
  );
}
