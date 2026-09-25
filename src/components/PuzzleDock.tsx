import type { ComponentChildren, Ref } from "preact";
import type { ExplainStep } from "../engine/hint-types.ts";
import { arrowNavHandler } from "../lib/keyboard.ts";
import { t } from "../i18n/index.ts";
import { HintStep } from "./HintStep.tsx";
import { IconUndo, IconRedo, IconPin, IconHint } from "./Icons.tsx";
import { Button, ButtonLink } from "./ui/Button.tsx";

function HintBox({ children }: { children: ComponentChildren }) {
  return (
    <div
      class="mt-3 flex items-center justify-between gap-2 rounded-lg border border-accent bg-accent-soft px-4 py-[0.6rem] text-body text-accent"
      data-testid="hint-panel"
    >
      {children}
    </div>
  );
}

/** The hint so far, with a button for its next step while there is one. */
export function HintPanel({ step, onMore }: { step: ExplainStep; onMore?: () => void }) {
  const s = t();
  return (
    <HintBox>
      <HintStep step={step} />
      {onMore && (
        <Button variant="outline" size="sm" class="shrink-0" onClick={onMore}>
          {s.puzzle.more}
        </Button>
      )}
    </HintBox>
  );
}

/** Every step of the hint at once, the way debug mode shows it. */
export function DebugHintPanel({ steps }: { steps: ExplainStep[] }) {
  return (
    <HintBox>
      <ol class="list-decimal">
        {steps.map((step, i) => (
          // oxlint-disable-next-line react/no-array-index-key
          <li key={i}>
            <HintStep step={step} />
          </li>
        ))}
      </ol>
    </HintBox>
  );
}

/** The Checkpoint button's verdict; a click anywhere on it dismisses it. */
export function CheckpointNote({ text, onDismiss }: { text: string; onDismiss: () => void }) {
  const s = t();
  return (
    <div
      class="mt-3 flex cursor-pointer items-center gap-3 rounded-lg border bg-surface px-4 py-[0.6rem] text-body text-muted"
      role="status"
      onClick={onDismiss}
    >
      <span>{text}</span>
      <button
        class="ms-auto flex-none cursor-pointer border-none bg-transparent p-0 text-[1.1em] leading-none text-inherit opacity-70 hover:opacity-100"
        aria-label={s.aria.dismiss}
        onClick={onDismiss}
      >
        &times;
      </button>
    </div>
  );
}

/** The controls a press can land on, for the toolbar's arrow keys and tab stop. */
export const ENABLED_CONTROL = "button:not(:disabled)";

/** Undo, redo, checkpoint and hint, as one arrow-key toolbar. */
export function PuzzleControls({
  toolbarRef,
  checkpointRef,
  hintRef,
  canUndo,
  canRedo,
  canCheckpoint,
  onUndo,
  onRedo,
  onCheckpoint,
  onHint,
  onHintIntent,
}: {
  toolbarRef: Ref<HTMLDivElement>;
  checkpointRef: Ref<HTMLButtonElement>;
  hintRef: Ref<HTMLButtonElement>;
  canUndo: boolean;
  canRedo: boolean;
  canCheckpoint: boolean;
  onUndo: () => void;
  onRedo: () => void;
  onCheckpoint: () => void;
  onHint: () => void;
  /** A pointer or focus nearing the Hint button, so the solution can load ahead. */
  onHintIntent: () => void;
}) {
  const s = t();
  return (
    <div
      ref={toolbarRef}
      class="flex flex-none items-center gap-1 py-2"
      role="toolbar"
      onKeyDown={arrowNavHandler(ENABLED_CONTROL)}
    >
      <Button
        variant="outline-muted"
        size="icon"
        icon={<IconUndo />}
        onClick={onUndo}
        disabled={!canUndo}
        title={s.puzzle.undo}
      />
      <Button
        variant="outline-muted"
        size="icon"
        icon={<IconRedo />}
        onClick={onRedo}
        disabled={!canRedo}
        title={s.puzzle.redo}
      />
      <Button
        variant="ghost"
        size="md-compact"
        icon={<IconPin class="text-valid in-disabled:text-inherit" />}
        ref={checkpointRef}
        onClick={onCheckpoint}
        disabled={!canCheckpoint}
      >
        {s.puzzle.checkpoint}
      </Button>
      <Button
        variant="ghost"
        size="md-compact"
        icon={<IconHint class="text-pending" />}
        ref={hintRef}
        onClick={onHint}
        onMouseEnter={onHintIntent}
        onFocus={onHintIntent}
        onTouchStart={onHintIntent}
        title={s.puzzle.hint}
      >
        {s.puzzle.hint}
      </Button>
    </div>
  );
}

/**
 * The solved board's ways onward: its summary, then the next level, or the
 * archive after the last. `quiet` while the solved dialog carries the same two.
 * The auto margin holds it to the row's end whether it shares the track's line
 * or wraps below it, level with an expanded track's first row.
 */
export function CompletionBar({
  barRef,
  nextRef,
  quiet,
  hasNext,
  onSummary,
  onNext,
}: {
  barRef: Ref<HTMLDivElement>;
  /** A callback, so one ref takes whichever element renders: the button or the link. */
  nextRef: (el: HTMLElement | null) => void;
  quiet: boolean;
  hasNext: boolean;
  onSummary: () => void;
  onNext: () => void;
}) {
  const s = t();
  // Only the colors change, so going quiet doesn't resize the button.
  const nextVariant = quiet ? "next-muted" : "next";
  return (
    <div
      ref={barRef}
      class="ms-auto flex flex-none items-center gap-2 self-start py-2"
      data-testid="completion-bar"
      aria-label={s.puzzle.solved}
    >
      <Button variant="ghost" onClick={onSummary}>
        {s.puzzle.summary}
      </Button>
      {hasNext ? (
        <Button variant={nextVariant} ref={nextRef} onClick={onNext}>
          {s.puzzle.nextPuzzle} &rarr;
        </Button>
      ) : (
        <ButtonLink variant={nextVariant} ref={nextRef} href="/archive">
          {s.daily.archive} &rarr;
        </ButtonLink>
      )}
    </div>
  );
}
