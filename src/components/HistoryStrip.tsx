import { useEffect, useState } from "preact/hooks";
import type { ButtonHTMLAttributes, ComponentChildren } from "preact";
import { LETTERS } from "../engine/types.ts";
import { arrowNavHandler } from "../lib/keyboard.ts";
import { classNames } from "../lib/classNames.ts";
import type { FailMarker, HintMarker, QuestionState } from "../lib/store.ts";
import { t } from "../i18n/index.ts";
import {
  IconUndo,
  IconPin,
  IconHint,
  IconCheck,
  IconX,
  IconPlay,
  IconChevronDown,
  IconAlert,
  IconReplay,
} from "./Icons.tsx";

interface MoveInfo {
  text: string;
  icon: "ok" | "no" | "un" | "pin";
  qi: number;
  oi: number;
}

export function describeDiff(prev: QuestionState[], next: QuestionState[]): MoveInfo {
  let best: MoveInfo | null = null;
  let bestPriority = -1;
  for (let qi = 0; qi < prev.length; qi++) {
    for (let oi = 0; oi < 5; oi++) {
      const p = prev[qi].marks[oi];
      const n = next[qi].marks[oi];
      if (p === n) continue;
      const letter = LETTERS[oi];
      let priority: number;
      let text: string;
      let icon: MoveInfo["icon"];
      if (n === "correct") {
        text = `#${qi + 1}=${letter}`;
        icon = "ok";
        priority = 2;
      } else if (n === "incorrect") {
        text = `#${qi + 1} ${letter}`;
        icon = "no";
        priority = 1;
      } else {
        text = `#${qi + 1} ${letter}`;
        icon = "un";
        priority = 0;
      }
      if (priority > bestPriority) {
        best = { text, icon, qi, oi };
        bestPriority = priority;
      }
    }
  }
  if (!best) {
    return { text: "", icon: "pin", qi: -1, oi: -1 };
  }
  return best;
}

/** The strip's pills a press can land on, for its arrow keys and tab stop. */
export const ENABLED_HISTORY_STEP = "[data-history-step]:not(:disabled)";

/** How one pill of the strip stands; everything unset is a plain step. */
interface StepState {
  /** The settled range folded into one pill. */
  collapsed?: boolean;
  /** The folded pill once the board is solved: it carries the verdict. */
  solved?: boolean;
  /** Butted against its neighbor, so the folded pill and the pin read as one control. */
  joined?: boolean;
  /** Play again, pill-shaped like its Solved neighbor but the quieter of the two. */
  replay?: boolean;
  /** Replay's first press landed: the next one throws the solve away. */
  armed?: boolean;
  current?: boolean;
  /** Past the cursor. */
  future?: boolean;
  /** The checkpoint a rewind would land on. */
  checkpoint?: boolean;
  /** Any older checkpoint. */
  checkpointOld?: boolean;
  disabled?: boolean;
}

/**
 * A pill's classes, one value per property; where states compete the order
 * below decides. A flex container, so the icon is an item rather than an
 * inline box: aligned middle in a line box it would push the step half a pixel
 * taller than the pills, and the whole line stretches to the tallest step.
 */
