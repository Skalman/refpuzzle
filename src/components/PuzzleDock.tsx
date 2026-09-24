import type { ComponentChildren, Ref } from "preact";
import type { ExplainStep } from "../engine/hint-types.ts";
import { arrowNavHandler } from "../lib/keyboard.ts";
import { classNames } from "../lib/classNames.ts";
import { t } from "../i18n/index.ts";
import { HintStep } from "./HintStep.tsx";
import { IconUndo, IconRedo, IconPin, IconHint } from "./Icons.tsx";
import { Button, ButtonLink } from "./ui/Button.tsx";

function HintBox({ children }: { children: ComponentChildren }) {
  return (
    <div class="puzzle-hint" data-testid="hint-panel">
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
        <button class="hint-more" onClick={onMore}>
          {s.puzzle.more}
        </button>
      )}
    </HintBox>
  );
}

/** Every step of the hint at once, the way debug mode shows it. */
export function DebugHintPanel({ steps }: { steps: ExplainStep[] }) {
  return (
    <HintBox>
      <ol>
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
    <div class="puzzle-note" role="status" onClick={onDismiss}>
      <span>{text}</span>
      <button class="note-dismiss" aria-label={s.aria.dismiss} onClick={onDismiss}>
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
      class="puzzle-controls"
      role="toolbar"
      onKeyDown={arrowNavHandler(ENABLED_CONTROL)}
    >
      <Button variant="icon" onClick={onUndo} disabled={!canUndo} title={s.puzzle.undo}>
        <IconUndo />
      </Button>
      <Button variant="icon" onClick={onRedo} disabled={!canRedo} title={s.puzzle.redo}>
        <IconRedo />
      </Button>
      <Button variant="text" ref={checkpointRef} onClick={onCheckpoint} disabled={!canCheckpoint}>
        <IconPin size="0.9em" class="icon-checkpoint" /> {s.puzzle.checkpoint}
      </Button>
      <Button
        variant="text"
        ref={hintRef}
        onClick={onHint}
        onMouseEnter={onHintIntent}
        onFocus={onHintIntent}
        onTouchStart={onHintIntent}
        title={s.puzzle.hint}
      >
        <IconHint size="0.9em" class="icon-hint" /> {s.puzzle.hint}
      </Button>
    </div>
  );
}

/**
 * The solved board's ways onward: its summary, then the next level, or the
 * archive after the last. `quiet` while the solved dialog carries the same two.
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
  return (
    <div
      ref={barRef}
      class={classNames("puzzle-complete", quiet && "quiet")}
      data-testid="completion-bar"
      aria-label={s.puzzle.solved}
    >
      <Button variant="text" onClick={onSummary}>
        {s.puzzle.summary}
      </Button>
      {hasNext ? (
        <Button variant="next" ref={nextRef} onClick={onNext}>
          {s.puzzle.nextPuzzle} &rarr;
        </Button>
      ) : (
        <ButtonLink variant="next" ref={nextRef} href="/archive">
          {s.daily.archive} &rarr;
        </ButtonLink>
      )}
    </div>
  );
}
