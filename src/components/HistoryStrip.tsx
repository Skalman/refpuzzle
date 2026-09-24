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
export const ENABLED_HISTORY_STEP = "button.history-step:not(:disabled)";

/** One pill of the strip: a step, Start, the folded range, or Replay. */
function HistoryStepButton({
  class: extraClass,
  ...rest
}: Omit<ButtonHTMLAttributes, "class" | "className"> & { class?: string }) {
  return <button class={classNames("history-step", extraClass)} {...rest} />;
}

/** The icon slot leading a history pill. */
function HistoryIcon({
  class: extraClass,
  children,
}: {
  class?: string;
  children: ComponentChildren;
}) {
  return <span class={classNames("history-icon", extraClass)}>{children}</span>;
}

/** Each kind of step's icon: an answer, an elimination, a cleared cell, a checkpoint. */
const MOVE_ICONS: Record<MoveInfo["icon"], { class?: string; icon: ComponentChildren }> = {
  ok: { class: "icon-correct", icon: <IconCheck size="1.5em" strokeWidth={3} /> },
  no: { class: "icon-incorrect", icon: <IconX size="1.5em" strokeWidth={3} /> },
  un: { icon: <IconUndo size="1.5em" strokeWidth={3} /> },
  pin: { class: "icon-checkpoint", icon: <IconPin size="1.1em" /> },
};

/**
 * Hint marker: the escalation level reached at one step, or the number of markers
 * folded into the pill. Same badge either way. Renders nothing at zero or absent.
 */
function HintBadge({ value }: { value: number | undefined }) {
  if (!value) return null;
  return (
    <span class="history-hint">
      <IconHint size="1.5em" strokeWidth={3} class="icon-hint" />
      {value}
    </span>
  );
}

/** Refused checkpoint presses at one step. Renders nothing at zero. */
function FailBadge({ count }: { count: number }) {
  if (count <= 0) return null;
  return (
    <span class="history-fail" title={t().puzzle.checkpointFailsTitle(count)}>
      <IconAlert size="1.5em" strokeWidth={4} class="icon-error" />
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
      class={classNames("history-replay", armed && "armed")}
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

  return (
    <div
      ref={containerRef}
      class="history-strip"
      role="toolbar"
      onKeyDown={arrowNavHandler(ENABLED_HISTORY_STEP)}
    >
      {/* Leads the row: expanding the Solved pill pushes the whole track out to
          the right, and the way out shouldn't travel with it. */}
      {completed && <ReplayButton onPlayAgain={onPlayAgain} />}
      {collapsible && (
        <HistoryStepButton
          class={classNames(
            "history-collapsed",
            showAll && "expanded",
            !showAll && !completed && "joined",
            completed && "solved",
          )}
          aria-expanded={showAll}
          title={s.puzzle.verifiedTitle(answered, history[0].length)}
          onClick={() => setExpandedFold(expanded ? null : foldId)}
        >
          <HistoryIcon>
            <IconChevronDown size="1.2em" />
          </HistoryIcon>
          {completed ? s.puzzle.solvedBadge : s.puzzle.verifiedMarks(verifiedCount)}
          {/* Aggregates summarize the folded range; expanded, the steps show
              their own markers in place and the summary would double them. */}
          {!showAll && (
            <>
              <HintBadge value={hiddenHints} />
              <FailBadge count={hiddenFails} />
            </>
          )}
        </HistoryStepButton>
      )}
      {showAll && (
        <span class="history-entry">
          <HistoryStepButton
            class={classNames(currentIdx === 0 && "current")}
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
          <span key={i} class="history-entry">
            <HistoryStepButton
              class={classNames(
                joined && "joined",
                !completed && stepIdx === currentIdx && "current",
                stepIdx > currentIdx && "future",
                isCheckpoint && (isLastCp ? "checkpoint" : "checkpoint-old"),
              )}
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