function stepClass(state: StepState): string {
  const pill = state.collapsed || state.replay;
  return classNames(
    "inline-flex cursor-pointer items-center gap-[0.25em] border py-0.5 text-chip leading-(--strip-line) whitespace-nowrap disabled:cursor-default",
    pill ? "px-2" : "px-1.5",
    state.collapsed && state.joined
      ? "-mr-0.5 rounded-l-full rounded-r-none border-r-0"
      : pill
        ? "rounded-full"
        : state.joined
          ? "rounded-l-none rounded-r-sm"
          : "rounded-sm",
    state.collapsed && state.solved
      ? "border-valid"
      : state.armed
        ? "border-invalid"
        : state.collapsed
          ? "border-muted"
          : state.checkpoint
            ? "border-valid"
            : // An older checkpoint keeps the plain border, even under the cursor.
              state.checkpointOld
              ? undefined
              : state.current
                ? "border-accent"
                : undefined,
    state.armed
      ? "bg-invalid-soft"
      : state.current
        ? "bg-accent-soft hover:not-disabled:bg-hover"
        : state.collapsed
          ? "bg-hover"
          : "bg-surface hover:not-disabled:bg-hover",
    state.collapsed && state.solved
      ? "text-valid"
      : state.armed
        ? "text-invalid"
        : state.current
          ? "text-accent"
          : "text-muted",
    ((state.collapsed && state.solved) || state.armed || state.current) && "font-semibold",
    state.future ? "opacity-35" : state.disabled && "opacity-70",
  );
}

/** One pill of the strip: a step, Start, the folded range, or Replay. */
function HistoryStepButton({
  state,
  ...rest
}: Omit<ButtonHTMLAttributes, "class" | "className"> & { state: StepState }) {
  return (
    <button
      data-history-step
      class={stepClass({ ...state, disabled: rest.disabled === true })}
      {...rest}
    />
  );
}

/** The icon slot leading a history pill. */
function HistoryIcon({
  class: extraClass,
  children,
}: {
  class?: string;
  children: ComponentChildren;
}) {
  return (
    <span class={classNames("inline-flex h-(--strip-line) items-center", extraClass)}>
      {children}
    </span>
  );
}

/**
 * A marker badge beside a step, or inside the folded pill as plain text: no
 * box, so content on the label's line can't change the pill's height, and
 * icons at the chevron's scale so they read as line content.
 */
function badgeClass(folded: boolean | undefined, fail: boolean): string {
  return classNames(
    "inline-flex items-center leading-(--strip-line)",
    folded
      ? "ml-[0.15em] border-0 bg-transparent p-0"
      : "rounded-sm border bg-surface px-1 py-0.5 text-badge",
    fail ? "border-invalid text-invalid" : "opacity-70",
  );
}

/** Each kind of step's icon: an answer, an elimination, a cleared cell, a checkpoint. */
const MOVE_ICONS: Record<MoveInfo["icon"], { class?: string; icon: ComponentChildren }> = {
  ok: { class: "text-valid", icon: <IconCheck size="1.5em" strokeWidth={3} /> },
  no: { class: "text-invalid", icon: <IconX size="1.5em" strokeWidth={3} /> },
  un: { icon: <IconUndo size="1.5em" strokeWidth={3} /> },
  pin: { class: "text-valid", icon: <IconPin size="1.1em" /> },
};

/**
 * Hint marker: the escalation level reached at one step, or the number of markers
 * folded into the pill. Same badge either way. Renders nothing at zero or absent.
 */
function HintBadge({ value, folded }: { value: number | undefined; folded?: boolean }) {
  if (!value) return null;
  return (
    <span class={badgeClass(folded, false)} data-testid="history-hint">
      <IconHint size={folded ? "1.2em" : "1.5em"} strokeWidth={3} class="text-pending" />
      {value}
    </span>
  );
}

/** Refused checkpoint presses at one step. Renders nothing at zero. */
function FailBadge({ count, folded }: { count: number; folded?: boolean }) {
  if (count <= 0) return null;
  return (
    <span class={badgeClass(folded, true)} title={t().puzzle.checkpointFailsTitle(count)}>
      <IconAlert size={folded ? "1.2em" : "1.5em"} strokeWidth={4} class="text-invalid" />
      {count}
    </span>
  );
}

/**
 * Wipes the board for a second run at the same puzzle. Shown only once solved,
 * where it is the one way back — every step in the track is frozen by then.
 * Two presses: the first arms the button for three seconds, since the press
 * discards the solve and nothing can undo it.
 */
