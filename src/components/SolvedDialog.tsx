import { useEffect } from "preact/hooks";
import type { ComponentChildren } from "preact";
import { t } from "../i18n/index.ts";
import { confetti } from "../lib/confetti.ts";
import { useShareable } from "../lib/hooks.ts";
import { hostOf } from "../lib/share.ts";
import { Modal } from "./ui/Modal.tsx";
import { Button, ButtonLink } from "./ui/Button.tsx";
import { dayNumber } from "../puzzles/daily.ts";
import { classNames } from "../lib/classNames.ts";
import type { QuestionOutcome, SolveStats } from "../lib/solve-summary.ts";
import { IconAlert, IconClock, IconHint, IconPin, IconShare, IconUndo } from "./Icons.tsx";
import { Brand } from "./Brand.tsx";

type TimeBand = "hot" | "speedy" | "smooth" | "deliberate";

/**
 * Per level, in seconds: a solve under the first is Hot!, under the second
 * Speedy, under the third Smooth, else Deliberate.
 */
const TIME_BANDS: Record<number, [number, number, number]> = {
  1: [5, 15, 60],
  2: [25, 50, 120],
  3: [90, 150, 240],
  4: [240, 420, 600],
  5: [360, 510, 840],
  6: [480, 720, 1200],
};

function timeBand(level: number, elapsedS: number): TimeBand {
  const [hot, speedy, smooth] = TIME_BANDS[level] ?? TIME_BANDS[6];
  if (elapsedS < hot) return "hot";
  if (elapsedS < speedy) return "speedy";
  if (elapsedS < smooth) return "smooth";
  return "deliberate";
}

/** `m:ss`, or `h:mm:ss` past the hour; null passes through. */
function formatDuration(totalS: number | null): string | null {
  if (totalS === null) return null;
  const hours = Math.floor(totalS / 3600);
  const minutes = Math.floor((totalS % 3600) / 60);
  const seconds = totalS % 60;
  const mm = hours ? String(minutes).padStart(2, "0") : String(minutes);
  return `${hours ? `${hours}:` : ""}${mm}:${String(seconds).padStart(2, "0")}`;
}

/** The most squares on one line of the result. */
const SQUARES_PER_LINE = 6;

/** The result's squares split into as few lines as fit, evened out: 8 → 4 + 4. */
function splitSquareLines(outcomes: QuestionOutcome[]): QuestionOutcome[][] {
  const lineCount = Math.ceil(outcomes.length / SQUARES_PER_LINE);
  const perLine = Math.ceil(outcomes.length / lineCount);
  return Array.from({ length: lineCount }, (_x, li) =>
    outcomes.slice(li * perLine, (li + 1) * perLine),
  );
}

/**
 * The shareable picture of a solve: level, day, time, and one square per
 * question colored by how it went.
 */
function ResultCard({
  dateStr,
  level,
  time,
  squareLines,
  perfect,
  host,
}: {
  dateStr: string;
  level: number;
  /** Null when the solve was never timed here; the card then skips it. */
  time: string | null;
  squareLines: QuestionOutcome[][];
  /** Every square clean; the squares then carry their caption. */
  perfect: boolean;
  host: string;
}) {
  const s = t();
  return (
    <div class="result-card" aria-hidden="true">
      <div class="result-card-top">
        <span class="result-card-brand">
          <Brand />
        </span>
        <span>{s.daily.dayLabel(dayNumber(dateStr), dateStr)}</span>
      </div>
      <div class="result-card-level">{s.difficulty[level]}</div>
      {time && (
        <>
          <div class="result-card-caption">{s.puzzle.solvedIn}</div>
          <div class="result-card-time">{time}</div>
        </>
      )}
      <div class="result-card-squares">
        {squareLines.map((line, li) => (
          // oxlint-disable-next-line react/no-array-index-key
          <div key={li} class="result-card-line">
            {line.map((outcome, oi) => (
              // oxlint-disable-next-line react/no-array-index-key
              <span key={oi} class={classNames("result-card-square", outcome)} />
            ))}
          </div>
        ))}
        {perfect && <div class="result-card-perfect">{s.share.perfectCaption}</div>}
      </div>
      <div class="result-card-host">{host}</div>
    </div>
  );
}

/** One summary line: a label, then what earned it when there is more to say. */
type SolvedLine = { icon: ComponentChildren; label: string; detail?: string };

