import { memo } from "preact/compat";
import type { RenderedQuestion, Marks } from "../engine/types.ts";
import { LETTERS } from "../engine/types.ts";
import type { Validity } from "../engine/state.ts";
import { OptionButton } from "./OptionButton.tsx";
import { classNames, tw } from "../lib/classNames.ts";

interface Props {
  index: number;
  question: RenderedQuestion;
  marks: Marks;
  validity: Validity;
  disabled?: boolean;
  /** Bitmask of options this question had marked at the last checkpoint. */
  checkpointedMask?: number;
  /** Bitmask of options playing the checkpointed sweep right now. */
  sweepMask?: number;
  focusedOption?: number | null;
  defaultFocus?: boolean;
  onOptionClick: (questionIndex: number, optionIndex: number) => void;
}

const LONG_THRESHOLD = 12;

/**
 * The bar down a question's left edge, in styles that read without color.
 * Widths stay in px: the pending and invalid states are dotted and double
 * borders, which need whole pixels to render as two lines rather than
 * collapsing to solid at the smaller root font sizes.
 */
const VALIDITY_BAR: Record<Validity, string> = {
  neutral: tw`relative w-[4px] before:absolute before:inset-y-0 before:right-px before:left-0 before:rounded-full before:bg-neutral-bar`,
  valid: tw`w-[4px] rounded-full bg-valid`,
  consistent: tw`w-[4px] rounded-full bg-valid`,
  pending: tw`w-0 border-0 border-l-4 border-dotted border-pending`,
  invalid: tw`w-0 border-0 border-l-4 border-double border-invalid`,
};

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
    checkpointedMask = 0,
    sweepMask = 0,
    focusedOption,
    defaultFocus,
    onOptionClick,
  }: Props) {
    const labels = question.options;
    const isLong = labels.some((l) => l.length > LONG_THRESHOLD);
    const hasCorrect = marks.indexOf("correct") >= 0;

    return (
      <div
        class="group grid scroll-mb-20 grid-cols-[auto_1fr] grid-rows-[auto_auto] border-b py-2 lg:row-span-2 lg:grid-rows-subgrid"
        data-row={index}
      >
        <div class={`row-span-full mr-2 shrink-0 self-stretch ${VALIDITY_BAR[validity]}`} />
        <div class="col-start-2 mb-[0.3rem] flex gap-[0.4rem]">
          <span class="shrink-0 text-body font-bold text-muted group-has-focus-visible:text-accent">
            {index + 1}.
          </span>
          <span class="text-body group-has-focus-visible:text-accent">{question.text}</span>
        </div>
        <div
          class={classNames(
            "col-start-2 flex self-start *:flex-1",
            isLong ? "flex-col gap-[0.2rem] *:whitespace-normal" : "gap-1",
          )}
        >
          {question.options.map((_label, oi) => (
            <OptionButton
              key={LETTERS[oi]}
              index={oi}
              questionIndex={index}
              label={labels[oi]}
              mark={marks[oi]}
              implied={hasCorrect && marks[oi] === "unmarked"}
              disabled={disabled || (hasCorrect && marks[oi] !== "correct")}
              checkpointed={((checkpointedMask >> oi) & 1) === 1}
              sweep={((sweepMask >> oi) & 1) === 1}
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
    prev.checkpointedMask === next.checkpointedMask &&
    prev.sweepMask === next.sweepMask &&
    prev.focusedOption === next.focusedOption &&
    prev.defaultFocus === next.defaultFocus &&
    prev.onOptionClick === next.onOptionClick,
);