function ReplayButton({ onPlayAgain }: { onPlayAgain: () => void }) {
  const s = t();
  const [armed, setArmed] = useState(false);
  useEffect(() => {
    if (!armed) return undefined;
    const timer = setTimeout(() => setArmed(false), 3000);
    return () => clearTimeout(timer);
  }, [armed]);
  return (
    <HistoryStepButton
      state={{ replay: true, armed }}
      onClick={() => {
        if (!armed) {
          setArmed(true);
          return;
        }
        setArmed(false);
        onPlayAgain();
      }}
    >
      <HistoryIcon>
        <IconReplay size="1em" />
      </HistoryIcon>
      {armed ? s.puzzle.playAgainConfirm : s.puzzle.playAgain}
    </HistoryStepButton>
  );
}

/** Questions holding an answer on one board. */
function answeredCount(board: QuestionState[]): number {
  let n = 0;
  for (const q of board) {
    if (q.marks.indexOf("correct") >= 0) n++;
  }
  return n;
}

/**
 * Step index of the newest checkpoint at or before `upto`, or 0 when there is
 * none. A checkpoint is the step that changes no marks.
 */
export function lastCheckpointIdx(history: QuestionState[][], upto: number): number {
  for (let i = Math.min(upto, history.length - 1); i >= 1; i--) {
    if (describeDiff(history[i - 1], history[i]).qi < 0) return i;
  }
  return 0;
}

