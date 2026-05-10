import { LETTERS } from "../engine/types.ts";
import { arrowNavHandler } from "../lib/keyboard.ts";
import type { QuestionState } from "../lib/store.ts";
import { t } from "../i18n/index.ts";
import { IconUndo, IconPin, IconHint, IconCheck, IconX, IconPlay } from "./Icons.tsx";

interface MoveInfo {
  text: string;
  icon: string;
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
      let icon: string;
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

export function HistoryStrip({
  history,
  currentIdx,
  hints,
  completed,
  onJump,
  containerRef,
}: {
  history: QuestionState[][];
  currentIdx: number;
  hints: Map<number, number>;
  completed: boolean;
  onJump: (idx: number) => void;
  containerRef?: { current: HTMLDivElement | null };
}) {
  const s = t();
  if (history.length <= 1) return null;

  const moves: MoveInfo[] = [];
  for (let i = 1; i < history.length; i++) {
    moves.push(describeDiff(history[i - 1], history[i]));
  }

  let lastCp = -1;
  if (!completed)
    for (let i = Math.min(currentIdx - 1, moves.length - 1); i >= 0; i--) {
      if (moves[i].qi < 0 && moves[i].icon) {
        lastCp = i;
        break;
      }
    }

  return (
    <div
      ref={containerRef}
      class="history-strip"
      role="toolbar"
      onKeyDown={arrowNavHandler("button.history-step:not(:disabled)")}
    >
      <button
        class={`history-step ${currentIdx === 0 ? "current" : ""}`}
        onClick={completed ? undefined : () => onJump(0)}
        disabled={completed}
      >
        <span class="history-icon">
          <IconPlay size="1em" />
        </span>{" "}
        {s.puzzle.start}
      </button>
      {moves.map((move, i) => {
        const stepIdx = i + 1;
        const hintLevel = hints.get(stepIdx);
        const isCheckpoint = move.qi < 0 && move.icon;
        return (
          // oxlint-disable-next-line react/no-array-index-key
          <span key={i} class="history-entry">
            <button
              class={`history-step ${!completed && stepIdx === currentIdx ? "current" : ""} ${stepIdx > currentIdx ? "future" : ""} ${isCheckpoint && i === lastCp ? "checkpoint" : ""} ${isCheckpoint && i !== lastCp ? "checkpoint-old" : ""}`}
              onClick={completed ? undefined : () => onJump(stepIdx)}
              disabled={completed}
              title={move.text}
            >
              {move.icon === "pin" && (
                <span class="history-icon">
                  <IconPin size="1.1em" />{" "}
                </span>
              )}
              {move.icon === "ok" && (
                <span class="history-icon icon-correct">
                  <IconCheck size="1.5em" strokeWidth={3} />{" "}
                </span>
              )}
              {move.icon === "no" && (
                <span class="history-icon icon-incorrect">
                  <IconX size="1.5em" strokeWidth={3} />{" "}
                </span>
              )}
              {move.icon === "un" && (
                <span class="history-icon">
                  <IconUndo size="1.5em" strokeWidth={3} />{" "}
                </span>
              )}
              {move.text}
            </button>
            {hintLevel != null && (
              <span class="history-hint">
                <IconHint size="1.5em" strokeWidth={3} class="icon-hint" />
                {hintLevel}
              </span>
            )}
          </span>
        );
      })}
      {completed && (
        <span class="history-step completed-step" aria-hidden="true">
          <IconCheck size="1.5em" strokeWidth={3} class="icon-correct" /> {s.puzzle.solvedBadge}
        </span>
      )}
    </div>
  );
}