/**
 * The solve's summary: stats, the result to share, then the ways onward.
 * Opened on the solve itself it celebrates, with confetti over the dialog;
 * reopened later from the completion bar it just reports.
 */
export function SolvedDialog({
  stats,
  dateStr,
  level,
  outcomes,
  hasNext,
  shareUrl,
  celebrate,
  onNext,
  onClose,
}: {
  stats: SolveStats;
  dateStr: string;
  level: number;
  outcomes: QuestionOutcome[];
  hasNext: boolean;
  shareUrl: string;
  celebrate: boolean;
  onNext: () => void;
  onClose: () => void;
}) {
  const s = t();

  // The shell opens the dialog in its own effect, which runs first, so the
  // confetti lands in the top layer above it.
  useEffect(() => (celebrate ? confetti() : undefined), [celebrate]);

  const time = formatDuration(stats.elapsedS);
  const perfect = outcomes.every((outcome) => outcome === "clean");
  const squareLines = splitSquareLines(outcomes);
  const emojiLines = squareLines.map((line) =>
    line.map((outcome) => s.share.outcomeEmoji[outcome]).join(""),
  );
  const shareable = useShareable({
    text: [
      s.share.resultHeadline(dayNumber(dateStr), s.difficulty[level], time),
      ...emojiLines,
      ...(perfect ? [s.share.perfectCaption] : []),
      shareUrl,
    ].join("\n"),
  });

  // One line per measure — time, hints, checkpoints, rewinds — leaving out
  // the measures this solve can't know.
  const { solvedLines } = s.puzzle;
  const pressed = stats.checkpoints + stats.checkpointFails;
  const lines: SolvedLine[] = [];
  if (time !== null && stats.elapsedS !== null) {
    lines.push({
      icon: <IconClock />,
      ...solvedLines.solvedIn[timeBand(level, stats.elapsedS)](time),
    });
  }
  lines.push({
    icon: <IconHint class="icon-hint" />,
    ...(stats.hints ? solvedLines.peeker(stats.hints) : solvedLines.pathfinder),
  });
  lines.push(
    stats.checkpointFails
      ? {
          icon: <IconAlert strokeWidth={3} class="icon-error" />,
          ...solvedLines.oopsie(stats.checkpointFails, pressed),
        }
      : {
          icon: <IconPin class="icon-checkpoint" />,
          ...(stats.checkpoints
            ? solvedLines.doubleChecker(stats.checkpoints)
            : solvedLines.freeSpirit),
        },
  );
  if (stats.historyBursts !== null) {
    lines.push({
      icon: <IconUndo />,
      ...(stats.historyBursts
        ? solvedLines.timeTraveler(stats.historyBursts)
        : solvedLines.straightSolver),
    });
  }

  return (
    <Modal
      title={perfect ? s.puzzle.solvedPerfect : s.puzzle.solved}
      class="solved-dialog"
      onClose={onClose}
    >
      <ul class="solved-lines">
        {lines.map((line) => (
          <li key={line.label}>
            <span class="solved-line-icon">{line.icon}</span>
            <span>
              <strong>{line.label}</strong>
              {line.detail && ` — ${line.detail}`}
            </span>
          </li>
        ))}
      </ul>
      <ResultCard
        dateStr={dateStr}
        level={level}
        time={time}
        squareLines={squareLines}
        perfect={perfect}
        host={hostOf(shareUrl)}
      />
      <div class="solved-share">
        {shareable.canShare && (
          <Button variant="outline" onClick={shareable.share}>
            <IconShare size="0.9em" /> {s.share.share}
          </Button>
        )}
        <Button variant="outline" onClick={shareable.copy}>
          {shareable.copied ? s.share.copied : s.share.copyText}
        </Button>
      </div>
      <div class="solved-actions">
        {hasNext && (
          <Button variant="next" class="solved-primary" onClick={onNext} autofocus>
            {s.puzzle.nextPuzzle} &rarr;
          </Button>
        )}
        {hasNext ? (
          <ButtonLink variant="text" href="/archive">
            {s.daily.archive}
          </ButtonLink>
        ) : (
          <ButtonLink variant="primary" class="solved-primary" href="/archive" autofocus>
            {s.daily.archive} &rarr;
          </ButtonLink>
        )}
      </div>
    </Modal>
  );
}
