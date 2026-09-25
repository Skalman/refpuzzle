import type { OptionMark } from "../engine/types.ts";
import { LETTERS } from "../engine/types.ts";
import { IconCheck, IconX } from "./Icons.tsx";
import { classNames, tw } from "../lib/classNames.ts";

interface Props {
  index: number;
  questionIndex: number;
  label: string;
  mark: OptionMark;
  implied?: boolean;
  disabled?: boolean;
  /** Verified by a checkpoint: still clickable, but the click only sweeps. */
  checkpointed?: boolean;
  sweep?: boolean;
  focused?: boolean;
  onClick: () => void;
}

const CELL = tw`inline-flex items-center gap-[0.45em] rounded-md border px-[0.3rem] py-[0.35rem] text-left text-option whitespace-nowrap text-default transition-[background,border-color,opacity] duration-100 hover:not-disabled:bg-hover focus-visible:z-1 focus-visible:-outline-offset-1 disabled:cursor-not-allowed`;

/**
 * The checkpointed sweep, in the pin's green: one diagonal wave over the cells
 * a checkpoint just settled, or the one cell a click bounced off. Each cell's
 * overlay paints its window of one shared gradient, a square three board spans
 * (width plus height) wide, shifted by the cell's diagonal distance from the
 * sweep's origin (`--sweep-d`, set from script; `background-attachment: fixed`
 * would do it alone, but not on iOS). The wave moves as a background position
 * rather than a transform, so it stays unclipped in the padding box: overflow
 * would make the cell a scroll container, clip-path eats the border edge.
 * Along the gradient: a ramp to 50%, a 1.4-span plateau, a 0.4-span tail.
 */
const SWEEP = tw`relative after:pointer-events-none after:absolute after:inset-0 after:rounded-[inherit] after:bg-[linear-gradient(135deg,transparent_20%,var(--tint)_26.67%,var(--tint)_45%,transparent_50%)] after:bg-size-[calc(3*var(--sweep-span))_calc(3*var(--sweep-span))] after:bg-no-repeat after:animate-checkpoint-sweep after:[--tint:color-mix(in_srgb,var(--valid)_35%,transparent)] motion-reduce:after:animate-none motion-reduce:after:bg-none motion-reduce:after:bg-[color-mix(in_srgb,var(--valid)_25%,transparent)]`;

export function OptionButton({
  index,
  questionIndex,
  label,
  mark,
  implied,
  disabled,
  checkpointed,
  sweep,
  focused,
  onClick,
}: Props) {
  const letter = LETTERS[index];
  const title = `${letter}: ${label}`;

  const showCross = mark === "incorrect" || implied;
  const showIcon = mark === "correct" || showCross;

  return (
    <button
      class={classNames(
        CELL,
        sweep
          ? "border-valid"
          : mark === "correct"
            ? "border-accent"
            : showCross
              ? "border-muted"
              : undefined,
        showCross && "border-dashed",
        mark === "correct" ? "bg-accent-soft" : showCross ? "bg-invalid-soft" : "bg-surface",
        implied || checkpointed ? "cursor-not-allowed" : "cursor-pointer",
        sweep && SWEEP,
      )}
      onClick={disabled ? undefined : onClick}
      disabled={disabled}
      aria-disabled={checkpointed}
      title={title}
      aria-label={title}
      tabIndex={focused ? 0 : -1}
      data-qi={questionIndex}
      data-oi={index}
      data-mark={mark}
      data-sweep={sweep || undefined}
    >
      <span class="inline-flex size-[1.4em] shrink-0 items-center justify-center">
        {showIcon ? (
          mark === "correct" ? (
            <IconCheck size="1.4em" strokeWidth={4} class="text-valid" />
          ) : (
            <IconX size="1.4em" strokeWidth={4} class="text-invalid" />
          )
        ) : (
          <span class="inline-block size-[1.4em]" />
        )}
      </span>
      <span class={showCross ? "opacity-45" : undefined}>
        <span class="text-caption font-bold opacity-45">{letter}.</span> {label}
      </span>
    </button>
  );
}
