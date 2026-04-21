import type { OptionMark } from "../engine/types.ts";
import { LETTERS } from "../engine/types.ts";

interface Props {
  index: number;
  label: string;
  mark: OptionMark;
  onClick: () => void;
}

export function OptionButton({ index, label, mark, onClick }: Props) {
  const letter = LETTERS[index];
  const title = `${letter}: ${label}`;

  const indicator = mark === "correct" ? "\u2705" : mark === "incorrect" ? "\u274C" : "\u2B1C";

  return (
    <button class={`option-btn ${mark}`} onClick={onClick} title={title} aria-label={title}>
      <span class="option-letter">{letter}.</span> {label}
      <span class={mark === "unmarked" ? "option-indicator invisible" : "option-indicator"}>
        {indicator}
      </span>
    </button>
  );
}