export function HistoryStrip({
  history,
  currentIdx,
  hints,
  fails,
  completed,
  onJump,
  onPlayAgain,
  containerRef,
}: {
  history: QuestionState[][];
  currentIdx: number;
  hints: Map<number, HintMarker>;
  /** Refused checkpoint presses, keyed by history step. */
  fails: Map<number, FailMarker>;
  completed: boolean;
  onJump: (idx: number) => void;
  onPlayAgain: () => void;
  containerRef?: { current: HTMLDivElement | null };
}) {
  const s = t();
  // The collapse boundary is the newest checkpoint anywhere in the track, so the
  // cursor can sit before it; the green highlight is the one the cursor stands
  // after, i.e. the checkpoint a rewind would land on.
  const cpStepIdx = lastCheckpointIdx(history, history.length - 1);
  const activeCpIdx = completed ? 0 : lastCheckpointIdx(history, currentIdx);
  // The pill is expanded only for the range it was opened on, so a newly planted
  // checkpoint re-collapses it, and so does solving the board — the range it
  // folds is what just changed underneath.
  const foldId = `${cpStepIdx}:${completed}`;
  const [expandedFold, setExpandedFold] = useState<string | null>(null);
  const expanded = expandedFold === foldId;
  if (history.length <= 1) return null;

  const moves: MoveInfo[] = [];
  for (let i = 1; i < history.length; i++) {
    moves.push(describeDiff(history[i - 1], history[i]));
  }

  // Steps 1 … foldTo-1 fold into one pill. It carries the range's progress, so it
  // earns its place from the very first folded step rather than once it saves
  // space. Its count is the conclusions the range established — answers and
  // eliminations — not the pins and retractions that are also steps.
  //
  // A solved board is settled ground in its entirety, so the pill swallows the
  // whole track and carries the Solved assertion itself.
  const foldTo = completed ? history.length : cpStepIdx;
  const hiddenSteps = foldTo - 1;
  const collapsible = completed || hiddenSteps >= 1;
  let verifiedCount = 0;
  for (let i = 0; i < hiddenSteps; i++) {
    if (moves[i].icon === "ok" || moves[i].icon === "no") verifiedCount++;
  }
  // Never hide the cursor: a jump back into the range forces it open. A completed
  // board has no live cursor — every step is disabled — so the rule lifts.
  const showAll = !collapsible || expanded || (!completed && currentIdx < foldTo);
  // The pill hides Start too, so index 0's annotations fold in with the steps'.
  let hiddenHints = 0;
  for (const key of hints.keys()) {
    if (key < foldTo) hiddenHints++;
  }
  let hiddenFails = 0;
  for (const [key, fail] of fails) {
    if (key < foldTo) hiddenFails += fail.count;
  }
  const answered = answeredCount(history[Math.min(foldTo, history.length - 1)]);

  // --strip-line: one content line shared by every control in the strip, so
  // steps, the pill and the badges come out exactly equal whatever their icon
  // sizes; it must fit the largest icon. The top padding centers the first row
  // on the dock's 3rem buttons.
  return (
    <div
      ref={containerRef}
      class="flex flex-auto flex-wrap gap-0.5 self-start pt-[calc((3rem-var(--strip-line)-0.25rem)/2)] pb-1.5 [--strip-line:1rem]"
      role="toolbar"
      onKeyDown={arrowNavHandler(ENABLED_HISTORY_STEP)}
    >
      {/* Leads the row: expanding the Solved pill pushes the whole track out to
          the right, and the way out shouldn't travel with it. */}
      {completed && <ReplayButton onPlayAgain={onPlayAgain} />}
      {collapsible && (
        <HistoryStepButton
          state={{ collapsed: true, joined: !showAll && !completed, solved: completed }}
          aria-expanded={showAll}
          title={s.puzzle.verifiedTitle(answered, history[0].length)}
          onClick={() => setExpandedFold(expanded ? null : foldId)}
        >
          <HistoryIcon
            class={classNames("transition-transform duration-150", showAll && "rotate-180")}
          >
            <IconChevronDown size="1.2em" />
          </HistoryIcon>
          {completed ? s.puzzle.solvedBadge : s.puzzle.verifiedMarks(verifiedCount)}
          {/* Aggregates summarize the folded range; expanded, the steps show
              their own markers in place and the summary would double them. */}
          {!showAll && (
            <>
              <HintBadge value={hiddenHints} folded />
              <FailBadge count={hiddenFails} folded />
            </>
          )}
        </HistoryStepButton>
      )}
      {showAll && (
        <span class="inline-flex items-center gap-0.5">
          <HistoryStepButton
            state={{ current: currentIdx === 0 }}
            onClick={completed ? undefined : () => onJump(0)}
            disabled={completed}
          >
            <HistoryIcon>
              <IconPlay size="1em" />
            </HistoryIcon>{" "}
            {s.puzzle.start}
          </HistoryStepButton>
          <HintBadge value={hints.get(0)?.level} />
          <FailBadge count={fails.get(0)?.count ?? 0} />
        </span>
      )}
      {moves.map((move, i) => {
        const stepIdx = i + 1;
        if (!showAll && stepIdx < foldTo) return null;
        const hintLevel = hints.get(stepIdx)?.level;
        const isCheckpoint = move.qi < 0;
        const isLastCp = stepIdx === activeCpIdx;
        // Butt up against the collapsed pill, so the pair reads as one control.
        const joined = !showAll && stepIdx === foldTo;
        return (
          // oxlint-disable-next-line react/no-array-index-key
          <span key={i} class="inline-flex items-center gap-0.5">
            <HistoryStepButton
              state={{
                joined,
                current: !completed && stepIdx === currentIdx,
                future: stepIdx > currentIdx,
                checkpoint: isCheckpoint && isLastCp,
                checkpointOld: isCheckpoint && !isLastCp,
              }}
              onClick={completed ? undefined : () => onJump(stepIdx)}
              disabled={completed}
              title={move.text}
            >
              <HistoryIcon class={MOVE_ICONS[move.icon].class}>
                {MOVE_ICONS[move.icon].icon}{" "}
              </HistoryIcon>
              {move.text}
            </HistoryStepButton>
            <HintBadge value={hintLevel} />
            <FailBadge count={fails.get(stepIdx)?.count ?? 0} />
          </span>
        );
      })}
    </div>
  );
}
