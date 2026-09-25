import { useEffect } from "preact/hooks";
import type { ComponentChildren } from "preact";
import { t } from "../i18n/index.ts";
import { confetti } from "../lib/confetti.ts";
import { useShareable } from "../lib/hooks.ts";
import { hostOf } from "../lib/share.ts";
import { Dialog } from "./ui/Dialog.tsx";
import { Button, ButtonLink } from "./ui/Button.tsx";
import { dayNumber } from "../puzzles/daily.ts";
import type { QuestionOutcome, SolveStats } from "../lib/solve-summary.ts";
import { IconAlert, IconClock, IconHint, IconPin, IconShare, IconUndo } from "./Icons.tsx";
import { Brand } from "./Brand.tsx";
import { tw } from "../lib/classNames.ts";

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

/** Each outcome's square in the result. */
const OUTCOME_SQUARE: Record<QuestionOutcome, string> = {
  clean: tw`inline-block size-[1.1rem] rounded-sm bg-valid`,
  hinted: tw`inline-block size-[1.1rem] rounded-sm bg-pending`,
  caught: tw`inline-block size-[1.1rem] rounded-sm bg-invalid`,
};

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
    <div
      class="rounded-lg border bg-[linear-gradient(135deg,var(--accent-soft),var(--bg-surface)_55%,var(--valid-soft))] px-[0.9rem] py-3 text-center leading-[1.3] text-default"
      aria-hidden="true"
    >
      <div class="mb-3 flex items-baseline justify-between gap-2 text-caption text-muted">
        <span class="font-bold text-default">
          <Brand />
        </span>
        <span>{s.daily.dayLabel(dayNumber(dateStr), dateStr)}</span>
      </div>
      <div class="text-section font-bold">{s.difficulty[level]}</div>
      {time && (
        <>
          <div class="text-caption text-muted">{s.puzzle.solvedIn}</div>
          <div class="mb-[0.6rem] text-title font-bold tabular-nums">{time}</div>
        </>
      )}
      <div class="mb-3 flex flex-col items-center gap-[0.2rem]">
        {squareLines.map((line, li) => (
          // oxlint-disable-next-line react/no-array-index-key
          <div key={li} class="flex gap-[0.2rem]">
            {line.map((outcome, oi) => (
              // oxlint-disable-next-line react/no-array-index-key
              <span key={oi} class={OUTCOME_SQUARE[outcome]} />
            ))}
          </div>
        ))}
        {perfect && <div class="text-caption text-muted">{s.share.perfectCaption}</div>}
      </div>
      <div class="text-caption text-muted">{host}</div>
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
    icon: <IconHint class="text-pending" />,
    ...(stats.hints ? solvedLines.peeker(stats.hints) : solvedLines.pathfinder),
  });
  lines.push(
    stats.checkpointFails
      ? {
          icon: <IconAlert strokeWidth={3} class="text-invalid" />,
          ...solvedLines.oopsie(stats.checkpointFails, pressed),
        }
      : {
          icon: <IconPin class="text-valid" />,
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
    <Dialog
      title={perfect ? s.puzzle.solvedPerfect : s.puzzle.solved}
      titleClass="text-valid"
      onClose={onClose}
    >
      <ul class="mb-3 ml-5 flex list-none flex-col gap-[0.45rem] p-0 text-left text-body text-muted">
        {lines.map((line) => (
          <li key={line.label} class="flex items-start gap-[0.6rem]">
            <span class="inline-flex h-lh flex-none items-center text-section">{line.icon}</span>
            <span>
              <strong class="text-default">{line.label}</strong>
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
      <div class="mt-3 mb-5 flex items-center justify-center gap-2">
        {shareable.canShare && (
          <Button variant="outline" icon={<IconShare />} onClick={shareable.share}>
            {s.share.share}
          </Button>
        )}
        <Button variant="outline" onClick={shareable.copy}>
          {shareable.copied ? s.share.copied : s.share.copyText}
        </Button>
      </div>
      <div class="flex flex-col items-center gap-2">
        {hasNext && (
          <Button
            variant="next"
            size="lg"
            class="w-full shadow-[0_0_12px_var(--valid-soft)]"
            onClick={onNext}
            autofocus
          >
            {s.puzzle.nextPuzzle} &rarr;
          </Button>
        )}
        {hasNext ? (
          <ButtonLink variant="ghost" href="/archive">
            {s.daily.archive}
          </ButtonLink>
        ) : (
          <ButtonLink
            variant="primary"
            size="lg"
            class="w-full shadow-[0_0_12px_var(--accent-soft)]"
            href="/archive"
            autofocus
          >
            {s.daily.archive} &rarr;
          </ButtonLink>
        )}
      </div>
    </Dialog>
  );
}
